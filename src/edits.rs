//! Lock-free queue through which the GUI thread reports parameter edits.
//!
//! The GUI writes the latest value and gesture state per parameter; the audio
//! thread (or the main-thread flush) drains them into host output events.

use crate::model::{BASS_WAVEFORM_PARAMETER_ID, CHANNEL_PARAMETER_ID, CONTINUOUS_PARAMETER_COUNT};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const NONE: u64 = u64::MAX;
/// One slot per continuous parameter, then one per switch or MIDI parameter.
const FIRST_SWITCH_SLOT: usize = CONTINUOUS_PARAMETER_COUNT;
const SLOT_COUNT: usize =
    FIRST_SWITCH_SLOT + (BASS_WAVEFORM_PARAMETER_ID - CHANNEL_PARAMETER_ID) as usize + 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    Begin(u32),
    Value(u32, f64),
    End(u32),
}

struct Slot {
    id: u32,
    pending: AtomicU64,
    want_gesture: AtomicBool,
    host_gesture: AtomicBool,
}

pub struct EditQueue {
    slots: [Slot; SLOT_COUNT],
    any: AtomicBool,
}

impl Default for EditQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn slot_index(id: u32) -> Option<usize> {
    if (id as usize) < CONTINUOUS_PARAMETER_COUNT {
        Some(id as usize)
    } else if (CHANNEL_PARAMETER_ID..=BASS_WAVEFORM_PARAMETER_ID).contains(&id) {
        Some(FIRST_SWITCH_SLOT + (id - CHANNEL_PARAMETER_ID) as usize)
    } else {
        None
    }
}

fn slot_id(index: usize) -> u32 {
    if index < FIRST_SWITCH_SLOT {
        index as u32
    } else {
        CHANNEL_PARAMETER_ID + (index - FIRST_SWITCH_SLOT) as u32
    }
}

impl EditQueue {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|index| Slot {
                id: slot_id(index),
                pending: AtomicU64::new(NONE),
                want_gesture: AtomicBool::new(false),
                host_gesture: AtomicBool::new(false),
            }),
            any: AtomicBool::new(false),
        }
    }

    /// GUI thread: the latest value wins. Non-finite values are ignored.
    pub fn set_value(&self, id: u32, value: f64) {
        if let Some(index) = slot_index(id)
            && value.is_finite()
        {
            self.slots[index]
                .pending
                .store(value.to_bits(), Ordering::Relaxed);
            // Release pairs with the Acquire in `drain`: a drain that sees `any`
            // also sees the slot written before it.
            self.any.store(true, Ordering::Release);
        }
    }

    /// GUI thread: a fader drag started (`true`) or was released (`false`).
    pub fn set_gesture(&self, id: u32, active: bool) {
        if let Some(index) = slot_index(id) {
            self.slots[index]
                .want_gesture
                .store(active, Ordering::Relaxed);
            self.any.store(true, Ordering::Release);
        }
    }

    /// GUI thread: releases every held gesture, e.g. when the editor closes mid-drag.
    pub fn end_all_gestures(&self) {
        for slot in &self.slots {
            slot.want_gesture.store(false, Ordering::Relaxed);
        }
        self.any.store(true, Ordering::Release);
    }

    /// Main thread, on state load: drops queued values so stale edits made before
    /// the load are not reported to the host afterwards. Gestures are kept so
    /// Begin and End stay paired.
    pub fn clear_values(&self) {
        for slot in &self.slots {
            slot.pending.store(NONE, Ordering::Relaxed);
        }
    }

    pub fn has_pending(&self) -> bool {
        self.any.load(Ordering::Acquire)
    }

    /// Emits pending edits to `sink`, which returns false when the host buffer is full.
    /// On failure nothing is lost: the next drain retries.
    pub fn drain(&self, mut sink: impl FnMut(Edit) -> bool) {
        if !self.any.swap(false, Ordering::AcqRel) {
            return;
        }
        for slot in &self.slots {
            if !Self::drain_slot(slot, &mut sink) {
                self.any.store(true, Ordering::Relaxed);
                return;
            }
        }
    }

    fn drain_slot(slot: &Slot, sink: &mut impl FnMut(Edit) -> bool) -> bool {
        let id = slot.id;
        let want = slot.want_gesture.load(Ordering::Relaxed);
        let mut have = slot.host_gesture.load(Ordering::Relaxed);
        if want && !have {
            if !sink(Edit::Begin(id)) {
                return false;
            }
            have = true;
            slot.host_gesture.store(true, Ordering::Relaxed);
        }
        let bits = slot.pending.swap(NONE, Ordering::Relaxed);
        if bits != NONE {
            let value = f64::from_bits(bits);
            let restore = || {
                let _ =
                    slot.pending
                        .compare_exchange(NONE, bits, Ordering::Relaxed, Ordering::Relaxed);
            };
            if have {
                if !sink(Edit::Value(id, value)) {
                    restore();
                    return false;
                }
            } else {
                if !sink(Edit::Begin(id)) {
                    restore();
                    return false;
                }
                // From here the host has seen a Begin that still needs its End.
                slot.host_gesture.store(true, Ordering::Relaxed);
                if !sink(Edit::Value(id, value)) {
                    restore();
                    return false;
                }
                if !sink(Edit::End(id)) {
                    return false;
                }
                slot.host_gesture.store(false, Ordering::Relaxed);
            }
        }
        // Re-read: a failed self-contained triple leaves host_gesture set.
        have = slot.host_gesture.load(Ordering::Relaxed);
        if !want && have {
            if !sink(Edit::End(id)) {
                return false;
            }
            slot.host_gesture.store(false, Ordering::Relaxed);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain_all(queue: &EditQueue) -> Vec<Edit> {
        let mut out = Vec::new();
        queue.drain(|edit| {
            out.push(edit);
            true
        });
        out
    }

    /// Sink wrapper that checks Begin/End alternate per id.
    struct Checker {
        open: [bool; SLOT_COUNT],
        values: Vec<Edit>,
    }

    impl Checker {
        fn new() -> Self {
            Self {
                open: [false; SLOT_COUNT],
                values: Vec::new(),
            }
        }
        fn push(&mut self, edit: Edit) {
            match edit {
                Edit::Begin(id) => {
                    let open = &mut self.open[slot_index(id).unwrap()];
                    assert!(!*open, "double Begin for {id}");
                    *open = true;
                }
                Edit::End(id) => {
                    let open = &mut self.open[slot_index(id).unwrap()];
                    assert!(*open, "End without Begin for {id}");
                    *open = false;
                }
                Edit::Value(id, _) => {
                    // A value outside a gesture is only valid inside a triple, which the
                    // alternation check above already covers.
                    let _ = id;
                }
            }
            self.values.push(edit);
        }
    }

    #[test]
    fn clear_values_drops_values_but_keeps_gestures_paired() {
        let q = EditQueue::new();
        q.set_gesture(4, true);
        assert_eq!(drain_all(&q), vec![Edit::Begin(4)]);
        q.set_value(4, 10.0);
        q.set_value(5, 20.0);
        q.clear_values();
        q.set_gesture(4, false);
        assert_eq!(drain_all(&q), vec![Edit::End(4)]);
    }

    #[test]
    fn end_all_gestures_closes_every_open_gesture() {
        let q = EditQueue::new();
        q.set_gesture(4, true);
        q.set_gesture(110, true);
        assert_eq!(drain_all(&q), vec![Edit::Begin(4), Edit::Begin(110)]);
        q.end_all_gestures();
        assert_eq!(drain_all(&q), vec![Edit::End(4), Edit::End(110)]);
        q.end_all_gestures();
        assert!(drain_all(&q).is_empty());
    }

    #[test]
    fn coalesces_values() {
        let q = EditQueue::new();
        q.set_value(3, 1.0);
        q.set_value(3, 2.0);
        q.set_value(3, 3.0);
        assert!(q.has_pending());
        assert_eq!(
            drain_all(&q),
            vec![Edit::Begin(3), Edit::Value(3, 3.0), Edit::End(3)]
        );
        assert!(!q.has_pending());
    }

    #[test]
    fn bare_value_is_self_contained() {
        let q = EditQueue::new();
        q.set_value(100, 42.0);
        assert_eq!(
            drain_all(&q),
            vec![Edit::Begin(100), Edit::Value(100, 42.0), Edit::End(100)]
        );
    }

    #[test]
    fn slot_mapping() {
        assert_eq!(slot_index(14), Some(14));
        assert_eq!(slot_index(100), Some(15));
        assert_eq!(slot_index(114), Some(29));
        assert_eq!(slot_index(15), None);
        assert_eq!(slot_index(115), None);
        let q = EditQueue::new();
        for (i, slot) in q.slots.iter().enumerate() {
            assert_eq!(slot_index(slot.id), Some(i));
        }
    }

    #[test]
    fn drag_has_one_begin_and_one_end() {
        let q = EditQueue::new();
        let mut all = Vec::new();
        q.set_gesture(5, true);
        q.set_value(5, 1.0);
        all.extend(drain_all(&q));
        q.set_value(5, 2.0);
        all.extend(drain_all(&q));
        q.set_value(5, 3.0);
        all.extend(drain_all(&q));
        q.set_gesture(5, false);
        all.extend(drain_all(&q));
        assert_eq!(
            all,
            vec![
                Edit::Begin(5),
                Edit::Value(5, 1.0),
                Edit::Value(5, 2.0),
                Edit::Value(5, 3.0),
                Edit::End(5)
            ]
        );
    }

    #[test]
    fn gesture_without_values_is_matched_pair() {
        let q = EditQueue::new();
        q.set_gesture(1, true);
        q.set_gesture(1, false);
        assert!(drain_all(&q).is_empty());
        q.set_gesture(1, true);
        let a = drain_all(&q);
        q.set_gesture(1, false);
        let b = drain_all(&q);
        assert_eq!([a, b].concat(), vec![Edit::Begin(1), Edit::End(1)]);
    }

    #[test]
    fn unknown_ids_ignored() {
        let q = EditQueue::new();
        q.set_value(50, 1.0);
        q.set_gesture(115, true);
        q.set_value(0, f64::NAN);
        assert!(!q.has_pending());
        assert!(drain_all(&q).is_empty());
    }

    #[test]
    fn empty_drain_never_calls_sink() {
        let q = EditQueue::new();
        let mut calls = 0;
        q.drain(|_| {
            calls += 1;
            true
        });
        assert_eq!(calls, 0);
    }

    #[test]
    fn sink_failure_at_every_position_loses_nothing() {
        // Bare value: Begin, Value, End. Drag release: End only.
        for fail_at in 0..3 {
            let q = EditQueue::new();
            q.set_value(7, 9.0);
            let mut checker = Checker::new();
            let mut call = 0;
            q.drain(|edit| {
                let ok = call != fail_at;
                call += 1;
                if ok {
                    checker.push(edit);
                }
                ok
            });
            assert!(q.has_pending(), "fail_at {fail_at}");
            q.drain(|edit| {
                checker.push(edit);
                true
            });
            assert!(checker.values.contains(&Edit::Value(7, 9.0)));
            assert!(!checker.open[7], "fail_at {fail_at}");
            assert!(!q.has_pending());
        }
        // Failure while gesture is held and when it is released.
        for fail_at in 0..2 {
            let q = EditQueue::new();
            let mut checker = Checker::new();
            q.set_gesture(2, true);
            q.set_value(2, 4.0);
            let mut call = 0;
            q.drain(|edit| {
                let ok = call != fail_at;
                call += 1;
                if ok {
                    checker.push(edit);
                }
                ok
            });
            q.drain(|edit| {
                checker.push(edit);
                true
            });
            assert!(checker.values.contains(&Edit::Value(2, 4.0)));
            assert!(checker.open[2]);
            q.set_gesture(2, false);
            q.drain(|edit| {
                checker.push(edit);
                true
            });
            assert!(!checker.open[2]);
        }
    }

    #[test]
    fn failed_value_does_not_overwrite_newer() {
        let q = EditQueue::new();
        q.set_gesture(4, true);
        q.set_value(4, 1.0);
        q.drain(|edit| {
            if matches!(edit, Edit::Value(..)) {
                // GUI writes a newer value while the sink is failing.
                q.set_value(4, 8.0);
                return false;
            }
            true
        });
        assert_eq!(drain_all(&q), vec![Edit::Value(4, 8.0)]);
    }

    #[test]
    fn pseudo_random_sequence_keeps_alternation() {
        let mut state: u64 = 0x1234_5678_9abc_def0;
        let mut next = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u32
        };
        let ids = [0u32, 3, 14, 100, 107, 114];
        let q = EditQueue::new();
        let mut checker = Checker::new();
        for _ in 0..20_000 {
            let id = ids[next() as usize % ids.len()];
            match next() % 4 {
                0 => q.set_value(id, (next() % 128) as f64),
                1 => q.set_gesture(id, true),
                2 => q.set_gesture(id, false),
                _ => {
                    let fail_in = next() % 6; // 0..=2 fail at that call, else succeed
                    let mut call = 0;
                    q.drain(|edit| {
                        let ok = call != fail_in;
                        call += 1;
                        if ok {
                            checker.push(edit);
                        }
                        ok
                    });
                }
            }
        }
        for id in ids {
            q.set_gesture(id, false);
        }
        for _ in 0..2 {
            q.drain(|edit| {
                checker.push(edit);
                true
            });
        }
        assert!(checker.open.iter().all(|open| !open));
        assert!(!q.has_pending());
    }
}

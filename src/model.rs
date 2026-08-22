use crate::protocol::{
    BASS_PARAMETERS, CHORD_PARAMETERS, MELODY_PARAMETERS, ParameterDefinition, bass_switches_1,
    control_change, melody_chord_dynamics, melody_chord_switches_1, melody_chord_switches_2,
    parameter_change,
};
use std::array;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};

pub const CHANNEL_PARAMETER_ID: u32 = 100;
pub const MIDI_OUTPUT_PARAMETER_ID: u32 = 101;
pub const WHOLE_MODE_PARAMETER_ID: u32 = 102;
pub const RANGE_PARAMETER_ID: u32 = 103;
pub const PULSE_PARAMETER_ID: u32 = 104;
pub const SAW_PARAMETER_ID: u32 = 105;
pub const CHORUS_PARAMETER_ID: u32 = 106;
pub const VCF_DYNAMICS_PARAMETER_ID: u32 = 107;
pub const VCA_DYNAMICS_PARAMETER_ID: u32 = 108;
pub const PWM_SOURCE_PARAMETER_ID: u32 = 109;
pub const VCA_MODE_PARAMETER_ID: u32 = 110;
pub const ENV_POLARITY_PARAMETER_ID: u32 = 111;
pub const HPF_PARAMETER_ID: u32 = 112;
pub const NOISE_PARAMETER_ID: u32 = 113;
pub const BASS_WAVEFORM_PARAMETER_ID: u32 = 114;

const RELAXED: Ordering = Ordering::Relaxed;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    Melody,
    Chord,
    Bass,
}

impl Part {
    pub const fn parameters(self) -> &'static [ParameterDefinition] {
        match self {
            Self::Melody => &MELODY_PARAMETERS,
            Self::Chord => &CHORD_PARAMETERS,
            Self::Bass => &BASS_PARAMETERS,
        }
    }

    pub const fn default_channel(self) -> i32 {
        match self {
            Self::Melody => 1,
            Self::Chord => 3,
            Self::Bass => 2,
        }
    }

    pub const fn name(self) -> &'static [u8] {
        match self {
            Self::Melody => b"Melody",
            Self::Chord => b"Chord",
            Self::Bass => b"Bass",
        }
    }

    pub const fn supports_whole_mode(self) -> bool {
        matches!(self, Self::Chord)
    }
    pub const fn supports_oscillator_switches(self) -> bool {
        !matches!(self, Self::Bass)
    }
    pub const fn is_melody(self) -> bool {
        matches!(self, Self::Melody)
    }
    pub const fn is_bass(self) -> bool {
        matches!(self, Self::Bass)
    }
    pub const fn switch_control_count(self) -> usize {
        if self.supports_oscillator_switches() {
            10 + self.is_melody() as usize
        } else {
            1
        }
    }
    pub const fn parameter_count(self) -> usize {
        self.parameters().len()
            + self.switch_control_count()
            + 2
            + self.supports_whole_mode() as usize
    }
}

pub struct AtomicF64(AtomicU64);

impl AtomicF64 {
    fn new(value: f64) -> Self {
        Self(AtomicU64::new(value.to_bits()))
    }
    pub fn load(&self) -> f64 {
        f64::from_bits(self.0.load(RELAXED))
    }
    fn store(&self, value: f64) {
        self.0.store(value.to_bits(), RELAXED);
    }
    fn swap(&self, value: f64) -> f64 {
        f64::from_bits(self.0.swap(value.to_bits(), RELAXED))
    }
}

pub struct State {
    pub part: Part,
    values: [AtomicF64; 15],
    modulations: [AtomicF64; 15],
    dirty: [AtomicBool; 15],
    channel: AtomicI32,
    output_index: AtomicUsize,
    whole_mode: AtomicBool,
    whole_mode_dirty: AtomicBool,
    range: AtomicI32,
    pulse: AtomicBool,
    saw: AtomicBool,
    chorus: AtomicBool,
    has_oscillator: AtomicBool,
    oscillator_dirty: AtomicBool,
    vcf_dynamics: AtomicBool,
    vca_dynamics: AtomicBool,
    has_dynamics: AtomicBool,
    dynamics_dirty: AtomicBool,
    pwm_manual: AtomicBool,
    env_negative: AtomicBool,
    vca_gate: AtomicBool,
    hpf: AtomicBool,
    noise: AtomicBool,
    has_secondary: AtomicBool,
    secondary_dirty: AtomicBool,
    bass_saw: AtomicBool,
    has_bass_waveform: AtomicBool,
    bass_waveform_dirty: AtomicBool,
    authoritative: AtomicBool,
}

impl State {
    pub fn new(part: Part) -> Self {
        Self {
            part,
            values: array::from_fn(|_| AtomicF64::new(0.0)),
            modulations: array::from_fn(|_| AtomicF64::new(0.0)),
            dirty: array::from_fn(|_| AtomicBool::new(false)),
            channel: AtomicI32::new(part.default_channel()),
            output_index: AtomicUsize::new(0),
            whole_mode: AtomicBool::new(false),
            whole_mode_dirty: AtomicBool::new(false),
            range: AtomicI32::new(1),
            pulse: AtomicBool::new(true),
            saw: AtomicBool::new(false),
            chorus: AtomicBool::new(false),
            has_oscillator: AtomicBool::new(false),
            oscillator_dirty: AtomicBool::new(false),
            vcf_dynamics: AtomicBool::new(false),
            vca_dynamics: AtomicBool::new(false),
            has_dynamics: AtomicBool::new(false),
            dynamics_dirty: AtomicBool::new(false),
            pwm_manual: AtomicBool::new(false),
            env_negative: AtomicBool::new(false),
            vca_gate: AtomicBool::new(false),
            hpf: AtomicBool::new(false),
            noise: AtomicBool::new(false),
            has_secondary: AtomicBool::new(false),
            secondary_dirty: AtomicBool::new(false),
            bass_saw: AtomicBool::new(false),
            has_bass_waveform: AtomicBool::new(false),
            bass_waveform_dirty: AtomicBool::new(false),
            authoritative: AtomicBool::new(false),
        }
    }

    pub fn output_index(&self) -> usize {
        self.output_index.load(RELAXED)
    }
    pub fn has_authoritative_state(&self) -> bool {
        self.authoritative.load(RELAXED)
    }

    pub fn get_value(&self, id: u32) -> Option<f64> {
        if (id as usize) < self.part.parameters().len() {
            return Some(self.values[id as usize].load());
        }
        Some(match id {
            CHANNEL_PARAMETER_ID => self.channel.load(RELAXED) as f64,
            MIDI_OUTPUT_PARAMETER_ID => self.output_index() as f64,
            WHOLE_MODE_PARAMETER_ID if self.part.supports_whole_mode() => {
                self.whole_mode.load(RELAXED) as u8 as f64
            }
            RANGE_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.range.load(RELAXED) as f64
            }
            PULSE_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.pulse.load(RELAXED) as u8 as f64
            }
            SAW_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.saw.load(RELAXED) as u8 as f64
            }
            CHORUS_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.chorus.load(RELAXED) as u8 as f64
            }
            VCF_DYNAMICS_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.vcf_dynamics.load(RELAXED) as u8 as f64
            }
            VCA_DYNAMICS_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.vca_dynamics.load(RELAXED) as u8 as f64
            }
            PWM_SOURCE_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.pwm_manual.load(RELAXED) as u8 as f64
            }
            VCA_MODE_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.vca_gate.load(RELAXED) as u8 as f64
            }
            ENV_POLARITY_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.env_negative.load(RELAXED) as u8 as f64
            }
            HPF_PARAMETER_ID if self.part.supports_oscillator_switches() => {
                self.hpf.load(RELAXED) as u8 as f64
            }
            NOISE_PARAMETER_ID if self.part.is_melody() => self.noise.load(RELAXED) as u8 as f64,
            BASS_WAVEFORM_PARAMETER_ID if self.part.is_bass() => {
                self.bass_saw.load(RELAXED) as u8 as f64
            }
            _ => return None,
        })
    }

    pub fn apply_value(&self, id: u32, value: f64, output_count: usize) {
        if (id as usize) < self.part.parameters().len() {
            let index = id as usize;
            let value = value.round().clamp(0.0, 127.0);
            if self.values[index].swap(value) != value {
                self.dirty[index].store(true, RELAXED);
                self.authoritative.store(true, RELAXED);
            }
            return;
        }
        if id == CHANNEL_PARAMETER_ID {
            let channel = (value.round() as i32).clamp(1, 16);
            if self.channel.swap(channel, RELAXED) != channel {
                self.authoritative.store(true, RELAXED);
                self.mark_all_dirty();
            }
            return;
        }
        if id == MIDI_OUTPUT_PARAMETER_ID {
            let output =
                (value.round() as isize).clamp(0, output_count.saturating_sub(1) as isize) as usize;
            if self.output_index.swap(output, RELAXED) != output {
                self.mark_all_dirty();
            }
            return;
        }
        if id == WHOLE_MODE_PARAMETER_ID && self.part.supports_whole_mode() {
            let enabled = value >= 0.5;
            if self.whole_mode.swap(enabled, RELAXED) != enabled {
                self.whole_mode_dirty.store(true, RELAXED);
                self.authoritative.store(true, RELAXED);
            }
            return;
        }
        if self.part.supports_oscillator_switches() {
            let enabled = value >= 0.5;
            let (changed, present, dirty) = match id {
                RANGE_PARAMETER_ID => (
                    self.range.swap((value.round() as i32).clamp(0, 2), RELAXED)
                        != (value.round() as i32).clamp(0, 2),
                    &self.has_oscillator,
                    &self.oscillator_dirty,
                ),
                PULSE_PARAMETER_ID => (
                    self.pulse.swap(enabled, RELAXED) != enabled,
                    &self.has_oscillator,
                    &self.oscillator_dirty,
                ),
                SAW_PARAMETER_ID => (
                    self.saw.swap(enabled, RELAXED) != enabled,
                    &self.has_oscillator,
                    &self.oscillator_dirty,
                ),
                CHORUS_PARAMETER_ID => (
                    self.chorus.swap(enabled, RELAXED) != enabled,
                    &self.has_oscillator,
                    &self.oscillator_dirty,
                ),
                VCF_DYNAMICS_PARAMETER_ID => (
                    self.vcf_dynamics.swap(enabled, RELAXED) != enabled,
                    &self.has_dynamics,
                    &self.dynamics_dirty,
                ),
                VCA_DYNAMICS_PARAMETER_ID => (
                    self.vca_dynamics.swap(enabled, RELAXED) != enabled,
                    &self.has_dynamics,
                    &self.dynamics_dirty,
                ),
                PWM_SOURCE_PARAMETER_ID => (
                    self.pwm_manual.swap(enabled, RELAXED) != enabled,
                    &self.has_secondary,
                    &self.secondary_dirty,
                ),
                VCA_MODE_PARAMETER_ID => (
                    self.vca_gate.swap(enabled, RELAXED) != enabled,
                    &self.has_secondary,
                    &self.secondary_dirty,
                ),
                ENV_POLARITY_PARAMETER_ID => (
                    self.env_negative.swap(enabled, RELAXED) != enabled,
                    &self.has_secondary,
                    &self.secondary_dirty,
                ),
                HPF_PARAMETER_ID => (
                    self.hpf.swap(enabled, RELAXED) != enabled,
                    &self.has_secondary,
                    &self.secondary_dirty,
                ),
                NOISE_PARAMETER_ID if self.part.is_melody() => (
                    self.noise.swap(enabled, RELAXED) != enabled,
                    &self.has_secondary,
                    &self.secondary_dirty,
                ),
                _ => return,
            };
            if changed || !present.swap(true, RELAXED) {
                dirty.store(true, RELAXED);
                self.authoritative.store(true, RELAXED);
            }
            return;
        }
        if id == BASS_WAVEFORM_PARAMETER_ID && self.part.is_bass() {
            let saw = value >= 0.5;
            if self.bass_saw.swap(saw, RELAXED) != saw
                || !self.has_bass_waveform.swap(true, RELAXED)
            {
                self.bass_waveform_dirty.store(true, RELAXED);
                self.authoritative.store(true, RELAXED);
            }
        }
    }

    pub fn apply_modulation(&self, id: u32, amount: f64) {
        if (id as usize) < self.part.parameters().len()
            && self.modulations[id as usize].swap(amount) != amount
        {
            self.dirty[id as usize].store(true, RELAXED);
        }
    }

    pub fn mark_all_dirty(&self) {
        for dirty in &self.dirty[..self.part.parameters().len()] {
            dirty.store(true, RELAXED);
        }
        if self.part.supports_whole_mode() {
            self.whole_mode_dirty.store(true, RELAXED);
        }
        if self.part.supports_oscillator_switches() && self.has_oscillator.load(RELAXED) {
            self.oscillator_dirty.store(true, RELAXED);
        }
        if self.part.supports_oscillator_switches() && self.has_dynamics.load(RELAXED) {
            self.dynamics_dirty.store(true, RELAXED);
        }
        if self.part.supports_oscillator_switches() && self.has_secondary.load(RELAXED) {
            self.secondary_dirty.store(true, RELAXED);
        }
        if self.part.is_bass() && self.has_bass_waveform.load(RELAXED) {
            self.bass_waveform_dirty.store(true, RELAXED);
        }
    }

    pub fn has_dirty(&self) -> bool {
        self.whole_mode_dirty.load(RELAXED)
            || self.oscillator_dirty.load(RELAXED)
            || self.dynamics_dirty.load(RELAXED)
            || self.secondary_dirty.load(RELAXED)
            || self.bass_waveform_dirty.load(RELAXED)
            || self.dirty[..self.part.parameters().len()]
                .iter()
                .any(|d| d.load(RELAXED))
    }

    pub fn take_next_message(&self, next_parameter: &mut usize) -> Option<PendingMessage> {
        let channel = self.channel.load(RELAXED);
        macro_rules! take {
            ($condition:expr, $dirty:expr, $kind:expr, $message:expr) => {
                if $condition && $dirty.swap(false, RELAXED) {
                    return Some(PendingMessage {
                        bytes: $message.to_vec(),
                        kind: $kind,
                    });
                }
            };
        }
        take!(
            self.part.supports_whole_mode(),
            self.whole_mode_dirty,
            DirtyKind::WholeMode,
            control_change(
                channel,
                121,
                if self.whole_mode.load(RELAXED) {
                    127
                } else {
                    0
                }
            )
            .unwrap()
        );
        take!(
            self.part.supports_oscillator_switches(),
            self.oscillator_dirty,
            DirtyKind::Oscillator,
            parameter_change(
                channel,
                16,
                melody_chord_switches_1(
                    self.range.load(RELAXED),
                    self.pulse.load(RELAXED),
                    self.saw.load(RELAXED),
                    self.chorus.load(RELAXED)
                )
                .unwrap() as i32
            )
            .unwrap()
        );
        take!(
            self.part.supports_oscillator_switches(),
            self.dynamics_dirty,
            DirtyKind::Dynamics,
            parameter_change(
                channel,
                4,
                melody_chord_dynamics(
                    self.vcf_dynamics.load(RELAXED),
                    self.vca_dynamics.load(RELAXED)
                ) as i32
            )
            .unwrap()
        );
        take!(
            self.part.supports_oscillator_switches(),
            self.secondary_dirty,
            DirtyKind::Secondary,
            parameter_change(
                channel,
                17,
                melody_chord_switches_2(
                    self.pwm_manual.load(RELAXED),
                    self.env_negative.load(RELAXED),
                    self.vca_gate.load(RELAXED),
                    self.hpf.load(RELAXED),
                    self.part.is_melody() && self.noise.load(RELAXED)
                ) as i32
            )
            .unwrap()
        );
        take!(
            self.part.is_bass(),
            self.bass_waveform_dirty,
            DirtyKind::BassWaveform,
            parameter_change(
                channel,
                16,
                bass_switches_1(self.bass_saw.load(RELAXED)) as i32
            )
            .unwrap()
        );

        let count = self.part.parameters().len();
        for offset in 0..count {
            let index = (*next_parameter + offset) % count;
            if !self.dirty[index].swap(false, RELAXED) {
                continue;
            }
            let effective = (self.values[index].load() + self.modulations[index].load())
                .round()
                .clamp(0.0, 127.0) as i32;
            let bytes = parameter_change(
                channel,
                self.part.parameters()[index].protocol_number as i32,
                effective,
            )
            .unwrap()
            .to_vec();
            *next_parameter = (index + 1) % count;
            return Some(PendingMessage {
                bytes,
                kind: DirtyKind::Parameter(index),
            });
        }
        None
    }

    pub fn retry(&self, kind: DirtyKind) {
        match kind {
            DirtyKind::WholeMode => self.whole_mode_dirty.store(true, RELAXED),
            DirtyKind::Oscillator => self.oscillator_dirty.store(true, RELAXED),
            DirtyKind::Dynamics => self.dynamics_dirty.store(true, RELAXED),
            DirtyKind::Secondary => self.secondary_dirty.store(true, RELAXED),
            DirtyKind::BassWaveform => self.bass_waveform_dirty.store(true, RELAXED),
            DirtyKind::Parameter(index) => self.dirty[index].store(true, RELAXED),
        }
    }

    pub fn save(&self, selected_output_unique_id: i32) -> Vec<u8> {
        let count = self.part.parameters().len();
        let v1_size = 6 + count;
        let switch_offset = v1_size + 4 + self.part.supports_whole_mode() as usize;
        let secondary_offset = switch_offset + 5;
        let size = if self.part.supports_oscillator_switches() {
            secondary_offset + 9
        } else {
            switch_offset + 2
        };
        let mut out = vec![0; size];
        out[..5].copy_from_slice(b"MKS7\x05");
        out[5] = self.channel.load(RELAXED) as u8;
        for index in 0..count {
            out[6 + index] = self.values[index].load().round().clamp(0.0, 127.0) as u8;
        }
        out[v1_size..v1_size + 4]
            .copy_from_slice(&(selected_output_unique_id as u32).to_le_bytes());
        if self.part.supports_whole_mode() {
            out[v1_size + 4] = self.whole_mode.load(RELAXED) as u8;
        }
        if self.part.supports_oscillator_switches() {
            out[switch_offset..switch_offset + 5].copy_from_slice(&[
                self.has_oscillator.load(RELAXED) as u8,
                self.range.load(RELAXED) as u8,
                self.pulse.load(RELAXED) as u8,
                self.saw.load(RELAXED) as u8,
                self.chorus.load(RELAXED) as u8,
            ]);
            out[secondary_offset..secondary_offset + 9].copy_from_slice(&[
                self.has_dynamics.load(RELAXED) as u8,
                self.vcf_dynamics.load(RELAXED) as u8,
                self.vca_dynamics.load(RELAXED) as u8,
                self.has_secondary.load(RELAXED) as u8,
                self.pwm_manual.load(RELAXED) as u8,
                self.env_negative.load(RELAXED) as u8,
                self.vca_gate.load(RELAXED) as u8,
                self.hpf.load(RELAXED) as u8,
                self.noise.load(RELAXED) as u8,
            ]);
        } else {
            out[switch_offset] = self.has_bass_waveform.load(RELAXED) as u8;
            out[switch_offset + 1] = self.bass_saw.load(RELAXED) as u8;
        }
        out
    }

    pub fn serialized_size(&self, version: u8) -> Option<usize> {
        let count = self.part.parameters().len();
        let v1_size = 6 + count;
        let switch_offset = v1_size + 4 + self.part.supports_whole_mode() as usize;
        let secondary_offset = switch_offset + 5;
        let state_size = if self.part.supports_oscillator_switches() {
            secondary_offset + 9
        } else {
            switch_offset + 2
        };
        match version {
            1 => Some(v1_size),
            2 => Some(v1_size + 4),
            3 if self.part.supports_whole_mode() => Some(v1_size + 5),
            4 if self.part.supports_oscillator_switches() => Some(switch_offset + 5),
            5 => Some(state_size),
            _ => None,
        }
    }

    pub fn load(&self, data: &[u8], output_ids: &[i32]) -> Result<(), StateError> {
        if data.len() < 5 || &data[..4] != b"MKS7" {
            return Err(StateError);
        }
        let version = data[4];
        let count = self.part.parameters().len();
        let v1_size = 6 + count;
        let switch_offset = v1_size + 4 + self.part.supports_whole_mode() as usize;
        let secondary_offset = switch_offset + 5;
        let serialized_size = self.serialized_size(version).ok_or(StateError)?;
        if data.len() < serialized_size
            || !(1..=16).contains(&data[5])
            || data[6..6 + count].iter().any(|v| *v > 127)
        {
            return Err(StateError);
        }
        if self.part.supports_whole_mode() && version >= 3 && data[v1_size + 4] > 1 {
            return Err(StateError);
        }
        if self.part.supports_oscillator_switches()
            && version >= 4
            && (data[switch_offset] > 1
                || data[switch_offset + 1] > 2
                || data[switch_offset + 2..switch_offset + 5]
                    .iter()
                    .any(|v| *v > 1))
        {
            return Err(StateError);
        }
        if version == 5 {
            if self.part.supports_oscillator_switches() {
                if data[secondary_offset..secondary_offset + 9]
                    .iter()
                    .any(|v| *v > 1)
                {
                    return Err(StateError);
                }
            } else if data[switch_offset..switch_offset + 2]
                .iter()
                .any(|v| *v > 1)
            {
                return Err(StateError);
            }
        }

        self.channel.store(data[5] as i32, RELAXED);
        for index in 0..count {
            self.values[index].store(data[6 + index] as f64);
            self.modulations[index].store(0.0);
        }
        let output_index = if version >= 2 {
            let id = i32::from_le_bytes(data[v1_size..v1_size + 4].try_into().unwrap());
            output_ids
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(i, candidate)| (*candidate == id).then_some(i))
                .unwrap_or(0)
        } else {
            0
        };
        self.output_index.store(output_index, RELAXED);
        self.whole_mode.store(
            self.part.supports_whole_mode() && version >= 3 && data[v1_size + 4] != 0,
            RELAXED,
        );
        if self.part.supports_oscillator_switches() && version >= 4 {
            self.has_oscillator.store(data[switch_offset] != 0, RELAXED);
            self.range.store(data[switch_offset + 1] as i32, RELAXED);
            self.pulse.store(data[switch_offset + 2] != 0, RELAXED);
            self.saw.store(data[switch_offset + 3] != 0, RELAXED);
            self.chorus.store(data[switch_offset + 4] != 0, RELAXED);
        } else {
            self.has_oscillator.store(false, RELAXED);
            self.range.store(1, RELAXED);
            self.pulse.store(true, RELAXED);
            self.saw.store(false, RELAXED);
            self.chorus.store(false, RELAXED);
        }
        if self.part.supports_oscillator_switches() && version == 5 {
            self.has_dynamics
                .store(data[secondary_offset] != 0, RELAXED);
            self.vcf_dynamics
                .store(data[secondary_offset + 1] != 0, RELAXED);
            self.vca_dynamics
                .store(data[secondary_offset + 2] != 0, RELAXED);
            self.has_secondary
                .store(data[secondary_offset + 3] != 0, RELAXED);
            self.pwm_manual
                .store(data[secondary_offset + 4] != 0, RELAXED);
            self.env_negative
                .store(data[secondary_offset + 5] != 0, RELAXED);
            self.vca_gate
                .store(data[secondary_offset + 6] != 0, RELAXED);
            self.hpf.store(data[secondary_offset + 7] != 0, RELAXED);
            self.noise.store(
                self.part.is_melody() && data[secondary_offset + 8] != 0,
                RELAXED,
            );
        } else {
            self.has_dynamics.store(false, RELAXED);
            self.vcf_dynamics.store(false, RELAXED);
            self.vca_dynamics.store(false, RELAXED);
            self.has_secondary.store(false, RELAXED);
            self.pwm_manual.store(false, RELAXED);
            self.env_negative.store(false, RELAXED);
            self.vca_gate.store(false, RELAXED);
            self.hpf.store(false, RELAXED);
            self.noise.store(false, RELAXED);
        }
        if self.part.is_bass() && version == 5 {
            self.has_bass_waveform
                .store(data[switch_offset] != 0, RELAXED);
            self.bass_saw.store(data[switch_offset + 1] != 0, RELAXED);
        } else {
            self.has_bass_waveform.store(false, RELAXED);
            self.bass_saw.store(false, RELAXED);
        }
        self.oscillator_dirty.store(false, RELAXED);
        self.dynamics_dirty.store(false, RELAXED);
        self.secondary_dirty.store(false, RELAXED);
        self.bass_waveform_dirty.store(false, RELAXED);
        self.authoritative.store(true, RELAXED);
        self.mark_all_dirty();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirtyKind {
    WholeMode,
    Oscillator,
    Dynamics,
    Secondary,
    BassWaveform,
    Parameter(usize),
}

#[derive(Debug, Eq, PartialEq)]
pub struct PendingMessage {
    pub bytes: Vec<u8>,
    pub kind: DirtyKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_uses_priority_coalescing_modulation_and_round_robin() {
        let state = State::new(Part::Chord);
        state.apply_value(4, 96.0, 1);
        state.apply_modulation(4, 2.4);
        state.apply_value(RANGE_PARAMETER_ID, 2.0, 1);
        state.apply_value(VCF_DYNAMICS_PARAMETER_ID, 1.0, 1);
        state.apply_value(PWM_SOURCE_PARAMETER_ID, 1.0, 1);
        state.apply_value(WHOLE_MODE_PARAMETER_ID, 1.0, 1);
        let mut next = 0;
        assert_eq!(
            state.take_next_message(&mut next).unwrap().bytes,
            [0xb2, 121, 127]
        );
        assert_eq!(state.take_next_message(&mut next).unwrap().bytes[4], 16);
        assert_eq!(state.take_next_message(&mut next).unwrap().bytes[4], 4);
        assert_eq!(state.take_next_message(&mut next).unwrap().bytes[4], 17);
        assert_eq!(
            state.take_next_message(&mut next).unwrap().bytes,
            [0xf0, 0x41, 0x32, 2, 5, 98, 0xf7]
        );
        assert!(state.take_next_message(&mut next).is_none());
    }

    #[test]
    fn presence_and_authoritative_semantics_match_legacy_behavior() {
        let state = State::new(Part::Melody);
        state.apply_value(0, 0.0, 1);
        assert!(!state.has_authoritative_state());
        state.apply_value(SAW_PARAMETER_ID, 0.0, 1);
        assert!(state.has_authoritative_state());
        assert!(state.has_dirty());
        let saved = state.save(0);
        assert_eq!(saved[25], 1);
        assert_eq!(saved[28], 0);
    }

    #[test]
    fn state_v1_through_v5_load_and_v5_round_trip() {
        let state = State::new(Part::Melody);
        for version in 1..=5 {
            let mut bytes = match version {
                1 => vec![0; 21],
                2 => vec![0; 25],
                3 => continue,
                4 => vec![0; 30],
                _ => vec![0; 39],
            };
            bytes[..5].copy_from_slice(&[b'M', b'K', b'S', b'7', version]);
            bytes[5] = 7;
            bytes[10] = 24;
            if version >= 4 {
                bytes[26] = 1;
                bytes[27] = 1;
            }
            state.load(&bytes, &[0]).unwrap();
            assert_eq!(state.get_value(CHANNEL_PARAMETER_ID), Some(7.0));
            assert_eq!(state.get_value(4), Some(24.0));
        }
        let saved = state.save(-1234);
        let restored = State::new(Part::Melody);
        restored.load(&saved, &[0, -1234]).unwrap();
        assert_eq!(restored.output_index(), 1);
        assert_eq!(restored.save(-1234), saved);
    }

    #[test]
    fn validates_part_specific_legacy_versions() {
        let melody = State::new(Part::Melody);
        let mut v3 = vec![0; 26];
        v3[..6].copy_from_slice(b"MKS7\x03\x01");
        assert_eq!(melody.load(&v3, &[0]), Err(StateError));
        let bass = State::new(Part::Bass);
        let mut v4 = vec![0; 25];
        v4[..6].copy_from_slice(b"MKS7\x04\x02");
        assert_eq!(bass.load(&v4, &[0]), Err(StateError));
        let chord = State::new(Part::Chord);
        let mut chord_v3 = vec![0; 26];
        chord_v3[..6].copy_from_slice(b"MKS7\x03\x03");
        chord_v3[25] = 1;
        chord.load(&chord_v3, &[0]).unwrap();
        assert_eq!(chord.get_value(WHOLE_MODE_PARAMETER_ID), Some(1.0));
    }
}

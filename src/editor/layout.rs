//! Panel geometry in design units (the 100 % zoom size in logical pixels).
//!
//! The section order follows the classic six-voice panel arrangement
//! (LFO, DCO, HPF, VCF, VCA, ENV, CHORUS), adapted to the MKS-7 parameter set.

use crate::model::*;
use egui::{Pos2, Rect, Vec2, pos2, vec2};

pub const ZOOMS: [f32; UI_ZOOM_STEPS] = [0.75, 1.0, 1.25, 1.5];

pub const MARGIN: f32 = 12.0;
pub const HEADER_TOP: f32 = 12.0;
pub const HEADER_BOTTOM: f32 = 32.0;
pub const GROUP_Y: f32 = 41.0;
pub const LABEL_Y: f32 = 53.0;
pub const CELL_TOP: f32 = 62.0;
pub const CELL_BOTTOM: f32 = 212.0;
pub const UPPER_BOTTOM: f32 = 224.0;
pub const CAPTION_TOP: f32 = 238.0;
pub const CAPTION_BOTTOM: f32 = 250.0;
pub const LOWER_TOP: f32 = 256.0;
pub const LOWER_BOTTOM: f32 = 300.0;
pub const HEIGHT: f32 = 312.0;

const FADER_W: f32 = 32.0;
const BUTTON_W: f32 = 32.0;
const SWITCH_W: f32 = 36.0;
const SECTION_PAD: f32 = 10.0;
const LOWER_GAP: f32 = 14.0;
const PLATE_MIN_W: f32 = 150.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tint {
    Cream,
    Yellow,
    Orange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Glyph {
    Text(&'static str),
    Pulse,
    Saw,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Vertical fader with `max + 1` positions.
    Fader { max: u8 },
    /// Latching on/off button with an LED.
    Toggle(Tint),
    /// One button of a radio group: lit while the parameter equals the value.
    Choice(u8, Tint),
    /// Two-position slide switch: value 0 is the upper position.
    Switch(&'static str, &'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Control {
    pub id: u32,
    pub label: Glyph,
    /// Name shown in the value readout.
    pub name: &'static str,
    pub kind: Kind,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub title: &'static str,
    pub rect: Rect,
    /// Caption spanning several controls: text, left x, right x.
    pub group: Option<(&'static str, f32, f32)>,
}

#[derive(Clone, Debug)]
pub struct Caption {
    pub text: &'static str,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub struct Panel {
    pub part: Part,
    pub size: Vec2,
    pub sections: Vec<Section>,
    pub controls: Vec<Control>,
    pub captions: Vec<Caption>,
    pub channel_down: Rect,
    pub channel_display: Rect,
    pub channel_up: Rect,
    pub output: Rect,
    pub readout: Rect,
    pub zoom: Rect,
    pub plate: Rect,
}

struct Spec {
    id: u32,
    label: Glyph,
    name: &'static str,
    kind: Kind,
}

const fn fader(id: u32, label: &'static str, name: &'static str) -> Spec {
    Spec {
        id,
        label: Glyph::Text(label),
        name,
        kind: Kind::Fader { max: 127 },
    }
}

const fn toggle(id: u32, label: Glyph, name: &'static str, tint: Tint) -> Spec {
    Spec {
        id,
        label,
        name,
        kind: Kind::Toggle(tint),
    }
}

const fn choice(id: u32, value: u8, label: Glyph, name: &'static str, tint: Tint) -> Spec {
    Spec {
        id,
        label,
        name,
        kind: Kind::Choice(value, tint),
    }
}

const fn switch(
    id: u32,
    label: &'static str,
    name: &'static str,
    top: &'static str,
    bottom: &'static str,
) -> Spec {
    Spec {
        id,
        label: Glyph::Text(label),
        name,
        kind: Kind::Switch(top, bottom),
    }
}

struct SectionSpec {
    title: &'static str,
    controls: Vec<Spec>,
    /// Caption over controls `start..start + len`.
    group: Option<(&'static str, usize, usize)>,
}

fn specs(part: Part) -> Vec<SectionSpec> {
    use Glyph::Text;
    if part.is_bass() {
        return vec![
            SectionSpec {
                title: "DCO",
                controls: vec![
                    choice(
                        BASS_WAVEFORM_PARAMETER_ID,
                        0,
                        Glyph::Pulse,
                        "WAVEFORM",
                        Tint::Cream,
                    ),
                    choice(
                        BASS_WAVEFORM_PARAMETER_ID,
                        1,
                        Glyph::Saw,
                        "WAVEFORM",
                        Tint::Cream,
                    ),
                    fader(0, "PWM", "DCO PWM"),
                ],
                group: Some(("WAVE", 0, 2)),
            },
            SectionSpec {
                title: "VCF",
                controls: vec![
                    fader(1, "FREQ", "VCF FREQ"),
                    fader(2, "RES", "VCF RES"),
                    fader(3, "ENV", "VCF ENV"),
                    fader(4, "KYBD", "VCF KYBD"),
                ],
                group: None,
            },
            SectionSpec {
                title: "VCA",
                controls: vec![fader(5, "LEVEL", "VCA LEVEL")],
                group: None,
            },
            SectionSpec {
                title: "ENV",
                controls: vec![
                    fader(6, "A", "ATTACK"),
                    fader(7, "D", "DECAY"),
                    fader(8, "S", "SUSTAIN"),
                    fader(9, "R", "RELEASE"),
                ],
                group: None,
            },
        ];
    }

    let mut dco = vec![
        choice(RANGE_PARAMETER_ID, 0, Text("16'"), "RANGE", Tint::Cream),
        choice(RANGE_PARAMETER_ID, 1, Text("8'"), "RANGE", Tint::Cream),
        choice(RANGE_PARAMETER_ID, 2, Text("4'"), "RANGE", Tint::Cream),
        fader(2, "LFO", "DCO LFO"),
        fader(3, "PWM", "DCO PWM"),
        switch(PWM_SOURCE_PARAMETER_ID, "", "PWM SOURCE", "LFO", "MAN"),
        toggle(PULSE_PARAMETER_ID, Glyph::Pulse, "PULSE", Tint::Cream),
        toggle(SAW_PARAMETER_ID, Glyph::Saw, "SAW", Tint::Cream),
        Spec {
            id: SUB_LEVEL_PARAMETER_ID,
            label: Text("SUB"),
            name: "SUB LEVEL",
            kind: Kind::Fader { max: 3 },
        },
    ];
    if part.is_melody() {
        dco.push(toggle(
            NOISE_PARAMETER_ID,
            Text("NOISE"),
            "NOISE",
            Tint::Cream,
        ));
    }
    vec![
        SectionSpec {
            title: "LFO",
            controls: vec![fader(0, "RATE", "LFO RATE"), fader(1, "DELAY", "LFO DELAY")],
            group: None,
        },
        SectionSpec {
            title: "DCO",
            controls: dco,
            group: Some(("RANGE", 0, 3)),
        },
        SectionSpec {
            title: "HPF",
            controls: vec![
                choice(HPF_PARAMETER_ID, 0, Text("OFF"), "HPF", Tint::Cream),
                choice(HPF_PARAMETER_ID, 1, Text("ON"), "HPF", Tint::Yellow),
            ],
            group: None,
        },
        SectionSpec {
            title: "VCF",
            controls: vec![
                fader(4, "FREQ", "VCF FREQ"),
                fader(5, "RES", "VCF RES"),
                switch(ENV_POLARITY_PARAMETER_ID, "", "ENV POLARITY", "POS", "NEG"),
                fader(6, "ENV", "VCF ENV"),
                fader(7, "LFO", "VCF LFO"),
                fader(8, "KYBD", "VCF KYBD"),
                toggle(
                    VCF_DYNAMICS_PARAMETER_ID,
                    Text("DYN"),
                    "VCF DYNAMICS",
                    Tint::Yellow,
                ),
            ],
            group: None,
        },
        SectionSpec {
            title: "VCA",
            controls: vec![
                switch(VCA_MODE_PARAMETER_ID, "", "VCA MODE", "ENV", "GATE"),
                fader(9, "LEVEL", "VCA LEVEL"),
                toggle(
                    VCA_DYNAMICS_PARAMETER_ID,
                    Text("DYN"),
                    "VCA DYNAMICS",
                    Tint::Yellow,
                ),
            ],
            group: None,
        },
        SectionSpec {
            title: "ENV",
            controls: vec![
                fader(10, "A", "ATTACK"),
                fader(11, "D", "DECAY"),
                fader(12, "S", "SUSTAIN"),
                fader(13, "R", "RELEASE"),
            ],
            group: None,
        },
        SectionSpec {
            title: "CHORUS",
            controls: vec![
                choice(CHORUS_PARAMETER_ID, 0, Text("OFF"), "CHORUS", Tint::Cream),
                choice(CHORUS_PARAMETER_ID, 1, Text("ON"), "CHORUS", Tint::Orange),
            ],
            group: None,
        },
    ]
}

fn cell_width(kind: Kind) -> f32 {
    match kind {
        Kind::Fader { .. } => FADER_W,
        Kind::Toggle(_) | Kind::Choice(..) => BUTTON_W,
        Kind::Switch(..) => SWITCH_W,
    }
}

impl Panel {
    pub fn new(part: Part) -> Self {
        let specs = specs(part);
        let natural: Vec<f32> = specs
            .iter()
            .map(|section| {
                section
                    .controls
                    .iter()
                    .map(|c| cell_width(c.kind))
                    .sum::<f32>()
                    + 2.0 * SECTION_PAD
            })
            .collect();
        let natural_total: f32 = natural.iter().sum();

        // The lower strip sets a minimum width (it matters for the Bass panel).
        let whole_w = if part.supports_whole_mode() {
            2.0 * BUTTON_W + LOWER_GAP
        } else {
            0.0
        };
        let (channel_w, output_w, readout_w, zoom_w) = (104.0, 170.0, 160.0, 48.0);
        let lower_min =
            channel_w + output_w + whole_w + readout_w + zoom_w + 4.0 * LOWER_GAP + PLATE_MIN_W;
        let inner = natural_total.max(lower_min);
        let stretch = inner / natural_total;
        let size = vec2((inner + 2.0 * MARGIN).round(), HEIGHT);

        let mut sections = Vec::new();
        let mut controls = Vec::new();
        let mut x = MARGIN;
        for (spec, natural) in specs.into_iter().zip(natural) {
            let width = natural * stretch;
            let cells: f32 = spec.controls.iter().map(|c| cell_width(c.kind)).sum();
            let mut cx = x + (width - cells) / 2.0;
            let first = controls.len();
            for c in spec.controls {
                let w = cell_width(c.kind);
                controls.push(Control {
                    id: c.id,
                    label: c.label,
                    name: c.name,
                    kind: c.kind,
                    rect: Rect::from_min_max(pos2(cx, CELL_TOP), pos2(cx + w, CELL_BOTTOM)),
                });
                cx += w;
            }
            let group = spec.group.map(|(text, start, len)| {
                (
                    text,
                    controls[first + start].rect.left() + 3.0,
                    controls[first + start + len - 1].rect.right() - 3.0,
                )
            });
            sections.push(Section {
                title: spec.title,
                rect: Rect::from_min_max(pos2(x, HEADER_TOP), pos2(x + width, UPPER_BOTTOM)),
                group,
            });
            x += width;
        }

        let lower = |x: f32, w: f32| -> Rect {
            Rect::from_min_max(pos2(x, LOWER_TOP), pos2(x + w, LOWER_BOTTOM))
        };
        let caption = |text: &'static str, x: f32, w: f32| Caption {
            text,
            rect: Rect::from_min_max(pos2(x, CAPTION_TOP), pos2(x + w, CAPTION_BOTTOM)),
        };
        let mut captions = Vec::new();
        let mut x = MARGIN;
        captions.push(caption("MIDI CH", x, channel_w));
        let channel_down = lower(x, 26.0);
        let channel_display = lower(x + 30.0, channel_w - 60.0);
        let channel_up = lower(x + channel_w - 26.0, 26.0);
        x += channel_w + LOWER_GAP;
        captions.push(caption("MIDI OUT", x, output_w));
        let output = lower(x, output_w);
        x += output_w + LOWER_GAP;
        if part.supports_whole_mode() {
            captions.push(caption("VOICES", x, 2.0 * BUTTON_W));
            for (value, label) in [(0, "4"), (1, "6")] {
                controls.push(Control {
                    id: WHOLE_MODE_PARAMETER_ID,
                    label: Glyph::Text(label),
                    name: "WHOLE MODE",
                    kind: Kind::Choice(value, Tint::Cream),
                    rect: lower(x + value as f32 * BUTTON_W, BUTTON_W),
                });
            }
            x += 2.0 * BUTTON_W + LOWER_GAP;
        }
        captions.push(caption("VALUE", x, readout_w));
        let readout = lower(x, readout_w);
        x += readout_w + LOWER_GAP;
        captions.push(caption("ZOOM", x, zoom_w));
        let zoom = lower(x, zoom_w);
        x += zoom_w + LOWER_GAP;
        let plate = Rect::from_min_max(pos2(x, CAPTION_TOP), pos2(size.x - MARGIN, LOWER_BOTTOM));

        Self {
            part,
            size,
            sections,
            controls,
            captions,
            channel_down,
            channel_display,
            channel_up,
            output,
            readout,
            zoom,
            plate,
        }
    }

    /// Window size in logical pixels at the given zoom step.
    pub fn window_size(part: Part, zoom_index: u8) -> (u32, u32) {
        let size = Self::new(part).size * zoom(zoom_index);
        (size.x.round() as u32, size.y.round() as u32)
    }
}

pub fn zoom(index: u8) -> f32 {
    ZOOMS[(index as usize).min(ZOOMS.len() - 1)]
}

/// Maps a design-space point to screen space.
pub fn to_screen(origin: Pos2, zoom: f32, point: Pos2) -> Pos2 {
    origin + point.to_vec2() * zoom
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn expected_ids(part: Part) -> BTreeSet<u32> {
        let mut ids: BTreeSet<u32> = (0..part.parameters().len() as u32).collect();
        ids.extend([CHANNEL_PARAMETER_ID, MIDI_OUTPUT_PARAMETER_ID]);
        if part.supports_whole_mode() {
            ids.insert(WHOLE_MODE_PARAMETER_ID);
        }
        if part.is_bass() {
            ids.insert(BASS_WAVEFORM_PARAMETER_ID);
        } else {
            ids.extend(RANGE_PARAMETER_ID..=HPF_PARAMETER_ID);
        }
        if part.is_melody() {
            ids.insert(NOISE_PARAMETER_ID);
        }
        ids
    }

    #[test]
    fn every_parameter_has_exactly_one_control() {
        for part in [Part::Melody, Part::Chord, Part::Bass] {
            let panel = Panel::new(part);
            let mut ids = BTreeSet::from([CHANNEL_PARAMETER_ID, MIDI_OUTPUT_PARAMETER_ID]);
            let mut choices = BTreeSet::new();
            for control in &panel.controls {
                match control.kind {
                    Kind::Choice(value, _) => {
                        assert!(choices.insert((control.id, value)), "{part:?} {control:?}");
                        ids.insert(control.id);
                    }
                    _ => assert!(ids.insert(control.id), "{part:?} duplicate {control:?}"),
                }
            }
            assert_eq!(ids, expected_ids(part), "{part:?}");
            assert_eq!(ids.len(), part.parameter_count(), "{part:?}");
        }
    }

    #[test]
    fn rects_stay_inside_the_panel_and_do_not_overlap() {
        for part in [Part::Melody, Part::Chord, Part::Bass] {
            let panel = Panel::new(part);
            let bounds = Rect::from_min_size(Pos2::ZERO, panel.size);
            let mut rects: Vec<Rect> = panel.controls.iter().map(|c| c.rect).collect();
            rects.extend([
                panel.channel_down,
                panel.channel_display,
                panel.channel_up,
                panel.output,
                panel.readout,
                panel.zoom,
            ]);
            for (index, rect) in rects.iter().enumerate() {
                assert!(rect.is_positive(), "{part:?} {rect:?}");
                assert!(bounds.contains_rect(*rect), "{part:?} {rect:?}");
                for other in &rects[index + 1..] {
                    assert!(
                        !rect.shrink(0.01).intersects(other.shrink(0.01)),
                        "{part:?} {rect:?} overlaps {other:?}"
                    );
                }
            }
            assert!(panel.plate.width() >= PLATE_MIN_W - 1.0, "{part:?}");
            assert!(bounds.contains_rect(panel.plate), "{part:?}");
        }
    }

    #[test]
    fn fader_ranges_and_choice_values_match_the_parameters() {
        for part in [Part::Melody, Part::Chord, Part::Bass] {
            for control in Panel::new(part).controls {
                match control.kind {
                    Kind::Fader { max } => {
                        let sub = control.id == SUB_LEVEL_PARAMETER_ID && part.has_sub_level();
                        assert_eq!(max, if sub { 3 } else { 127 }, "{part:?} {control:?}");
                    }
                    Kind::Choice(value, _) => {
                        let max = if control.id == RANGE_PARAMETER_ID {
                            2
                        } else {
                            1
                        };
                        assert!(value <= max, "{part:?} {control:?}");
                    }
                    Kind::Toggle(_) | Kind::Switch(..) => {}
                }
            }
        }
    }

    #[test]
    fn window_size_scales_with_zoom() {
        let (w, h) = Panel::window_size(Part::Melody, 1);
        assert_eq!(h, HEIGHT as u32);
        assert_eq!(Panel::window_size(Part::Melody, 3), (w * 3 / 2, h * 3 / 2));
        assert!(Panel::window_size(Part::Bass, 1).0 < w);
    }
}

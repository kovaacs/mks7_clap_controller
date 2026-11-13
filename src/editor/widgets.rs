//! Painters for the panel controls. All coordinates are design units; the
//! [`Canvas`] maps them to the screen at the current zoom.

use super::layout::{Glyph, Tint, to_screen};
use super::theme;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

pub struct Canvas<'a> {
    pub painter: &'a Painter,
    pub origin: Pos2,
    pub zoom: f32,
    /// False when no system font could be loaded: text is then skipped.
    pub text_ok: bool,
}

impl Canvas<'_> {
    pub fn pos(&self, point: Pos2) -> Pos2 {
        to_screen(self.origin, self.zoom, point)
    }

    pub fn rect(&self, rect: Rect) -> Rect {
        Rect::from_min_max(self.pos(rect.min), self.pos(rect.max))
    }

    pub fn fill(&self, rect: Rect, radius: f32, color: Color32) {
        self.painter
            .rect_filled(self.rect(rect), radius * self.zoom, color);
    }

    pub fn outline(&self, rect: Rect, radius: f32, width: f32, color: Color32) {
        self.painter.rect_stroke(
            self.rect(rect),
            radius * self.zoom,
            Stroke::new(width * self.zoom, color),
            StrokeKind::Inside,
        );
    }

    pub fn line(&self, from: Pos2, to: Pos2, width: f32, color: Color32) {
        self.painter.line_segment(
            [self.pos(from), self.pos(to)],
            Stroke::new(width * self.zoom, color),
        );
    }

    pub fn dot(&self, center: Pos2, radius: f32, color: Color32) {
        self.painter
            .circle_filled(self.pos(center), radius * self.zoom, color);
    }

    pub fn text(&self, point: Pos2, align: Align2, text: &str, size: f32, color: Color32) {
        if self.text_ok && !text.is_empty() {
            self.painter.text(
                self.pos(point),
                align,
                text,
                FontId::proportional(size * self.zoom),
                color,
            );
        }
    }

    fn text_width(&self, text: &str, size: f32) -> f32 {
        self.painter
            .layout_no_wrap(
                text.to_owned(),
                FontId::proportional(size * self.zoom),
                Color32::WHITE,
            )
            .size()
            .x
            / self.zoom
    }

    /// Returns `text` shortened with an ellipsis so it fits `width` design units.
    pub fn fit(&self, text: &str, size: f32, width: f32) -> String {
        if !self.text_ok || self.text_width(text, size) <= width {
            return text.to_owned();
        }
        // Binary search for the longest prefix that fits with an ellipsis.
        let ends: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
        let candidate = |count: usize| format!("{}...", &text[..ends[count]]);
        let (mut fits, mut too_long) = (0, ends.len());
        if self.text_width(&candidate(0), size) > width {
            return String::new();
        }
        while too_long - fits > 1 {
            let middle = (fits + too_long) / 2;
            if self.text_width(&candidate(middle), size) <= width {
                fits = middle;
            } else {
                too_long = middle;
            }
        }
        candidate(fits)
    }
}

/// Vertical extent of a fader's travel inside its cell (top = maximum).
pub fn fader_travel(cell: Rect) -> (f32, f32) {
    (cell.top() + 14.0, cell.bottom() - 14.0)
}

/// `position` and `ghost` are 0..=1; `steps` is the number of positions minus one.
pub fn fader(c: &Canvas, cell: Rect, position: f32, ghost: Option<f32>, steps: u8, active: bool) {
    let (top, bottom) = fader_travel(cell);
    let x = cell.center().x;
    let y_at = |t: f32| bottom - t.clamp(0.0, 1.0) * (bottom - top);

    let ticks = if steps < 10 { steps as usize } else { 10 };
    for tick in 0..=ticks {
        let y = y_at(tick as f32 / ticks as f32);
        let long = tick == 0 || tick == ticks || (ticks == 10 && tick == 5);
        let (inner, outer) = (6.0, if long { 12.0 } else { 10.0 });
        let color = if long { theme::PRINT } else { theme::TICK };
        c.line(pos2(x - outer, y), pos2(x - inner, y), 1.0, color);
        c.line(pos2(x + inner, y), pos2(x + outer, y), 1.0, color);
    }

    let slot = Rect::from_min_max(pos2(x - 2.5, top - 8.0), pos2(x + 2.5, bottom + 8.0));
    c.fill(slot, 1.5, theme::SLOT);
    c.outline(slot.expand(0.5), 2.0, 0.5, theme::PANEL_LINE);

    if let Some(ghost) = ghost {
        let y = y_at(ghost);
        c.line(pos2(x - 9.0, y), pos2(x + 9.0, y), 2.0, theme::GHOST);
    }

    let y = y_at(position);
    let cap = Rect::from_center_size(pos2(x, y), vec2(18.0, 12.0));
    c.fill(cap.translate(vec2(0.0, 1.5)), 1.5, theme::SHADOW);
    c.fill(cap, 1.5, theme::CAP);
    c.outline(
        cap,
        1.5,
        0.75,
        if active {
            theme::PRINT
        } else {
            theme::CAP_EDGE
        },
    );
    c.line(
        pos2(cap.left() + 2.0, y),
        pos2(cap.right() - 2.0, y),
        1.5,
        theme::CAP_LINE,
    );
}

/// The clickable body and LED centre of a button inside its cell.
pub fn button_parts(cell: Rect) -> (Rect, Pos2) {
    let x = cell.center().x;
    if cell.height() > 80.0 {
        let body = Rect::from_min_max(
            pos2(x - 12.0, cell.top() + 58.0),
            pos2(x + 12.0, cell.top() + 96.0),
        );
        (body, pos2(x, cell.top() + 46.0))
    } else {
        let body = Rect::from_min_max(
            pos2(x - 12.0, cell.top() + 12.0),
            pos2(x + 12.0, cell.bottom()),
        );
        (body, pos2(x, cell.top() + 4.5))
    }
}

pub fn led(c: &Canvas, center: Pos2, lit: bool) {
    if lit {
        c.dot(center, 5.0, Color32::from_rgba_premultiplied(90, 12, 8, 60));
    }
    c.dot(center, 3.2, theme::PANEL_EDGE);
    c.dot(
        center,
        2.5,
        if lit { theme::LED_ON } else { theme::LED_OFF },
    );
}

pub fn button(c: &Canvas, cell: Rect, tint: Tint, lit: bool, pressed: bool) {
    let (body, led_center) = button_parts(cell);
    led(c, led_center, lit);
    key(c, body, theme::button(tint), pressed);
}

/// A plastic key cap, used for panel buttons and the small lower-strip keys.
pub fn key(c: &Canvas, body: Rect, color: Color32, pressed: bool) {
    let face = if pressed {
        theme::shade(color, 0.18)
    } else {
        color
    };
    if !pressed {
        c.fill(body.translate(vec2(0.0, 2.0)), 2.0, theme::SHADOW);
    }
    c.fill(body, 2.0, face);
    c.outline(body, 2.0, 0.75, theme::shade(color, 0.45));
    let y = body.top() + 1.5;
    c.line(
        pos2(body.left() + 2.5, y),
        pos2(body.right() - 2.5, y),
        1.0,
        if pressed {
            theme::shade(color, 0.3)
        } else {
            Color32::from_rgba_premultiplied(255, 255, 255, 150)
        },
    );
}

/// Two-position slide switch; `down` selects the lower position.
pub fn switch(c: &Canvas, cell: Rect, top: &str, bottom: &str, down: bool) {
    let x = cell.center().x;
    let slot = Rect::from_min_max(
        pos2(x - 6.0, cell.top() + 56.0),
        pos2(x + 6.0, cell.top() + 96.0),
    );
    let label = |y: f32, text: &str, on: bool| {
        c.text(
            pos2(x, y),
            Align2::CENTER_CENTER,
            text,
            8.0,
            if on { theme::PRINT } else { theme::PRINT_DIM },
        );
    };
    label(slot.top() - 9.0, top, !down);
    label(slot.bottom() + 9.0, bottom, down);
    c.fill(slot, 2.0, theme::SLOT);
    c.outline(slot.expand(0.5), 2.5, 0.5, theme::PANEL_LINE);
    let knob_y = if down {
        slot.bottom() - 10.0
    } else {
        slot.top() + 10.0
    };
    let knob = Rect::from_center_size(pos2(x, knob_y), vec2(9.0, 16.0));
    c.fill(knob, 1.5, theme::SWITCH_KNOB);
    for offset in [-3.0, 0.0, 3.0] {
        c.line(
            pos2(knob.left() + 1.5, knob_y + offset),
            pos2(knob.right() - 1.5, knob_y + offset),
            0.75,
            theme::shade(theme::SWITCH_KNOB, 0.4),
        );
    }
}

pub fn glyph(c: &Canvas, center: Pos2, glyph: Glyph, color: Color32) {
    let (x, y) = (center.x, center.y);
    match glyph {
        Glyph::Text(text) => c.text(center, Align2::CENTER_CENTER, text, 8.5, color),
        Glyph::Pulse => {
            let points = [
                pos2(x - 7.0, y + 3.0),
                pos2(x - 4.0, y + 3.0),
                pos2(x - 4.0, y - 3.0),
                pos2(x + 1.0, y - 3.0),
                pos2(x + 1.0, y + 3.0),
                pos2(x + 7.0, y + 3.0),
            ];
            for pair in points.windows(2) {
                c.line(pair[0], pair[1], 1.2, color);
            }
        }
        Glyph::Saw => {
            let points = [
                pos2(x - 7.0, y + 3.0),
                pos2(x, y - 3.0),
                pos2(x, y + 3.0),
                pos2(x + 7.0, y - 3.0),
                pos2(x + 7.0, y + 3.0),
            ];
            for pair in points.windows(2) {
                c.line(pair[0], pair[1], 1.2, color);
            }
        }
    }
}

/// A recessed dark window for LED-style text.
pub fn display(c: &Canvas, rect: Rect) {
    c.fill(rect, 2.0, theme::DISPLAY);
    c.outline(rect, 2.0, 1.0, theme::PANEL_EDGE);
}

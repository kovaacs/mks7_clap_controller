//! Colours of the panel. Everything is drawn from these; there are no image assets.

use super::layout::Tint;
use egui::Color32;

pub const PANEL: Color32 = Color32::from_rgb(0x2a, 0x2b, 0x2f);
pub const PANEL_EDGE: Color32 = Color32::from_rgb(0x17, 0x18, 0x1a);
pub const PANEL_LINE: Color32 = Color32::from_rgb(0x3d, 0x3f, 0x45);
pub const HEADER: Color32 = Color32::from_rgb(0xc4, 0x35, 0x2b);
pub const ACCENT: Color32 = Color32::from_rgb(0x2c, 0x6c, 0xb4);
pub const PRINT: Color32 = Color32::from_rgb(0xe2, 0xe2, 0xdc);
pub const PRINT_DIM: Color32 = Color32::from_rgb(0x9a, 0x9b, 0x9d);
pub const SLOT: Color32 = Color32::from_rgb(0x08, 0x08, 0x09);
pub const TICK: Color32 = Color32::from_rgb(0x8c, 0x8d, 0x90);
pub const CAP: Color32 = Color32::from_rgb(0x1b, 0x1c, 0x1f);
pub const CAP_EDGE: Color32 = Color32::from_rgb(0x55, 0x57, 0x5d);
pub const CAP_LINE: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xee);
pub const GHOST: Color32 = Color32::from_rgb(0xff, 0x8a, 0x3c);
pub const LED_ON: Color32 = Color32::from_rgb(0xff, 0x3a, 0x2e);
pub const LED_OFF: Color32 = Color32::from_rgb(0x4a, 0x16, 0x13);
pub const DISPLAY: Color32 = Color32::from_rgb(0x12, 0x06, 0x06);
pub const DISPLAY_TEXT: Color32 = Color32::from_rgb(0xff, 0x45, 0x36);
pub const SWITCH_KNOB: Color32 = Color32::from_rgb(0xc9, 0xca, 0xcc);
pub const SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 90);
pub const POPUP: Color32 = Color32::from_rgb(0x1c, 0x1d, 0x20);
pub const POPUP_HOVER: Color32 = Color32::from_rgb(0x3a, 0x3c, 0x42);

pub const fn button(tint: Tint) -> Color32 {
    match tint {
        Tint::Cream => Color32::from_rgb(0xe6, 0xe2, 0xd2),
        Tint::Yellow => Color32::from_rgb(0xe8, 0xbd, 0x3f),
        Tint::Orange => Color32::from_rgb(0xe2, 0x7d, 0x2e),
    }
}

/// Darkens a colour; `amount` 0 leaves it unchanged, 1 gives black.
pub fn shade(color: Color32, amount: f32) -> Color32 {
    let k = 1.0 - amount.clamp(0.0, 1.0);
    Color32::from_rgb(
        (color.r() as f32 * k) as u8,
        (color.g() as f32 * k) as u8,
        (color.b() as f32 * k) as u8,
    )
}

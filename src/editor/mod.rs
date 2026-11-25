//! The plug-in's front panel: an egui window embedded in the host's view.

pub(crate) mod layout;
mod theme;
mod widgets;

use crate::edits::EditQueue;
use crate::midi::MidiBackend;
use crate::model::*;
use crate::{Mks7MainThread, Mks7Shared, display_text};
use baseview::WindowHandle;
use baseview::dpi::LogicalSize;
use clack_extensions::gui::{
    GuiApiType, GuiConfiguration, GuiSize, HostGui, PluginGuiImpl, Window,
};
use clack_extensions::params::HostParams;
use clack_plugin::prelude::*;
use egui::{Align2, Rect, Response, Sense, Ui, ViewportCommand, pos2, vec2};
use egui_baseview::{EguiWindow, EguiWindowSettings, KeyCapture};
use layout::{Control, Kind, Panel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use widgets::Canvas;

const SYSTEM_FONTS: [&str; 3] = [
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Geneva.ttf",
];
const POPUP_ROW: f32 = 18.0;

/// Host callbacks usable from the window's frame callback.
#[derive(Clone)]
pub struct HostLink {
    host: HostSharedHandle<'static>,
    params: Option<HostParams>,
    gui: Option<HostGui>,
    /// Cleared before the plug-in instance goes away; no host call happens after that.
    alive: Arc<AtomicBool>,
}

impl HostLink {
    fn request_flush(&self) {
        if self.alive.load(Ordering::Relaxed)
            && let Some(params) = &self.params
        {
            params.request_flush(&self.host);
        }
    }

    /// Whether the host accepted the new size.
    fn request_resize(&self, width: u32, height: u32) -> bool {
        self.alive.load(Ordering::Relaxed)
            && self
                .gui
                .is_some_and(|gui| gui.request_resize(&self.host, width, height).is_ok())
    }
}

pub struct App {
    state: Arc<State>,
    midi: Arc<MidiBackend>,
    edits: Arc<EditQueue>,
    host: Option<HostLink>,
    panel: Arc<Panel>,
    /// The fader being dragged and its unrounded value.
    held: Option<(u32, f64)>,
    /// Name and id of the control shown in the value readout.
    readout: Option<(&'static str, u32)>,
    picker_open: bool,
    picker_scroll: usize,
    applied_zoom: u8,
    snapshot: Vec<f64>,
    text_ok: bool,
    /// Trackpad scrolling not yet turned into whole steps, in points, and the
    /// control it was collected over.
    scroll_rest: f32,
    scroll_target: u32,
}

impl Drop for App {
    fn drop(&mut self) {
        // The window can close in the middle of a drag; never leave the host's gesture open.
        if let Some((id, _)) = self.held.take() {
            self.edits.set_gesture(id, false);
            self.flush();
        }
    }
}

/// Points of trackpad scrolling per step; a mouse wheel notch is always one step.
const SCROLL_POINTS_PER_STEP: f32 = 24.0;

/// Whole scroll steps this frame: positive means up (towards larger values).
fn scroll_steps(ui: &Ui, rest: &mut f32) -> i32 {
    let (lines, points) = ui.input(|input| {
        let mut lines = 0.0;
        let mut points = 0.0;
        for event in &input.events {
            if let egui::Event::MouseWheel { unit, delta, .. } = event
                && delta.y != 0.0
            {
                match unit {
                    egui::MouseWheelUnit::Point => points += delta.y,
                    egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                        lines += delta.y.signum()
                    }
                }
            }
        }
        (lines, points)
    });
    *rest += points;
    let from_points = (*rest / SCROLL_POINTS_PER_STEP).trunc();
    *rest -= from_points * SCROLL_POINTS_PER_STEP;
    (lines + from_points) as i32
}

impl App {
    fn new(
        state: Arc<State>,
        midi: Arc<MidiBackend>,
        edits: Arc<EditQueue>,
        host: Option<HostLink>,
    ) -> Self {
        Self {
            panel: Arc::new(Panel::new(state.part)),
            applied_zoom: state.ui_zoom(),
            state,
            midi,
            edits,
            host,
            held: None,
            readout: None,
            picker_open: false,
            picker_scroll: 0,
            snapshot: Vec::new(),
            text_ok: false,
            scroll_rest: 0.0,
            scroll_target: u32::MAX,
        }
    }

    fn value(&self, id: u32) -> f64 {
        match self.held {
            Some((held, value)) if held == id => value.round(),
            _ => self.state.get_value(id).unwrap_or(0.0),
        }
    }

    /// Applies a change made on the panel: the hardware gets it through the
    /// state's dirty flags, the host through the edit queue.
    fn set(&self, id: u32, value: f64) {
        self.state.apply_value(id, value, self.midi.count());
        if let Some(applied) = self.state.get_value(id) {
            self.edits.set_value(id, applied);
        }
        self.flush();
    }

    fn flush(&self) {
        if let Some(host) = &self.host {
            host.request_flush();
        }
    }

    /// Scroll steps over `target`; leftover trackpad scrolling does not carry over
    /// from one control to another.
    fn scroll_steps(&mut self, ui: &Ui, target: u32) -> i32 {
        if self.scroll_target != target {
            self.scroll_target = target;
            self.scroll_rest = 0.0;
        }
        scroll_steps(ui, &mut self.scroll_rest)
    }

    fn sense(
        &self,
        ui: &Ui,
        rect: Rect,
        key: impl std::hash::Hash + std::fmt::Debug,
        sense: Sense,
    ) -> Option<Response> {
        (!self.picker_open).then(|| ui.interact(rect, ui.id().with(key), sense))
    }
}

fn install_fonts(ctx: &egui::Context) -> bool {
    for path in SYSTEM_FONTS {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::empty();
        fonts.font_data.insert(
            "system".to_owned(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.insert(family, vec!["system".to_owned()]);
        }
        ctx.set_fonts(fonts);
        return true;
    }
    false
}

fn frame(ui: &mut Ui, app: &mut App) {
    let zoom_index = app.state.ui_zoom();
    if zoom_index != app.applied_zoom {
        let (width, height) = Panel::window_size(app.state.part, zoom_index);
        // Resize only once the host agrees, or the view and the host window disagree.
        let accepted = app
            .host
            .as_ref()
            .is_none_or(|host| host.request_resize(width, height));
        if accepted {
            app.applied_zoom = zoom_index;
            ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(vec2(
                width as f32,
                height as f32,
            )));
        } else {
            app.state.set_ui_zoom(app.applied_zoom);
        }
    }

    // The frame callback runs on a timer; repaint only when something visible changed.
    let state = &app.state;
    let current = app
        .panel
        .controls
        .iter()
        .flat_map(|control| {
            [
                state.get_value(control.id).unwrap_or(0.0),
                state.modulation(control.id),
            ]
        })
        .chain([
            state.get_value(CHANNEL_PARAMETER_ID).unwrap_or(0.0),
            state.output_index() as f64,
            app.applied_zoom as f64,
        ]);
    let mut changed = false;
    let mut length = 0;
    for (index, value) in current.enumerate() {
        length = index + 1;
        match app.snapshot.get_mut(index) {
            Some(old) if *old == value => {}
            Some(old) => {
                *old = value;
                changed = true;
            }
            None => {
                app.snapshot.push(value);
                changed = true;
            }
        }
    }
    app.snapshot.truncate(length);
    if changed {
        ui.ctx().request_repaint();
    }

    draw(ui, app);
}

fn draw(ui: &mut Ui, app: &mut App) {
    let panel = Arc::clone(&app.panel);
    let painter = ui.painter().clone();
    let c = Canvas {
        painter: &painter,
        origin: ui.max_rect().min,
        zoom: layout::zoom(app.applied_zoom),
        text_ok: app.text_ok,
    };

    draw_background(&c, &panel);

    let mut hovered = None;
    for (index, control) in panel.controls.iter().enumerate() {
        let response = match control.kind {
            Kind::Fader { max } => fader(ui, &c, app, index, control, max),
            Kind::Toggle(tint) => {
                let lit = app.value(control.id) >= 0.5;
                let response = app.sense(ui, c.rect(control.rect), index, Sense::click());
                let pressed = response
                    .as_ref()
                    .is_some_and(Response::is_pointer_button_down_on);
                widgets::button(&c, control.rect, tint, lit, pressed);
                if response.as_ref().is_some_and(Response::clicked) {
                    app.set(control.id, if lit { 0.0 } else { 1.0 });
                }
                response
            }
            Kind::Choice(value, tint) => {
                let lit = app.value(control.id).round() == value as f64;
                let response = app.sense(ui, c.rect(control.rect), index, Sense::click());
                let pressed = response
                    .as_ref()
                    .is_some_and(Response::is_pointer_button_down_on);
                widgets::button(&c, control.rect, tint, lit, pressed);
                if control.rect.top() >= layout::LOWER_TOP {
                    // Lower-strip keys carry their label on the cap.
                    let (body, _) = widgets::button_parts(control.rect);
                    widgets::glyph(&c, body.center(), control.label, theme::PANEL_EDGE);
                }
                if response.as_ref().is_some_and(Response::clicked) {
                    app.set(control.id, value as f64);
                }
                response
            }
            Kind::Switch(top, bottom) => {
                let down = app.value(control.id) >= 0.5;
                widgets::switch(&c, control.rect, top, bottom, down);
                let response = app.sense(ui, c.rect(control.rect), index, Sense::click());
                if response.as_ref().is_some_and(Response::clicked) {
                    app.set(control.id, if down { 0.0 } else { 1.0 });
                }
                response
            }
        };
        if response.is_some_and(|r| r.hovered() || r.dragged()) {
            hovered = Some((control.name, control.id));
        }
    }

    // MIDI channel stepper.
    let channel = app.value(CHANNEL_PARAMETER_ID);
    for (rect, label, step) in [
        (panel.channel_down, "-", -1.0),
        (panel.channel_up, "+", 1.0),
    ] {
        let response = app.sense(ui, c.rect(rect), ("channel", label), Sense::click());
        let pressed = response
            .as_ref()
            .is_some_and(Response::is_pointer_button_down_on);
        let body = rect.shrink2(vec2(1.0, 6.0));
        widgets::key(&c, body, theme::button(layout::Tint::Cream), pressed);
        c.text(
            body.center(),
            Align2::CENTER_CENTER,
            label,
            14.0,
            theme::PANEL_EDGE,
        );
        if let Some(response) = response {
            if response.clicked() {
                app.set(CHANNEL_PARAMETER_ID, (channel + step).clamp(1.0, 16.0));
            }
            if response.hovered() {
                hovered = Some(("MIDI CHANNEL", CHANNEL_PARAMETER_ID));
            }
        }
    }
    widgets::display(&c, panel.channel_display.shrink2(vec2(0.0, 4.0)));
    c.text(
        panel.channel_display.center(),
        Align2::CENTER_CENTER,
        &format!("{:02}", channel as i64),
        20.0,
        theme::DISPLAY_TEXT,
    );

    // MIDI output picker.
    let output = panel.output.shrink2(vec2(0.0, 4.0));
    let output_response = ui.interact(c.rect(output), ui.id().with("output"), Sense::click());
    widgets::display(&c, output);
    let output_name = app.midi.name(app.state.output_index()).to_owned();
    c.text(
        pos2(output.left() + 8.0, output.center().y),
        Align2::LEFT_CENTER,
        &c.fit(&output_name, 11.0, output.width() - 28.0),
        11.0,
        theme::DISPLAY_TEXT,
    );
    let arrow = pos2(output.right() - 11.0, output.center().y);
    c.line(
        arrow + vec2(-4.0, -2.0),
        arrow + vec2(0.0, 2.5),
        1.2,
        theme::DISPLAY_TEXT,
    );
    c.line(
        arrow + vec2(0.0, 2.5),
        arrow + vec2(4.0, -2.0),
        1.2,
        theme::DISPLAY_TEXT,
    );
    let output_clicked = output_response.clicked();
    if output_clicked {
        app.picker_open = !app.picker_open;
    }
    if output_response.hovered() {
        hovered = Some(("MIDI OUT", MIDI_OUTPUT_PARAMETER_ID));
    }

    // Zoom key.
    let zoom_body = panel.zoom.shrink2(vec2(0.0, 6.0));
    let zoom_response = app.sense(ui, c.rect(zoom_body), "zoom", Sense::click());
    widgets::key(
        &c,
        zoom_body,
        theme::button(layout::Tint::Cream),
        zoom_response
            .as_ref()
            .is_some_and(Response::is_pointer_button_down_on),
    );
    c.text(
        zoom_body.center(),
        Align2::CENTER_CENTER,
        &format!("{:.0}%", c.zoom * 100.0),
        10.0,
        theme::PANEL_EDGE,
    );
    if zoom_response.is_some_and(|r| r.clicked()) {
        app.state
            .set_ui_zoom((app.applied_zoom + 1) % UI_ZOOM_STEPS as u8);
        ui.ctx().request_repaint();
    }

    // Value readout: the control under the pointer, or the last one touched.
    if hovered.is_some() {
        app.readout = hovered;
    }
    let readout = panel.readout.shrink2(vec2(0.0, 4.0));
    widgets::display(&c, readout);
    if let Some((name, id)) = app.readout {
        let value = display_text(app.state.part, &app.midi, id, app.value(id)).unwrap_or_default();
        let value = value.to_uppercase();
        let value = c.fit(&value, 13.0, readout.width() * 0.45);
        c.text(
            pos2(readout.left() + 8.0, readout.center().y),
            Align2::LEFT_CENTER,
            &c.fit(name, 9.5, readout.width() * 0.55 - 12.0),
            9.5,
            theme::DISPLAY_TEXT,
        );
        c.text(
            pos2(readout.right() - 8.0, readout.center().y),
            Align2::RIGHT_CENTER,
            &value,
            13.0,
            theme::DISPLAY_TEXT,
        );
    }

    if app.picker_open {
        picker(ui, &c, app, output, output_clicked);
    }
}

fn draw_background(c: &Canvas, panel: &Panel) {
    let full = Rect::from_min_size(pos2(0.0, 0.0), panel.size);
    c.fill(full, 0.0, theme::PANEL);
    c.outline(full, 0.0, 1.5, theme::PANEL_EDGE);

    for (index, section) in panel.sections.iter().enumerate() {
        let rect = section.rect;
        let header = Rect::from_min_max(
            rect.min + vec2(1.0, 0.0),
            pos2(rect.right() - 1.0, layout::HEADER_BOTTOM),
        );
        c.fill(header, 0.0, theme::HEADER);
        c.text(
            header.center(),
            Align2::CENTER_CENTER,
            section.title,
            12.0,
            theme::PRINT,
        );
        if index > 0 {
            c.line(
                pos2(rect.left(), layout::HEADER_BOTTOM + 6.0),
                pos2(rect.left(), layout::UPPER_BOTTOM - 8.0),
                1.0,
                theme::PANEL_LINE,
            );
        }
        if let Some((text, left, right)) = section.group {
            let y = layout::GROUP_Y;
            let middle = (left + right) / 2.0;
            let gap = 4.0 + text.len() as f32 * 2.6;
            c.line(pos2(left, y), pos2(middle - gap, y), 0.75, theme::PRINT_DIM);
            c.line(
                pos2(middle + gap, y),
                pos2(right, y),
                0.75,
                theme::PRINT_DIM,
            );
            c.line(pos2(left, y), pos2(left, y + 4.0), 0.75, theme::PRINT_DIM);
            c.line(pos2(right, y), pos2(right, y + 4.0), 0.75, theme::PRINT_DIM);
            c.text(
                pos2(middle, y),
                Align2::CENTER_CENTER,
                text,
                7.5,
                theme::PRINT_DIM,
            );
        }
    }
    let (left, right) = (layout::MARGIN + 1.0, panel.size.x - layout::MARGIN - 1.0);
    c.fill(
        Rect::from_min_max(
            pos2(left, layout::UPPER_BOTTOM),
            pos2(right, layout::UPPER_BOTTOM + 2.5),
        ),
        0.0,
        theme::HEADER,
    );

    for control in &panel.controls {
        let x = control.rect.center().x;
        if control.rect.top() < layout::UPPER_BOTTOM {
            widgets::glyph(c, pos2(x, layout::LABEL_Y), control.label, theme::PRINT);
        }
    }

    for caption in &panel.captions {
        c.fill(caption.rect, 0.0, theme::ACCENT);
        c.text(
            caption.rect.center(),
            Align2::CENTER_CENTER,
            caption.text,
            8.0,
            theme::PRINT,
        );
    }

    let plate = panel.plate;
    c.fill(
        Rect::from_min_max(plate.min, pos2(plate.right(), layout::CAPTION_BOTTOM)),
        0.0,
        theme::ACCENT,
    );
    c.text(
        pos2(plate.right(), layout::LOWER_TOP + 14.0),
        Align2::RIGHT_CENTER,
        "MKS-7",
        24.0,
        theme::PRINT,
    );
    let part = String::from_utf8_lossy(panel.part.name()).to_uppercase();
    c.text(
        pos2(plate.right(), layout::LOWER_BOTTOM - 7.0),
        Align2::RIGHT_CENTER,
        &format!("{part} CONTROLLER"),
        8.5,
        theme::PRINT_DIM,
    );
}

fn fader(
    ui: &Ui,
    c: &Canvas,
    app: &mut App,
    index: usize,
    control: &Control,
    max: u8,
) -> Option<Response> {
    let id = control.id;
    let max = max as f64;
    let (top, bottom) = widgets::fader_travel(control.rect);
    let travel = (bottom - top) as f64;
    let response = app.sense(ui, c.rect(control.rect), index, Sense::click_and_drag());

    if let Some(response) = &response {
        // Pointer position in design units.
        let design_y = |screen_y: f32| ((screen_y - c.origin.y) / c.zoom) as f64;
        let value_at =
            |screen_y: f32| ((bottom as f64 - design_y(screen_y)) / travel * max).clamp(0.0, max);
        let current = app.value(id);
        let cap_y = bottom as f64 - current / max * travel;
        // Pressing the cap keeps its value; pressing the slot jumps there.
        let off_cap = |screen_y: f32| (design_y(screen_y) - cap_y).abs() > 8.0;
        let fine = if ui.input(|input| input.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let to_value = |screen_dy: f32| -(screen_dy / c.zoom) as f64 / travel * max * fine;
        // On the first drag frame, count the movement since the press: egui only
        // reports a drag once the pointer has moved past its threshold.
        let mut moved = response.drag_delta().y;
        if response.drag_started() {
            let origin = ui.input(|input| input.pointer.press_origin());
            let start = match origin {
                Some(origin) if off_cap(origin.y) => value_at(origin.y),
                _ => current,
            };
            if let (Some(origin), Some(pointer)) = (origin, response.interact_pointer_pos()) {
                moved = pointer.y - origin.y;
            }
            app.held = Some((id, start));
            app.edits.set_gesture(id, true);
        }
        if response.dragged()
            && let Some((held, value)) = app.held
            && held == id
        {
            let delta = to_value(moved);
            let value = (value + delta).clamp(0.0, max);
            app.held = Some((id, value));
            if app.state.get_value(id) != Some(value.round()) {
                app.set(id, value.round());
            }
        }
        if response.drag_stopped() && app.held.is_some_and(|(held, _)| held == id) {
            app.held = None;
            app.edits.set_gesture(id, false);
            app.flush();
        }
        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
            && off_cap(pointer.y)
        {
            app.set(id, value_at(pointer.y).round());
        }
        if response.hovered() && app.held.is_none() {
            let steps = app.scroll_steps(ui, id);
            if steps != 0 {
                app.set(id, (app.value(id) + steps as f64).clamp(0.0, max));
            }
        }
    }

    let value = app.value(id);
    let modulation = app.state.modulation(id);
    let ghost = (modulation != 0.0).then(|| ((value + modulation) / max) as f32);
    let active = app.held.is_some_and(|(held, _)| held == id);
    widgets::fader(
        c,
        control.rect,
        (value / max) as f32,
        ghost,
        max.min(127.0) as u8,
        active,
    );
    response
}

fn picker(ui: &Ui, c: &Canvas, app: &mut App, anchor: Rect, anchor_clicked: bool) {
    let count = app.midi.count();
    let max_rows = (((anchor.top() - 12.0) / POPUP_ROW) as usize).max(1);
    let rows = count.min(max_rows);
    let max_scroll = count - rows;
    let steps = app.scroll_steps(ui, MIDI_OUTPUT_PARAMETER_ID);
    app.picker_scroll = app
        .picker_scroll
        .saturating_add_signed(-steps as isize)
        .min(max_scroll);

    let width = anchor.width().max(220.0);
    let list = Rect::from_min_max(
        pos2(
            anchor.left(),
            anchor.top() - 4.0 - rows as f32 * POPUP_ROW - 4.0,
        ),
        pos2(anchor.left() + width, anchor.top() - 4.0),
    );
    c.fill(list.translate(vec2(0.0, 2.0)), 3.0, theme::SHADOW);
    c.fill(list, 3.0, theme::POPUP);
    c.outline(list, 3.0, 1.0, theme::PANEL_LINE);

    let selected = app.state.output_index();
    let mut chosen = None;
    for row in 0..rows {
        let index = row + app.picker_scroll;
        let rect = Rect::from_min_size(
            pos2(list.left() + 2.0, list.top() + 2.0 + row as f32 * POPUP_ROW),
            vec2(width - 4.0, POPUP_ROW),
        );
        let response = ui.interact(
            c.rect(rect),
            ui.id().with(("output", index)),
            Sense::click(),
        );
        if response.hovered() {
            c.fill(rect, 2.0, theme::POPUP_HOVER);
        }
        widgets::led(
            c,
            pos2(rect.left() + 9.0, rect.center().y),
            index == selected,
        );
        c.text(
            pos2(rect.left() + 20.0, rect.center().y),
            Align2::LEFT_CENTER,
            &c.fit(app.midi.name(index), 10.5, width - 30.0),
            10.5,
            theme::PRINT,
        );
        if response.clicked() {
            chosen = Some(index);
        }
    }
    if let Some(index) = chosen {
        app.set(MIDI_OUTPUT_PARAMETER_ID, index as f64);
        app.picker_open = false;
    } else if !anchor_clicked && ui.input(|input| input.pointer.any_click()) {
        app.picker_open = false;
    }
    if !app.picker_open {
        ui.ctx().request_repaint();
    }
}

/// An open editor window. Dropping it closes the window.
pub struct Editor {
    handle: WindowHandle,
    edits: Arc<EditQueue>,
    host: HostLink,
}

impl Editor {
    fn open(parent: &Window, shared: &Mks7Shared, host: HostLink) -> Self {
        let (width, height) = Panel::window_size(shared.state.part, shared.state.ui_zoom());
        let settings = EguiWindowSettings::new()
            .with_tile("MKS-7 Controller")
            .with_size(LogicalSize::new(width as f64, height as f64));
        let app = App::new(
            Arc::clone(&shared.state),
            Arc::clone(&shared.midi),
            Arc::clone(&shared.edits),
            Some(host.clone()),
        );
        let handle = EguiWindow::open_parented(
            parent,
            settings,
            app,
            |ctx, commands, app: &mut App| {
                app.text_ok = install_fonts(ctx);
                // The panel has no text entry: leave every key to the host.
                commands.set_key_capture(KeyCapture::IgnoreAll);
            },
            |_, _, _| {},
            |ui, _commands, app: &mut App| frame(ui, app),
        );
        Self {
            handle,
            edits: Arc::clone(&shared.edits),
            host,
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.handle.close();
        // AppKit may release the view (and `App` with it) later, so do not rely on
        // `App::drop`: end any drag here and get it to the host while it is reachable.
        self.edits.end_all_gestures();
        self.host.request_flush();
        self.host.alive.store(false, Ordering::Relaxed);
    }
}

fn supported(configuration: &GuiConfiguration) -> bool {
    !configuration.is_floating
        && Some(configuration.api_type) == GuiApiType::default_for_current_platform()
}

impl PluginGuiImpl for Mks7MainThread<'_> {
    fn is_api_supported(&self, configuration: GuiConfiguration) -> bool {
        supported(&configuration)
    }

    fn get_preferred_api(&self) -> Option<GuiConfiguration<'_>> {
        Some(GuiConfiguration {
            api_type: GuiApiType::default_for_current_platform()?,
            is_floating: false,
        })
    }

    fn create(&self, configuration: GuiConfiguration) -> Result<(), PluginError> {
        if supported(&configuration) {
            Ok(())
        } else {
            Err(PluginError::Message("Unsupported GUI configuration"))
        }
    }

    fn destroy(&self) {
        self.editor.take();
    }

    fn set_scale(&self, _scale: f64) -> Result<(), PluginError> {
        // Cocoa sizes are logical; the backing scale is read from the view.
        Ok(())
    }

    fn get_size(&self) -> Option<GuiSize> {
        let state = &self.shared.state;
        let (width, height) = Panel::window_size(state.part, state.ui_zoom());
        Some(GuiSize { width, height })
    }

    fn set_size(&self, size: GuiSize) -> Result<(), PluginError> {
        if Some(size) == self.get_size() {
            Ok(())
        } else {
            Err(PluginError::Message("The editor has a fixed size"))
        }
    }

    fn set_parent(&self, window: Window) -> Result<(), PluginError> {
        // SAFETY: the handle is only used through `HostLink`, which stops calling the host
        // once `alive` is cleared. `Editor::drop` clears it, and the editor is owned by this
        // main-thread object, which the host destroys before the `clap_host` goes away.
        let host = unsafe { self.host.shared().with_arbitrary_lifetime() };
        let link = HostLink {
            host,
            params: self.host_params,
            gui: self.host_gui,
            alive: Arc::new(AtomicBool::new(true)),
        };
        // Close a previous window first: its drop ends every open gesture.
        self.editor.take();
        self.editor
            .replace(Some(Editor::open(&window, self.shared, link)));
        Ok(())
    }

    fn set_transient(&self, _window: Window) -> Result<(), PluginError> {
        Ok(())
    }

    fn show(&self) -> Result<(), PluginError> {
        Ok(())
    }

    fn hide(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(part: Part) -> App {
        App::new(
            Arc::new(State::new(part)),
            Arc::new(MidiBackend::new()),
            Arc::new(EditQueue::new()),
            None,
        )
    }

    fn run(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> usize {
        let (width, height) = Panel::window_size(app.state.part, app.applied_zoom);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(width as f32, height as f32),
            )),
            events,
            ..Default::default()
        };
        let output = ctx.run_ui(input, |ui| frame(ui, app));
        ctx.tessellate(output.shapes, output.pixels_per_point).len()
    }

    #[test]
    fn every_panel_draws_at_every_zoom_with_the_system_font() {
        for part in [Part::Melody, Part::Chord, Part::Bass] {
            for zoom in 0..UI_ZOOM_STEPS as u8 {
                let ctx = egui::Context::default();
                let mut app = app(part);
                app.text_ok = install_fonts(&ctx);
                assert!(app.text_ok, "no system font found");
                app.state.set_ui_zoom(zoom);
                app.readout = Some(("VCA LEVEL", 5));
                app.picker_open = true;
                assert!(run(&ctx, &mut app, Vec::new()) > 0, "{part:?} zoom {zoom}");
                assert_eq!(app.applied_zoom, zoom);
            }
        }
    }

    #[test]
    fn panel_draws_without_any_font() {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());
        let mut app = app(Part::Melody);
        assert!(run(&ctx, &mut app, Vec::new()) > 0);
    }

    fn click(position: egui::Pos2) -> Vec<Vec<egui::Event>> {
        let button = |pressed| egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        vec![
            vec![egui::Event::PointerMoved(position)],
            vec![button(true)],
            vec![button(false)],
            Vec::new(),
        ]
    }

    #[test]
    fn clicking_a_button_changes_the_parameter_and_queues_a_host_edit() {
        let ctx = egui::Context::default();
        let mut app = app(Part::Melody);
        let saw = app
            .panel
            .controls
            .iter()
            .find(|control| control.id == SAW_PARAMETER_ID)
            .unwrap()
            .rect;
        let (body, _) = widgets::button_parts(saw);
        assert_eq!(app.state.get_value(SAW_PARAMETER_ID), Some(0.0));
        for events in click(body.center()) {
            run(&ctx, &mut app, events);
        }
        assert_eq!(app.state.get_value(SAW_PARAMETER_ID), Some(1.0));
        let mut edits = Vec::new();
        app.edits.drain(|edit| {
            edits.push(edit);
            true
        });
        use crate::edits::Edit;
        assert_eq!(
            edits,
            [
                Edit::Begin(SAW_PARAMETER_ID),
                Edit::Value(SAW_PARAMETER_ID, 1.0),
                Edit::End(SAW_PARAMETER_ID)
            ]
        );
    }

    #[test]
    fn dragging_a_fader_holds_one_gesture_and_follows_the_pointer() {
        let ctx = egui::Context::default();
        let mut app = app(Part::Melody);
        let cell = app.panel.controls.iter().find(|c| c.id == 4).unwrap().rect;
        let (top, bottom) = widgets::fader_travel(cell);
        let x = cell.center().x;
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let start = pos2(x, bottom);
        let end = pos2(x, top);
        let mut frames = vec![
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, true)],
        ];
        for step in 1..=10 {
            let y = bottom + (top - bottom) * step as f32 / 10.0;
            frames.push(vec![egui::Event::PointerMoved(pos2(x, y))]);
        }
        frames.push(vec![button(end, false)]);
        frames.push(Vec::new());
        for events in frames {
            run(&ctx, &mut app, events);
        }
        assert_eq!(app.state.get_value(4), Some(127.0));
        assert!(app.held.is_none());
        let mut edits = Vec::new();
        app.edits.drain(|edit| {
            edits.push(edit);
            true
        });
        use crate::edits::Edit;
        assert_eq!(
            edits,
            [Edit::Begin(4), Edit::Value(4, 127.0), Edit::End(4)],
            "one drain coalesces the drag into a single paired gesture"
        );
    }
    #[test]
    fn one_wheel_notch_moves_a_fader_one_step_and_trackpad_scroll_accumulates() {
        let ctx = egui::Context::default();
        let mut app = app(Part::Melody);
        app.state.apply_value(4, 64.0, 1);
        let cell = app.panel.controls.iter().find(|c| c.id == 4).unwrap().rect;
        let over = cell.center();
        let wheel = |unit, y| egui::Event::MouseWheel {
            unit,
            delta: vec2(0.0, y),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        };
        run(&ctx, &mut app, vec![egui::Event::PointerMoved(over)]);
        run(&ctx, &mut app, vec![wheel(egui::MouseWheelUnit::Line, 1.0)]);
        for _ in 0..5 {
            run(&ctx, &mut app, Vec::new());
        }
        assert_eq!(app.state.get_value(4), Some(65.0));
        for _ in 0..3 {
            run(
                &ctx,
                &mut app,
                vec![wheel(egui::MouseWheelUnit::Point, -10.0)],
            );
        }
        assert_eq!(app.state.get_value(4), Some(64.0), "30 points is one step");
    }

    #[test]
    fn leftover_trackpad_scroll_does_not_carry_to_another_fader() {
        let ctx = egui::Context::default();
        let mut app = app(Part::Melody);
        app.state.apply_value(4, 64.0, 1);
        app.state.apply_value(5, 64.0, 1);
        let center = |id: u32| app_rect(&app, id).center();
        let (first, second) = (center(4), center(5));
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, 20.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        };
        run(&ctx, &mut app, vec![egui::Event::PointerMoved(first)]);
        run(&ctx, &mut app, vec![wheel.clone()]);
        run(&ctx, &mut app, vec![egui::Event::PointerMoved(second)]);
        run(&ctx, &mut app, vec![wheel]);
        assert_eq!(app.state.get_value(4), Some(64.0));
        assert_eq!(
            app.state.get_value(5),
            Some(64.0),
            "20 + 20 points split over two faders"
        );
    }

    fn app_rect(app: &App, id: u32) -> Rect {
        app.panel.controls.iter().find(|c| c.id == id).unwrap().rect
    }

    #[test]
    fn horizontal_scroll_and_a_click_on_the_cap_leave_a_fader_alone() {
        let ctx = egui::Context::default();
        let mut app = app(Part::Melody);
        app.state.apply_value(4, 64.0, 1);
        let cell = app_rect(&app, 4);
        let (top, bottom) = widgets::fader_travel(cell);
        let cap = pos2(cell.center().x, bottom - 64.0 / 127.0 * (bottom - top));
        run(&ctx, &mut app, vec![egui::Event::PointerMoved(cap)]);
        run(
            &ctx,
            &mut app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: vec2(1.0, 0.0),
                phase: egui::TouchPhase::Move,
                modifiers: Default::default(),
            }],
        );
        for events in click(cap + vec2(0.0, 3.0)) {
            run(&ctx, &mut app, events);
        }
        assert_eq!(app.state.get_value(4), Some(64.0));
        for events in click(pos2(cap.x, bottom)) {
            run(&ctx, &mut app, events);
        }
        assert_eq!(
            app.state.get_value(4),
            Some(0.0),
            "a click off the cap jumps there"
        );
    }

    #[test]
    fn fit_keeps_the_longest_prefix_that_fits() {
        let ctx = egui::Context::default();
        assert!(install_fonts(&ctx));
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            let c = Canvas {
                painter: &painter,
                origin: pos2(0.0, 0.0),
                zoom: 1.0,
                text_ok: true,
            };
            let name = "A rather long CoreMIDI destination name, port 1";
            let width = |text: &str| {
                painter
                    .layout_no_wrap(
                        text.to_owned(),
                        egui::FontId::proportional(11.0),
                        theme::PRINT,
                    )
                    .size()
                    .x
            };
            assert_eq!(c.fit("Short", 11.0, 200.0), "Short");
            let fitted = c.fit(name, 11.0, 120.0);
            assert!(
                fitted.ends_with("...") && width(&fitted) <= 120.0,
                "{fitted}"
            );
            let kept = fitted.len() - 3;
            let longer = format!("{}...", &name[..kept + 1]);
            assert!(width(&longer) > 120.0, "{longer} would also fit");
            assert_eq!(c.fit(name, 11.0, 1.0), "");
        });
    }

    #[test]
    fn movement_before_the_drag_threshold_still_moves_the_fader() {
        let ctx = egui::Context::default();
        let mut app = app(Part::Melody);
        app.state.apply_value(4, 64.0, 1);
        let cell = app_rect(&app, 4);
        let (top, bottom) = widgets::fader_travel(cell);
        let per_point = 127.0 / (bottom - top) as f64;
        let cap = pos2(cell.center().x, bottom - 64.0 / 127.0 * (bottom - top));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let (half, full) = (cap - vec2(0.0, 4.0), cap - vec2(0.0, 8.0));
        for events in [
            vec![egui::Event::PointerMoved(cap)],
            vec![button(cap, true)],
            vec![egui::Event::PointerMoved(half)],
            vec![egui::Event::PointerMoved(full)],
            vec![button(full, false)],
            Vec::new(),
        ] {
            run(&ctx, &mut app, events);
        }
        assert_eq!(
            app.state.get_value(4),
            Some((64.0 + 8.0 * per_point).round())
        );
    }

    #[test]
    fn closing_the_window_mid_drag_ends_the_gesture() {
        let edits;
        {
            let mut app = app(Part::Melody);
            edits = Arc::clone(&app.edits);
            app.held = Some((4, 10.0));
            app.edits.set_gesture(4, true);
            let mut seen = Vec::new();
            edits.drain(|edit| {
                seen.push(edit);
                true
            });
            assert_eq!(seen, [crate::edits::Edit::Begin(4)]);
        }
        let mut seen = Vec::new();
        edits.drain(|edit| {
            seen.push(edit);
            true
        });
        assert_eq!(seen, [crate::edits::Edit::End(4)]);
    }

    /// Software-rasterises a frame so the panel can be inspected without a host:
    /// `MKS7_RENDER_DIR=/some/dir cargo test render_previews -- --ignored`
    /// writes `<part>.rgba` files (a `width height` header line, then RGBA8 rows).
    #[test]
    #[ignore = "writes preview images; needs MKS7_RENDER_DIR"]
    fn render_previews() {
        let dir = std::env::var("MKS7_RENDER_DIR").expect("MKS7_RENDER_DIR");
        const SCALE: f32 = 2.0;
        for part in [Part::Melody, Part::Chord, Part::Bass] {
            let ctx = egui::Context::default();
            let mut app = app(part);
            app.text_ok = install_fonts(&ctx);
            for (id, value) in [(0, 40.0), (2, 64.0), (4, 100.0), (6, 30.0), (9, 127.0)] {
                app.state.apply_value(id, value, 1);
            }
            app.state.apply_value(SUB_LEVEL_PARAMETER_ID, 2.0, 1);
            app.state.apply_value(CHORUS_PARAMETER_ID, 1.0, 1);
            app.state.apply_modulation(4, -30.0);
            app.readout = Some(("VCF FREQ", 4));
            app.picker_open = part == Part::Chord;

            let (width, height) = Panel::window_size(part, app.applied_zoom);
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2(width as f32, height as f32),
                )),
                viewports: std::iter::once((
                    egui::ViewportId::ROOT,
                    egui::ViewportInfo {
                        native_pixels_per_point: Some(SCALE),
                        ..Default::default()
                    },
                ))
                .collect(),
                ..Default::default()
            };
            let output = ctx.run_ui(input, |ui| frame(ui, &mut app));
            let mut atlas: Vec<egui::Color32> = Vec::new();
            let mut atlas_size = [0usize; 2];
            for (_, delta) in &output.textures_delta.set {
                let egui::ImageData::Color(image) = &delta.image;
                match delta.pos {
                    None => {
                        atlas_size = image.size;
                        atlas = image.pixels.clone();
                    }
                    Some([x, y]) => {
                        for row in 0..image.size[1] {
                            for column in 0..image.size[0] {
                                atlas[(y + row) * atlas_size[0] + x + column] =
                                    image.pixels[row * image.size[0] + column];
                            }
                        }
                    }
                }
            }

            let (w, h) = (
                (width as f32 * SCALE) as usize,
                (height as f32 * SCALE) as usize,
            );
            let mut pixels = vec![[0f32; 3]; w * h];
            for primitive in ctx.tessellate(output.shapes, SCALE) {
                let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                    continue;
                };
                for triangle in mesh.indices.as_chunks::<3>().0 {
                    let v = [0, 1, 2].map(|i| mesh.vertices[triangle[i] as usize]);
                    let p = v.map(|vertex| vertex.pos * SCALE);
                    let area =
                        (p[1] - p[0]).x * (p[2] - p[0]).y - (p[1] - p[0]).y * (p[2] - p[0]).x;
                    if area.abs() < 1e-6 {
                        continue;
                    }
                    let min_x = p.iter().map(|q| q.x).fold(f32::MAX, f32::min).max(0.0) as usize;
                    let max_x = (p.iter().map(|q| q.x).fold(0.0, f32::max).ceil() as usize).min(w);
                    let min_y = p.iter().map(|q| q.y).fold(f32::MAX, f32::min).max(0.0) as usize;
                    let max_y = (p.iter().map(|q| q.y).fold(0.0, f32::max).ceil() as usize).min(h);
                    for y in min_y..max_y {
                        for x in min_x..max_x {
                            let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
                            let edge = |a: egui::Pos2, b: egui::Pos2| {
                                ((b - a).x * (q - a).y - (b - a).y * (q - a).x) / area
                            };
                            let weights = [edge(p[1], p[2]), edge(p[2], p[0]), edge(p[0], p[1])];
                            if weights.iter().any(|weight| *weight < 0.0) {
                                continue;
                            }
                            let mix = |f: &dyn Fn(usize) -> f32| -> f32 {
                                (0..3).map(|i| weights[i] * f(i)).sum()
                            };
                            let u = mix(&|i| v[i].uv.x) * atlas_size[0] as f32;
                            let t = mix(&|i| v[i].uv.y) * atlas_size[1] as f32;
                            let texel = atlas[(t as usize).min(atlas_size[1] - 1) * atlas_size[0]
                                + (u as usize).min(atlas_size[0] - 1)]
                            .to_array();
                            let alpha = mix(&|i| v[i].color.a() as f32) * texel[3] as f32 / 65025.0;
                            let pixel = &mut pixels[y * w + x];
                            for channel in 0..3 {
                                let source = mix(&|i| v[i].color.to_array()[channel] as f32)
                                    * texel[channel] as f32
                                    / 255.0;
                                pixel[channel] = source + pixel[channel] * (1.0 - alpha);
                            }
                        }
                    }
                }
            }
            let mut bytes = format!("{w} {h}\n").into_bytes();
            for pixel in pixels {
                bytes.extend(pixel.map(|channel| channel.clamp(0.0, 255.0) as u8));
                bytes.push(255);
            }
            let name = String::from_utf8_lossy(part.name()).to_lowercase();
            std::fs::write(format!("{dir}/{name}.rgba"), bytes).unwrap();
        }
    }
}

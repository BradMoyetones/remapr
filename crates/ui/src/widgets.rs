//! Remapr's widgets. Each one reports itself to screen readers through `widget_info`.

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontFamily, FontId, InnerResponse, Margin, Rect, Response, Role, Sense,
    Stroke, StrokeKind, Ui, Vec2, WidgetInfo, vec2,
};

use crate::Palette;
use crate::theme::{Icon, SEMIBOLD, Tone};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    Idle,
    /// Waiting for the user to press a key: the keycap pulses in the accent colour.
    Listening,
}

/// A keyboard keycap showing `label`.
pub fn kbd(ui: &mut Ui, label: &str, state: KeyState) -> Response {
    const FACE_H: f32 = 30.0;
    const DEPTH: f32 = 3.0;
    let p = Palette::of(ui);
    let galley =
        ui.painter().layout_no_wrap(label.to_owned(), FontId::new(14.0, FontFamily::Name(SEMIBOLD.into())), p.text);
    let width = (galley.size().x + 20.0).max(FACE_H);
    let (rect, response) = ui.allocate_exact_size(vec2(width, FACE_H + DEPTH), Sense::hover());
    if ui.is_rect_visible(rect) {
        let face = Rect::from_min_size(rect.min, vec2(width, FACE_H));
        let radius = CornerRadius::same(6);
        let (stroke, edge) = match state {
            KeyState::Idle => (Stroke::new(1.0, p.border), p.key_edge),
            KeyState::Listening => {
                let t = ui.input(|i| i.time) as f32;
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                let pulse = 0.55 + 0.45 * (t * 4.0).sin();
                (Stroke::new(1.5, p.accent.gamma_multiply(pulse)), p.accent.gamma_multiply(0.6))
            }
        };
        let painter = ui.painter();
        painter.rect_filled(face.translate(vec2(0.0, DEPTH)), radius, edge);
        painter.rect(face, radius, p.key_face, stroke, StrokeKind::Inside);
        painter.galley(face.center() - galley.size() / 2.0, galley, p.text);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, label));
    response
}

/// A square, frameless button showing one [`Icon`](crate::icon). `label` is its tooltip and
/// accessible name.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    icon_button_toned(ui, icon, label, false)
}

/// An [`icon_button`] for an action that deletes something: red, and redder on hover.
pub fn danger_icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    icon_button_toned(ui, icon, label, true)
}

fn icon_button_toned(ui: &mut Ui, icon: Icon, label: &str, danger: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(32.0), Sense::click());
    let p = Palette::of(ui);
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let (hover_fill, ring, idle, active) = if danger {
            (p.danger_soft, p.danger, p.danger.gamma_multiply(0.7), p.danger)
        } else {
            (p.surface_hover, p.accent, p.muted, p.text)
        };
        if hovered {
            ui.painter().rect_filled(rect, CornerRadius::same(8), hover_fill);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, ring), StrokeKind::Inside);
        }
        let color = if hovered { active } else { idle };
        paint_icon(ui, icon, rect.center(), 16.0, color);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(label)
}

/// The one main action of a screen: filled with the accent colour, with an optional leading icon.
pub fn primary_button(ui: &mut Ui, icon: Option<Icon>, text: &str) -> Response {
    let p = Palette::of(ui);
    let enabled = ui.is_enabled();
    let galley =
        ui.painter().layout_no_wrap(text.to_owned(), FontId::new(14.0, FontFamily::Name(SEMIBOLD.into())), p.on_accent);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 28.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if response.hovered() || response.has_focus() { p.accent_hover } else { p.accent };
        let fill = if enabled { fill } else { fill.gamma_multiply(0.45) };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(8), fill);
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(10),
                Stroke::new(1.5, p.accent),
                StrokeKind::Outside,
            );
        }
        let mut x = rect.left() + 14.0;
        if let Some(icon) = icon {
            paint_icon(ui, icon, egui::pos2(x + 8.0, rect.center().y), 16.0, p.on_accent);
            x += icon_w;
        }
        painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, p.on_accent);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, text));
    if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response }
}

/// An on/off switch. Returns a response that is `changed()` when flipped.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let size = vec2(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let p = Palette::of(ui);
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let track = if *on { p.accent } else { p.control };
        let radius = CornerRadius::same(11);
        ui.painter().rect_filled(rect, radius, track);
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(13),
                Stroke::new(1.5, p.accent),
                StrokeKind::Outside,
            );
        }
        let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
        let knob = if *on { p.on_accent } else { p.surface };
        ui.painter().circle_filled(egui::pos2(x, rect.center().y), 8.0, knob);
    }
    response.widget_info(|| WidgetInfo::selected(Role::Switch, ui.is_enabled(), *on, label));
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A surface for one item or group. `highlight` draws an accent border (the item needs attention).
pub fn card<R>(ui: &mut Ui, highlight: bool, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = Palette::of(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(if highlight { 1.5 } else { 1.0 }, if highlight { p.accent } else { p.border }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, add_contents)
}

/// A full-width message in the colours of its `tone`, with the tone's icon.
pub fn banner(ui: &mut Ui, tone: Tone, text: &str) {
    let (strong, soft) = Palette::of(ui).tone(tone);
    egui::Frame::new()
        .fill(soft)
        .stroke(Stroke::new(1.0, strong.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icon_label(ui, tone.icon(), 16.0, strong);
                ui.add(egui::Label::new(egui::RichText::new(text).color(strong)).wrap());
            });
        });
}

/// An [`Icon`] of `size` points, laid out like a label. Decorative: give it a tooltip with
/// `on_hover_text` when it carries meaning on its own.
pub fn icon_label(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        icon.image(size, color).paint_at(ui, rect);
    }
    response
}

fn paint_icon(ui: &Ui, icon: Icon, center: egui::Pos2, size: f32, color: Color32) {
    icon.image(size, color).paint_at(ui, Rect::from_center_size(center, Vec2::splat(size)));
}

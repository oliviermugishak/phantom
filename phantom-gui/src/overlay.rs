use std::path::Path;
use std::time::Duration;

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

use phantom::profile::{
    JoystickMode, MouseCameraActivationMode, Node, Profile, Region, RelPos,
};

const OVERLAY_BG: Color32 = Color32::TRANSPARENT;
const LABEL_BG: Color32 = Color32::from_black_alpha(110);
pub fn run_overlay(profile_path: &Path) -> eframe::Result<()> {
    let profile = Profile::load(profile_path).map_err(|e| {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(e.to_string())))
    })?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Phantom GUI Overlay")
            .with_app_id("phantom-overlay")
            .with_fullscreen(true)
            .with_always_on_top()
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_mouse_passthrough(true),
        ..Default::default()
    };

    eframe::run_native(
        "phantom-overlay",
        options,
        Box::new(|cc| Ok(Box::new(OverlayApp::new(cc, profile)))),
    )
}

struct OverlayApp {
    profile: Profile,
}

impl OverlayApp {
    fn new(cc: &eframe::CreationContext<'_>, profile: Profile) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self { profile }
    }
}

impl eframe::App for OverlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(OVERLAY_BG))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                draw_profile_overlay(&painter, rect, &self.profile);
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

fn draw_profile_overlay(painter: &egui::Painter, rect: Rect, profile: &Profile) {
    for node in &profile.nodes {
        match node {
            Node::Tap { pos, .. }
            | Node::HoldTap { pos, .. }
            | Node::ToggleTap { pos, .. }
            | Node::RepeatTap { pos, .. } => draw_button_marker(painter, rect, node, pos),
            Node::Joystick {
                mode: JoystickMode::Fixed,
                pos,
                radius,
                ..
            } => draw_fixed_joystick(painter, rect, node, pos, *radius),
            Node::Joystick {
                mode: JoystickMode::Floating,
                region: Some(region),
                ..
            } => draw_floating_joystick(painter, rect, node, region),
            Node::Drag { start, end, .. } => draw_drag_gesture(painter, rect, node, start, end),
            Node::MouseCamera {
                region,
                activation_mode,
                ..
            } => draw_mouse_region(painter, rect, node, region, activation_mode),
            Node::Macro { .. } | Node::LayerShift { .. } => {}
            Node::Joystick {
                mode: JoystickMode::Floating,
                region: None,
                ..
            } => {}
        }
    }
}

fn draw_button_marker(painter: &egui::Painter, rect: Rect, node: &Node, pos: &RelPos) {
    let center = to_canvas_pos(rect, pos);
    let fill = match node {
        Node::Tap { .. } => tap_fill(),
        Node::HoldTap { .. } => hold_fill(),
        Node::ToggleTap { .. } => toggle_fill(),
        Node::RepeatTap { .. } => repeat_fill(),
        _ => tap_fill(),
    };
    painter.circle_filled(center, 26.0, fill);
    painter.circle_stroke(center, 26.0, Stroke::new(1.5, Color32::from_white_alpha(170)));
    draw_label(
        painter,
        center + Vec2::new(0.0, 36.0),
        compact_label(node),
        Some(node.layer()),
    );
}

fn draw_fixed_joystick(
    painter: &egui::Painter,
    rect: Rect,
    node: &Node,
    pos: &RelPos,
    radius: f64,
) {
    let center = to_canvas_pos(rect, pos);
    let radius_px = rect.width().min(rect.height()) * radius as f32;
    painter.circle_filled(center, radius_px, joystick_fill());
    painter.circle_stroke(center, radius_px, Stroke::new(2.0, joystick_stroke()));
    painter.circle_stroke(
        center,
        (radius_px * 0.38).max(18.0),
        Stroke::new(1.0, Color32::from_white_alpha(110)),
    );
    draw_label(
        painter,
        center + Vec2::new(0.0, radius_px + 20.0),
        compact_label(node),
        Some(node.layer()),
    );
}

fn draw_floating_joystick(
    painter: &egui::Painter,
    rect: Rect,
    node: &Node,
    region: &Region,
) {
    let zone = region_rect(rect, region);
    let fill = joystick_fill();
    painter.rect_filled(zone, 24.0, fill.gamma_multiply(0.35));
    painter.rect_stroke(
        zone,
        24.0,
        Stroke::new(2.0, joystick_stroke()),
        StrokeKind::Inside,
    );
    painter.circle_stroke(
        zone.center(),
        zone.width().min(zone.height()) * 0.22,
        Stroke::new(1.2, Color32::from_white_alpha(120)),
    );
    draw_label(
        painter,
        Pos2::new(zone.center().x, zone.bottom() + 18.0),
        compact_label(node),
        Some(node.layer()),
    );
}

fn draw_drag_gesture(
    painter: &egui::Painter,
    rect: Rect,
    node: &Node,
    start: &RelPos,
    end: &RelPos,
) {
    let start = to_canvas_pos(rect, start);
    let end = to_canvas_pos(rect, end);
    painter.arrow(start, end - start, Stroke::new(2.0, drag_stroke()));
    painter.circle_filled(start, 14.0, Color32::from_rgba_unmultiplied(0, 200, 140, 70));
    painter.circle_stroke(start, 14.0, Stroke::new(1.5, drag_stroke()));
    draw_label(
        painter,
        start + Vec2::new(0.0, -22.0),
        compact_label(node),
        Some(node.layer()),
    );
}

fn draw_mouse_region(
    painter: &egui::Painter,
    rect: Rect,
    node: &Node,
    region: &Region,
    activation_mode: &MouseCameraActivationMode,
) {
    let zone = region_rect(rect, region);
    painter.rect_stroke(
        zone,
        24.0,
        Stroke::new(2.0, look_stroke()),
        StrokeKind::Inside,
    );
    painter.rect_filled(
        zone,
        24.0,
        Color32::from_rgba_unmultiplied(251, 188, 4, 28),
    );
    let mode = match activation_mode {
        MouseCameraActivationMode::AlwaysOn => "Look",
        MouseCameraActivationMode::WhileHeld => "Hold Look",
        MouseCameraActivationMode::Toggle => "Toggle Look",
    };
    let layer = if node.layer().trim().is_empty() {
        None
    } else {
        Some(node.layer())
    };
    draw_label(painter, zone.center_top() + Vec2::new(0.0, 18.0), mode, layer);
}

fn draw_label(
    painter: &egui::Painter,
    anchor: Pos2,
    primary: impl AsRef<str>,
    layer: Option<&str>,
) {
    let text = if let Some(layer) = layer.filter(|layer| !layer.trim().is_empty()) {
        format!("{}\n[{}]", primary.as_ref(), layer)
    } else {
        primary.as_ref().to_string()
    };
    let font = FontId::proportional(16.0);
    let galley = painter.layout_no_wrap(text.clone(), font.clone(), Color32::WHITE);
    let padding = Vec2::new(12.0, 8.0);
    let bounds = Rect::from_center_size(anchor, galley.size() + padding * 2.0);
    painter.rect_filled(bounds, 12.0, LABEL_BG);
    painter.text(
        anchor,
        Align2::CENTER_CENTER,
        text,
        font,
        Color32::WHITE,
    );
}

fn compact_label(node: &Node) -> String {
    match node {
        Node::Tap { key, .. }
        | Node::HoldTap { key, .. }
        | Node::ToggleTap { key, .. }
        | Node::Drag { key, .. }
        | Node::RepeatTap { key, .. } => key.clone(),
        Node::Joystick { keys, .. } => {
            format!("{}/{}/{}/{}", keys.up, keys.left, keys.down, keys.right)
        }
        Node::MouseCamera {
            activation_mode,
            activation_key,
            ..
        } => match activation_mode {
            MouseCameraActivationMode::AlwaysOn => "Mouse Look".into(),
            MouseCameraActivationMode::WhileHeld => {
                format!("Look: {}", activation_key.as_deref().unwrap_or("?"))
            }
            MouseCameraActivationMode::Toggle => {
                format!("Toggle: {}", activation_key.as_deref().unwrap_or("?"))
            }
        },
        Node::Macro { id, .. } | Node::LayerShift { id, .. } => id.clone(),
    }
}

fn to_canvas_pos(rect: Rect, pos: &RelPos) -> Pos2 {
    Pos2::new(
        rect.left() + rect.width() * pos.x as f32,
        rect.top() + rect.height() * pos.y as f32,
    )
}

fn region_rect(content: Rect, region: &Region) -> Rect {
    Rect::from_min_size(
        to_canvas_pos(
            content,
            &RelPos {
                x: region.x,
                y: region.y,
            },
        ),
        Vec2::new(
            content.width() * region.w as f32,
            content.height() * region.h as f32,
        ),
    )
}

fn tap_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(66, 133, 244, 110)
}

fn hold_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(234, 67, 53, 110)
}

fn toggle_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 172, 193, 110)
}

fn repeat_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(171, 71, 188, 110)
}

fn joystick_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(52, 168, 83, 70)
}

fn joystick_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(114, 227, 146, 210)
}

fn drag_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 200, 140, 220)
}

fn look_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(251, 188, 4, 200)
}

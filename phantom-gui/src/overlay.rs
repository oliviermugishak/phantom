use std::path::Path;

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};

use phantom::profile::{JoystickMode, MouseCameraActivationMode, Node, Profile, Region, RelPos};

const OVERLAY_BG: Color32 = Color32::from_rgb(11, 13, 18);
const MARKER_RADIUS: f32 = 14.0;
const SMALL_MARKER_RADIUS: f32 = 9.0;
const TEXT_SHADOW: Color32 = Color32::from_black_alpha(190);
const GUIDE_LENGTH: f32 = 14.0;
const HEADER_TEXT: &str = "Experimental debug preview — not gameplay-safe";

pub fn run_overlay(profile_path: &Path) -> eframe::Result<()> {
    let profile = Profile::load(profile_path)
        .map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::other(e.to_string()))))?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Phantom Overlay Preview (Experimental)")
            .with_app_id("phantom-overlay")
            .with_fullscreen(true)
            .with_always_on_top()
            .with_transparent(false)
            .with_decorations(false)
            .with_resizable(false)
            .with_mouse_passthrough(false),
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
                draw_overlay_header(&painter, rect);
            });
    }
}

fn draw_profile_overlay(painter: &egui::Painter, rect: Rect, profile: &Profile) {
    for node in &profile.nodes {
        match node {
            Node::Tap { pos, .. }
            | Node::HoldTap { pos, .. }
            | Node::ToggleTap { pos, .. }
            | Node::RepeatTap { pos, .. } => draw_button_marker(painter, rect, node, pos),
            Node::Wheel {
                up_pos, down_pos, ..
            } => draw_wheel_markers(painter, rect, up_pos, down_pos),
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
                anchor,
                activation_mode,
                ..
            } => draw_aim_marker(painter, rect, anchor, activation_mode),
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
    draw_ring_marker(
        painter,
        center,
        MARKER_RADIUS,
        marker_stroke(node),
        compact_label(node),
    );
}

fn draw_wheel_markers(painter: &egui::Painter, rect: Rect, up_pos: &RelPos, down_pos: &RelPos) {
    let up_center = to_canvas_pos(rect, up_pos);
    let down_center = to_canvas_pos(rect, down_pos);
    let stroke = wheel_stroke();
    draw_ring_marker(
        painter,
        up_center,
        MARKER_RADIUS,
        Stroke::new(2.0, stroke),
        "Up",
    );
    draw_ring_marker(
        painter,
        down_center,
        MARKER_RADIUS,
        Stroke::new(2.0, stroke),
        "Dn",
    );
    painter.line_segment(
        [up_center, down_center],
        Stroke::new(1.5, stroke.gamma_multiply(0.7)),
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
    painter.circle_stroke(center, radius_px, Stroke::new(2.0, joystick_stroke()));
    painter.circle_stroke(
        center,
        (radius_px * 0.22).max(10.0),
        Stroke::new(1.5, joystick_stroke().gamma_multiply(0.8)),
    );
    if let Node::Joystick { keys, .. } = node {
        let label_radius = radius_px + 22.0;
        draw_ring_marker(
            painter,
            center + Vec2::new(0.0, -label_radius),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.up,
        );
        draw_ring_marker(
            painter,
            center + Vec2::new(-label_radius, 0.0),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.left,
        );
        draw_ring_marker(
            painter,
            center + Vec2::new(label_radius, 0.0),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.right,
        );
        draw_ring_marker(
            painter,
            center + Vec2::new(0.0, label_radius),
            SMALL_MARKER_RADIUS,
            Stroke::new(1.5, joystick_stroke()),
            &keys.down,
        );
    }
}

fn draw_floating_joystick(painter: &egui::Painter, rect: Rect, node: &Node, region: &Region) {
    let zone = region_rect(rect, region);
    draw_corner_guides(painter, zone, joystick_stroke());
    draw_ring_marker(
        painter,
        zone.center(),
        SMALL_MARKER_RADIUS,
        Stroke::new(1.5, joystick_stroke()),
        compact_label(node),
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
    draw_ring_marker(
        painter,
        start,
        SMALL_MARKER_RADIUS,
        Stroke::new(1.5, drag_stroke()),
        compact_label(node),
    );
    painter.circle_stroke(
        end,
        8.0,
        Stroke::new(1.5, drag_stroke().gamma_multiply(0.85)),
    );
}

fn draw_aim_marker(
    painter: &egui::Painter,
    rect: Rect,
    anchor: &RelPos,
    activation_mode: &MouseCameraActivationMode,
) {
    let center = to_canvas_pos(rect, anchor);
    let mode = match activation_mode {
        MouseCameraActivationMode::AlwaysOn => "Aim",
        MouseCameraActivationMode::WhileHeld => "Hold Aim",
        MouseCameraActivationMode::Toggle => "Toggle Aim",
    };
    draw_ring_marker(
        painter,
        center,
        MARKER_RADIUS,
        Stroke::new(2.0, look_stroke()),
        mode,
    );
}

fn draw_ring_marker(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    stroke: Stroke,
    text: impl AsRef<str>,
) {
    painter.circle_stroke(center, radius, stroke);
    painter.circle_filled(center, radius - 2.0, marker_fill(stroke.color));
    draw_centered_text(painter, center, text.as_ref(), Color32::WHITE);
}

fn draw_overlay_header(painter: &egui::Painter, rect: Rect) {
    let header_rect = Rect::from_min_size(
        rect.left_top() + Vec2::new(20.0, 20.0),
        Vec2::new(360.0, 30.0),
    );
    painter.rect_filled(header_rect, 8.0, Color32::from_black_alpha(190));
    painter.text(
        header_rect.center(),
        Align2::CENTER_CENTER,
        HEADER_TEXT,
        FontId::proportional(13.0),
        Color32::from_rgb(255, 226, 150),
    );
}

fn draw_centered_text(painter: &egui::Painter, center: Pos2, text: &str, color: Color32) {
    let font_size = if text.len() >= 8 {
        11.0
    } else if text.len() >= 5 {
        12.0
    } else {
        14.0
    };
    let font = FontId::proportional(font_size);
    painter.text(
        center + Vec2::new(1.0, 1.0),
        Align2::CENTER_CENTER,
        text,
        font.clone(),
        TEXT_SHADOW,
    );
    painter.text(center, Align2::CENTER_CENTER, text, font, color);
}

fn draw_corner_guides(painter: &egui::Painter, rect: Rect, color: Color32) {
    let stroke = Stroke::new(2.0, color);
    draw_corner_guide(
        painter,
        rect.left_top(),
        Vec2::new(GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, GUIDE_LENGTH),
        stroke,
    );
    draw_corner_guide(
        painter,
        rect.right_top(),
        Vec2::new(-GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, GUIDE_LENGTH),
        stroke,
    );
    draw_corner_guide(
        painter,
        rect.left_bottom(),
        Vec2::new(GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, -GUIDE_LENGTH),
        stroke,
    );
    draw_corner_guide(
        painter,
        rect.right_bottom(),
        Vec2::new(-GUIDE_LENGTH, 0.0),
        Vec2::new(0.0, -GUIDE_LENGTH),
        stroke,
    );
}

fn draw_corner_guide(
    painter: &egui::Painter,
    corner: Pos2,
    horizontal: Vec2,
    vertical: Vec2,
    stroke: Stroke,
) {
    painter.line_segment([corner, corner + horizontal], stroke);
    painter.line_segment([corner, corner + vertical], stroke);
}

fn compact_label(node: &Node) -> String {
    match node {
        Node::Tap { key, .. }
        | Node::HoldTap { key, .. }
        | Node::ToggleTap { key, .. }
        | Node::Drag { key, .. }
        | Node::RepeatTap { key, .. } => key.clone(),
        Node::Wheel { .. } => "Wheel".into(),
        Node::Joystick { .. } => "Move".into(),
        Node::MouseCamera {
            activation_mode,
            activation_key,
            ..
        } => match activation_mode {
            MouseCameraActivationMode::AlwaysOn => "Aim".into(),
            MouseCameraActivationMode::WhileHeld => {
                activation_key.as_deref().unwrap_or("Aim").into()
            }
            MouseCameraActivationMode::Toggle => activation_key.as_deref().unwrap_or("Aim").into(),
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

fn marker_fill(color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18)
}

fn joystick_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(114, 227, 146, 210)
}

fn drag_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 200, 140, 220)
}

fn wheel_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(87, 175, 255, 220)
}

fn look_stroke() -> Color32 {
    Color32::from_rgba_unmultiplied(251, 188, 4, 200)
}

fn marker_stroke(node: &Node) -> Stroke {
    let color = match node {
        Node::Tap { .. } => Color32::from_rgb(66, 133, 244),
        Node::HoldTap { .. } => Color32::from_rgb(234, 67, 53),
        Node::ToggleTap { .. } => Color32::from_rgb(0, 172, 193),
        Node::RepeatTap { .. } => Color32::from_rgb(171, 71, 188),
        _ => Color32::WHITE,
    };
    Stroke::new(2.0, color)
}

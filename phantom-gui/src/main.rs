use std::path::PathBuf;

use eframe::egui;
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use phantom::config;
use phantom::profile::{Node, Profile, RelPos};

const COLOR_TAP: Color32 = Color32::from_rgb(66, 133, 244);
const COLOR_HOLD: Color32 = Color32::from_rgb(234, 67, 53);
const COLOR_JOYSTICK: Color32 = Color32::from_rgb(52, 168, 83);
const COLOR_CAMERA: Color32 = Color32::from_rgb(251, 188, 4);
const COLOR_REPEAT: Color32 = Color32::from_rgb(171, 71, 188);
const COLOR_MACRO: Color32 = Color32::from_rgb(255, 112, 67);

fn node_color(node: &Node) -> Color32 {
    match node {
        Node::Tap { .. } => COLOR_TAP,
        Node::HoldTap { .. } => COLOR_HOLD,
        Node::Joystick { .. } => COLOR_JOYSTICK,
        Node::MouseCamera { .. } => COLOR_CAMERA,
        Node::RepeatTap { .. } => COLOR_REPEAT,
        Node::Macro { .. } => COLOR_MACRO,
    }
}

fn node_label(node: &Node) -> &str {
    match node {
        Node::Tap { id, .. }
        | Node::HoldTap { id, .. }
        | Node::Joystick { id, .. }
        | Node::MouseCamera { id, .. }
        | Node::RepeatTap { id, .. }
        | Node::Macro { id, .. } => id,
    }
}

fn node_type_name(node: &Node) -> &'static str {
    match node {
        Node::Tap { .. } => "tap",
        Node::HoldTap { .. } => "hold_tap",
        Node::Joystick { .. } => "joystick",
        Node::MouseCamera { .. } => "mouse_camera",
        Node::RepeatTap { .. } => "repeat_tap",
        Node::Macro { .. } => "macro",
    }
}

fn node_pos(node: &Node) -> Option<&RelPos> {
    match node {
        Node::Tap { pos, .. }
        | Node::HoldTap { pos, .. }
        | Node::Joystick { pos, .. }
        | Node::RepeatTap { pos, .. } => Some(pos),
        _ => None,
    }
}

fn node_pos_mut(node: &mut Node) -> Option<&mut RelPos> {
    match node {
        Node::Tap { pos, .. }
        | Node::HoldTap { pos, .. }
        | Node::Joystick { pos, .. }
        | Node::RepeatTap { pos, .. } => Some(pos),
        _ => None,
    }
}

pub struct PhantomGui {
    profile: Option<Profile>,
    profile_path: Option<PathBuf>,
    screenshot: Option<egui::TextureHandle>,
    selected: Option<usize>,
    dirty: bool,
    dragging: Option<usize>,
}

impl PhantomGui {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            profile: None,
            profile_path: None,
            screenshot: None,
            selected: None,
            dirty: false,
            dragging: None,
        }
    }

    fn load_profile(&mut self, path: &std::path::Path) {
        match Profile::load(path) {
            Ok(profile) => {
                self.profile = Some(profile);
                self.profile_path = Some(path.to_path_buf());
                self.selected = None;
                self.dirty = false;
            }
            Err(e) => {
                tracing::error!("load failed: {}", e);
            }
        }
    }

    fn save_profile(&mut self) {
        if let (Some(profile), Some(path)) = (&self.profile, &self.profile_path) {
            match serde_json::to_string_pretty(profile) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(path, &json) {
                        tracing::error!("save failed: {}", e);
                    } else {
                        self.dirty = false;
                    }
                }
                Err(e) => tracing::error!("serialize failed: {}", e),
            }
        }
    }

    fn load_screenshot(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
            .pick_file()
        {
            match image::open(&path) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels: Vec<Color32> = rgba
                        .pixels()
                        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                        .collect();
                    let texture = ctx.load_texture(
                        "screenshot",
                        egui::ImageData::Color(std::sync::Arc::new(egui::ColorImage {
                            size,
                            pixels,
                        })),
                        egui::TextureOptions::default(),
                    );
                    self.screenshot = Some(texture);
                }
                Err(e) => tracing::error!("image load failed: {}", e),
            }
        }
    }
}

impl eframe::App for PhantomGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // === Top bar ===
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_directory(config::profiles_dir())
                            .pick_file()
                        {
                            self.load_profile(&p);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.save_profile();
                        ui.close_menu();
                    }
                    if ui.button("Save As...").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_directory(config::profiles_dir())
                            .save_file()
                        {
                            self.profile_path = Some(p);
                            self.save_profile();
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button("Nodes", |ui| {
                    if ui.button("Add Tap").clicked() {
                        self.add_node("tap");
                        ui.close_menu();
                    }
                    if ui.button("Add Hold-Tap").clicked() {
                        self.add_node("hold_tap");
                        ui.close_menu();
                    }
                    if ui.button("Add Joystick").clicked() {
                        self.add_node("joystick");
                        ui.close_menu();
                    }
                    if ui.button("Add Camera").clicked() {
                        self.add_node("mouse_camera");
                        ui.close_menu();
                    }
                    if ui.button("Add Repeat-Tap").clicked() {
                        self.add_node("repeat_tap");
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Delete Selected").clicked() {
                        self.delete_selected();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Load Screenshot...").clicked() {
                        self.load_screenshot(ctx);
                        ui.close_menu();
                    }
                    if ui.button("Clear Screenshot").clicked() {
                        self.screenshot = None;
                        ui.close_menu();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.dirty {
                        ui.label(RichText::new("[unsaved]").color(Color32::YELLOW));
                    }
                    if let Some(ref p) = self.profile {
                        ui.label(RichText::new(&p.name).weak());
                    }
                });
            });
        });

        // === Sidebar ===
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                if let Some(ref profile) = self.profile {
                    ui.heading("Nodes");
                    ui.separator();

                    let mut clicked = None;
                    for (i, node) in profile.nodes.iter().enumerate() {
                        let sel = self.selected == Some(i);
                        let text = RichText::new(format!(
                            "[{}] {} ({})",
                            node.slot().map_or("?".into(), |s| s.to_string()),
                            node_label(node),
                            node_type_name(node),
                        ))
                        .color(node_color(node));
                        if ui.selectable_label(sel, text).clicked() {
                            clicked = Some(i);
                        }
                    }
                    if let Some(i) = clicked {
                        self.selected = Some(i);
                    }

                    ui.separator();
                    if let Some(idx) = self.selected {
                        self.show_editor(ui, idx);
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label("No profile loaded");
                        ui.add_space(10.0);
                        if ui.button("Open Profile").clicked() {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .set_directory(config::profiles_dir())
                                .pick_file()
                            {
                                self.load_profile(&p);
                            }
                        }
                    });
                }
            });

        // === Canvas ===
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.profile.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a profile to start editing");
                });
                return;
            }

            let resp = ui.allocate_response(ui.available_size(), Sense::click_and_drag());
            let canvas = resp.rect;
            let painter = ui.painter_at(canvas);

            // Background
            if let Some(ref tex) = self.screenshot {
                painter.image(
                    tex.id(),
                    canvas,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                painter.rect_filled(canvas, 0.0, Color32::from_gray(30));
            }

            // Grid
            for i in 1..10 {
                let f = i as f32 / 10.0;
                let c = Color32::from_white_alpha(15);
                painter.line_segment(
                    [pos2(canvas, f, 0.0), pos2(canvas, f, 1.0)],
                    Stroke::new(1.0, c),
                );
                painter.line_segment(
                    [pos2(canvas, 0.0, f), pos2(canvas, 1.0, f)],
                    Stroke::new(1.0, c),
                );
            }

            // Draw nodes
            if let Some(ref profile) = self.profile {
                for (i, node) in profile.nodes.iter().enumerate() {
                    let sel = self.selected == Some(i);
                    let color = node_color(node);
                    let label = node_label(node);

                    if let Node::MouseCamera { region, .. } = node {
                        let r = Rect::from_min_size(
                            pos2(canvas, region.x as f32, region.y as f32),
                            Vec2::new(
                                region.w as f32 * canvas.width(),
                                region.h as f32 * canvas.height(),
                            ),
                        );
                        let stroke = if sel {
                            Stroke::new(3.0, Color32::WHITE)
                        } else {
                            Stroke::new(2.0, color)
                        };
                        painter.rect_stroke(r, 4.0, stroke, egui::StrokeKind::Outside);
                        painter.text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::proportional(12.0),
                            color,
                        );
                    } else if let Some(pos) = node_pos(node) {
                        let pt = pos2(canvas, pos.x as f32, pos.y as f32);
                        let r = if sel { 14.0 } else { 10.0 };
                        painter.circle_filled(pt, r, color);
                        if sel {
                            painter.circle_stroke(pt, r + 3.0, Stroke::new(2.0, Color32::WHITE));
                        }
                        painter.text(
                            Pos2::new(pt.x, pt.y + r + 4.0),
                            egui::Align2::CENTER_TOP,
                            label,
                            egui::FontId::proportional(10.0),
                            Color32::WHITE,
                        );
                    }
                }
            }

            // Drag logic
            if let Some(ref mut profile) = self.profile {
                if resp.drag_started() {
                    let mouse = resp.hover_pos().unwrap_or(canvas.center());
                    let mut best: Option<(usize, f32)> = None;
                    for (i, node) in profile.nodes.iter().enumerate() {
                        if let Some(pos) = node_pos(node) {
                            let pt = pos2(canvas, pos.x as f32, pos.y as f32);
                            let d = (mouse - pt).length();
                            if d < 20.0 && best.as_ref().map_or(true, |(_, bd)| d < *bd) {
                                best = Some((i, d));
                            }
                        }
                    }
                    if let Some((i, _)) = best {
                        self.dragging = Some(i);
                        self.selected = Some(i);
                    }
                }

                if resp.dragged() {
                    if let (Some(idx), Some(mouse)) = (self.dragging, resp.hover_pos()) {
                        let rx = ((mouse.x - canvas.left()) / canvas.width()).clamp(0.0, 1.0);
                        let ry = ((mouse.y - canvas.top()) / canvas.height()).clamp(0.0, 1.0);
                        if idx < profile.nodes.len() {
                            if let Some(pos) = node_pos_mut(&mut profile.nodes[idx]) {
                                pos.x = (rx * 1000.0).round() as f64 / 1000.0;
                                pos.y = (ry * 1000.0).round() as f64 / 1000.0;
                                self.dirty = true;
                            }
                        }
                    }
                }

                if resp.drag_stopped() {
                    self.dragging = None;
                }
            }

            // Coordinates shown in top bar instead
        });
    }
}

impl PhantomGui {
    fn add_node(&mut self, kind: &str) {
        if let Some(ref mut profile) = self.profile {
            let slot = profile
                .nodes
                .iter()
                .filter_map(|n| n.slot())
                .max()
                .map(|s| s + 1)
                .unwrap_or(0);
            if slot > 9 {
                return;
            }
            let node = match kind {
                "tap" => Node::Tap {
                    id: format!("tap_{}", slot),
                    slot,
                    pos: RelPos { x: 0.5, y: 0.5 },
                    key: "Space".into(),
                },
                "hold_tap" => Node::HoldTap {
                    id: format!("hold_{}", slot),
                    slot,
                    pos: RelPos { x: 0.5, y: 0.5 },
                    key: "MouseLeft".into(),
                },
                "joystick" => Node::Joystick {
                    id: format!("joystick_{}", slot),
                    slot,
                    pos: RelPos { x: 0.2, y: 0.7 },
                    radius: 0.07,
                    keys: phantom::profile::JoystickKeys {
                        up: "W".into(),
                        down: "S".into(),
                        left: "A".into(),
                        right: "D".into(),
                    },
                },
                "mouse_camera" => Node::MouseCamera {
                    id: format!("camera_{}", slot),
                    slot,
                    region: phantom::profile::Region {
                        x: 0.35,
                        y: 0.0,
                        w: 0.65,
                        h: 1.0,
                    },
                    sensitivity: 1.0,
                    invert_y: false,
                },
                "repeat_tap" => Node::RepeatTap {
                    id: format!("repeat_{}", slot),
                    slot,
                    pos: RelPos { x: 0.5, y: 0.5 },
                    key: "F".into(),
                    interval_ms: 100,
                },
                _ => return,
            };
            profile.nodes.push(node);
            self.dirty = true;
            self.selected = Some(profile.nodes.len() - 1);
        }
    }

    fn delete_selected(&mut self) {
        if let (Some(ref mut profile), Some(idx)) = (&mut self.profile, self.selected) {
            if idx < profile.nodes.len() {
                profile.nodes.remove(idx);
                self.selected = None;
                self.dirty = true;
            }
        }
    }

    fn show_editor(&mut self, ui: &mut egui::Ui, idx: usize) {
        if let Some(ref mut profile) = self.profile {
            if idx >= profile.nodes.len() {
                return;
            }
            let node = &mut profile.nodes[idx];

            // Type label
            ui.label(format!("Type: {}", node_type_name(node)));
            if let Some(slot) = node.slot() {
                ui.label(format!("Slot: {}", slot));
            }

            // Position editor
            if let Some(pos) = node_pos_mut(node) {
                ui.separator();
                ui.label("Position:");
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("X:");
                    let mut x = pos.x;
                    if ui
                        .add(egui::DragValue::new(&mut x).speed(0.001).range(0.0..=1.0))
                        .changed()
                    {
                        pos.x = (x * 1000.0).round() / 1000.0;
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Y:");
                    let mut y = pos.y;
                    if ui
                        .add(egui::DragValue::new(&mut y).speed(0.001).range(0.0..=1.0))
                        .changed()
                    {
                        pos.y = (y * 1000.0).round() / 1000.0;
                        changed = true;
                    }
                });
                if changed {
                    self.dirty = true;
                }
            }

            // Type-specific fields
            ui.separator();
            match node {
                Node::Tap { key, .. } | Node::HoldTap { key, .. } => {
                    ui.label("Key:");
                    if ui.text_edit_singleline(key).changed() {
                        self.dirty = true;
                    }
                }
                Node::RepeatTap {
                    key, interval_ms, ..
                } => {
                    ui.label("Key:");
                    if ui.text_edit_singleline(key).changed() {
                        self.dirty = true;
                    }
                    ui.label("Interval (ms):");
                    let mut v = *interval_ms as f64;
                    if ui
                        .add(egui::DragValue::new(&mut v).speed(1.0).range(16.0..=1000.0))
                        .changed()
                    {
                        *interval_ms = v as u64;
                        self.dirty = true;
                    }
                }
                Node::Joystick { radius, keys, .. } => {
                    ui.label("Radius:");
                    let mut r = *radius;
                    if ui
                        .add(egui::DragValue::new(&mut r).speed(0.001).range(0.01..=0.5))
                        .changed()
                    {
                        *radius = (r * 1000.0).round() / 1000.0;
                        self.dirty = true;
                    }
                    ui.label("Direction keys:");
                    for (label, field) in [
                        ("Up", &mut keys.up),
                        ("Down", &mut keys.down),
                        ("Left", &mut keys.left),
                        ("Right", &mut keys.right),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            if ui.text_edit_singleline(field).changed() {
                                self.dirty = true;
                            }
                        });
                    }
                }
                Node::MouseCamera {
                    region,
                    sensitivity,
                    invert_y,
                    ..
                } => {
                    ui.label("Region:");
                    let mut changed = false;
                    for (label, val, lo, hi) in [
                        ("X", &mut region.x, 0.0, 1.0),
                        ("Y", &mut region.y, 0.0, 1.0),
                        ("W", &mut region.w, 0.01, 1.0),
                        ("H", &mut region.h, 0.01, 1.0),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            if ui
                                .add(egui::DragValue::new(val).speed(0.001).range(lo..=hi))
                                .changed()
                            {
                                changed = true;
                            }
                        });
                    }
                    ui.label("Sensitivity:");
                    let mut s = *sensitivity;
                    if ui
                        .add(egui::DragValue::new(&mut s).speed(0.01).range(0.01..=10.0))
                        .changed()
                    {
                        *sensitivity = (s * 100.0).round() / 100.0;
                        changed = true;
                    }
                    if ui.checkbox(invert_y, "Invert Y").changed() {
                        changed = true;
                    }
                    if changed {
                        self.dirty = true;
                    }
                }
                Node::Macro { sequence, .. } => {
                    ui.label(format!("{} steps", sequence.len()));
                    for (i, step) in sequence.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}. {:?}", i + 1, step.action));
                            if let Some(p) = &step.pos {
                                ui.label(format!("({:.2},{:.2})", p.x, p.y));
                            }
                            ui.label(format!("s:{}", step.slot));
                            if step.delay_ms > 0 {
                                ui.label(format!("+{}ms", step.delay_ms));
                            }
                        });
                    }
                }
            }
        }
    }
}

fn pos2(rect: Rect, rx: f32, ry: f32) -> Pos2 {
    Pos2::new(
        rect.left() + rx * rect.width(),
        rect.top() + ry * rect.height(),
    )
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Phantom — Profile Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "phantom-gui",
        options,
        Box::new(|cc| Ok(Box::new(PhantomGui::new(cc)))),
    )
}

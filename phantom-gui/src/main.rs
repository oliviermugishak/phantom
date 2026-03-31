use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

use phantom::config;
use phantom::ipc::{self, IpcRequest, IpcResponse};
use phantom::profile::{
    JoystickKeys, LayerMode, MacroAction, MacroStep, Node, Profile, Region, RelPos, ScreenOverride,
};

const COLOR_TAP: Color32 = Color32::from_rgb(66, 133, 244);
const COLOR_HOLD: Color32 = Color32::from_rgb(234, 67, 53);
const COLOR_TOGGLE: Color32 = Color32::from_rgb(0, 172, 193);
const COLOR_JOYSTICK: Color32 = Color32::from_rgb(52, 168, 83);
const COLOR_LOOK: Color32 = Color32::from_rgb(251, 188, 4);
const COLOR_REPEAT: Color32 = Color32::from_rgb(171, 71, 188);
const COLOR_MACRO: Color32 = Color32::from_rgb(255, 112, 67);
const COLOR_LAYER: Color32 = Color32::from_rgb(158, 158, 158);
const CANVAS_BG: Color32 = Color32::from_rgb(19, 21, 26);
const CONTENT_BG: Color32 = Color32::from_rgb(26, 29, 36);
const HANDLE_SIZE: f32 = 10.0;

const BINDABLE_KEYS: &[&str] = &[
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "Space",
    "Enter",
    "Tab",
    "Esc",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "LeftCtrl",
    "RightCtrl",
    "LeftShift",
    "RightShift",
    "LeftAlt",
    "RightAlt",
    "LeftMeta",
    "RightMeta",
    "MouseLeft",
    "MouseRight",
    "MouseMiddle",
    "MouseBack",
    "MouseForward",
    "WheelUp",
    "WheelDown",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Place(NodeTemplate),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeTemplate {
    Tap,
    HoldTap,
    ToggleTap,
    Joystick,
    MouseLook,
    RepeatTap,
    Macro,
    LayerShift,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JoyDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, PartialEq, Eq)]
enum BindingTarget {
    Primary(usize),
    Joystick { idx: usize, dir: JoyDir },
}

#[derive(Clone, Copy)]
enum RegionHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

enum DragState {
    Point {
        idx: usize,
    },
    RegionMove {
        idx: usize,
        origin: Region,
        start_mouse: Pos2,
    },
    RegionResize {
        idx: usize,
        handle: RegionHandle,
        origin: Region,
        start_mouse: Pos2,
    },
}

#[derive(Default)]
struct RuntimeState {
    connected: bool,
    profile: Option<String>,
    paused: bool,
    capture_active: bool,
    screen: Option<(u32, u32)>,
    last_error: Option<String>,
    last_checked: Option<Instant>,
}

struct Banner {
    text: String,
    is_error: bool,
}

pub struct PhantomGui {
    config: config::Config,
    profile: Option<Profile>,
    profile_path: Option<PathBuf>,
    screenshot: Option<egui::TextureHandle>,
    selected: Option<usize>,
    tool: Tool,
    drag_state: Option<DragState>,
    pending_binding: Option<BindingTarget>,
    pending_binding_started_at: Option<Instant>,
    runtime: RuntimeState,
    banner: Option<Banner>,
    dirty: bool,
    show_labels: bool,
    auto_push_on_save: bool,
}

impl PhantomGui {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = config::load_config();
        Self {
            config,
            profile: None,
            profile_path: None,
            screenshot: None,
            selected: None,
            tool: Tool::Select,
            drag_state: None,
            pending_binding: None,
            pending_binding_started_at: None,
            runtime: RuntimeState::default(),
            banner: None,
            dirty: false,
            show_labels: true,
            auto_push_on_save: true,
        }
    }

    fn set_banner(&mut self, text: impl Into<String>, is_error: bool) {
        self.banner = Some(Banner {
            text: text.into(),
            is_error,
        });
    }

    fn default_screen(&self) -> ScreenOverride {
        ScreenOverride {
            width: self.config.screen.width.unwrap_or(1920),
            height: self.config.screen.height.unwrap_or(1080),
        }
    }

    fn new_profile(&mut self) {
        self.profile = Some(Profile {
            name: "New Profile".into(),
            version: 1,
            screen: Some(self.default_screen()),
            global_sensitivity: 1.0,
            nodes: Vec::new(),
        });
        self.profile_path = None;
        self.selected = None;
        self.dirty = true;
        self.tool = Tool::Select;
        self.set_banner("New profile created", false);
    }

    fn load_profile(&mut self, path: &Path) {
        match Profile::load(path) {
            Ok(profile) => {
                self.profile = Some(profile);
                self.profile_path = Some(path.to_path_buf());
                self.selected = None;
                self.dirty = false;
                self.tool = Tool::Select;
                self.set_banner(format!("Loaded {}", path.display()), false);
            }
            Err(e) => self.set_banner(format!("Load failed: {}", e), true),
        }
    }

    fn save_profile(&mut self) {
        let (Some(profile), Some(path)) = (&self.profile, &self.profile_path) else {
            self.set_banner("Choose a file path before saving", true);
            return;
        };
        if let Err(e) = profile.validate() {
            self.set_banner(format!("Cannot save invalid profile: {}", e), true);
            return;
        }
        match serde_json::to_string_pretty(profile) {
            Ok(json) => match std::fs::write(path, json) {
                Ok(()) => {
                    self.dirty = false;
                    self.set_banner(format!("Saved {}", path.display()), false);
                    if self.auto_push_on_save {
                        self.push_profile_live();
                    }
                }
                Err(e) => self.set_banner(format!("Save failed: {}", e), true),
            },
            Err(e) => self.set_banner(format!("Serialize failed: {}", e), true),
        }
    }

    fn save_profile_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_directory(config::profiles_dir())
            .save_file()
        {
            self.profile_path = Some(path);
            self.save_profile();
        }
    }

    fn load_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
            .pick_file()
        else {
            return;
        };

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
                    egui::ImageData::Color(std::sync::Arc::new(egui::ColorImage { size, pixels })),
                    egui::TextureOptions::LINEAR,
                );
                self.screenshot = Some(texture);
                let mut banner = Some(format!("Loaded screenshot {}", path.display()));
                if let Some(profile) = &mut self.profile {
                    match &mut profile.screen {
                        Some(screen) => {
                            if screen.width != size[0] as u32 || screen.height != size[1] as u32 {
                                banner = Some(format!(
                                    "Screenshot is {}x{}, profile is locked to {}x{}",
                                    size[0], size[1], screen.width, screen.height
                                ));
                            }
                        }
                        None => {
                            profile.screen = Some(ScreenOverride {
                                width: size[0] as u32,
                                height: size[1] as u32,
                            });
                            self.dirty = true;
                        }
                    }
                }
                if let Some(message) = banner {
                    self.set_banner(message, false);
                }
            }
            Err(e) => self.set_banner(format!("Image load failed: {}", e), true),
        }
    }

    fn refresh_status(&mut self) {
        match ipc::send_command_blocking(&IpcRequest::Status) {
            Ok(response) => {
                self.apply_status(&response);
                self.runtime.connected = true;
                self.runtime.last_error = None;
                self.runtime.last_checked = Some(Instant::now());
            }
            Err(e) => {
                self.runtime.connected = false;
                self.runtime.last_error = Some(e.to_string());
                self.runtime.last_checked = Some(Instant::now());
            }
        }
    }

    fn apply_status(&mut self, response: &IpcResponse) {
        self.runtime.profile = response.profile.clone();
        self.runtime.paused = response.paused.unwrap_or(false);
        self.runtime.capture_active = response.capture_active.unwrap_or(false);
        self.runtime.screen = match (response.screen_width, response.screen_height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        };
    }

    fn push_profile_live(&mut self) {
        let Some(profile) = self.profile.clone() else {
            self.set_banner("Open or create a profile first", true);
            return;
        };
        if let Err(e) = profile.validate() {
            self.set_banner(format!("Cannot push invalid profile: {}", e), true);
            return;
        }
        match ipc::send_command_blocking(&IpcRequest::LoadProfileData { profile }) {
            Ok(response) if response.ok => {
                self.apply_status(&response);
                self.runtime.connected = true;
                self.runtime.last_error = None;
                self.runtime.last_checked = Some(Instant::now());
                self.set_banner("Live profile pushed to daemon", false);
            }
            Ok(response) => {
                self.runtime.connected = false;
                self.runtime.last_error = response.error.clone();
                self.set_banner(
                    response
                        .error
                        .unwrap_or_else(|| "daemon rejected profile".into()),
                    true,
                );
            }
            Err(e) => {
                self.runtime.connected = false;
                self.runtime.last_error = Some(e.to_string());
                self.set_banner(format!("Push failed: {}", e), true);
            }
        }
    }

    fn send_runtime_request(&mut self, request: IpcRequest, success_message: &'static str) {
        match ipc::send_command_blocking(&request) {
            Ok(response) if response.ok => {
                self.apply_status(&response);
                self.runtime.connected = true;
                self.runtime.last_error = None;
                self.runtime.last_checked = Some(Instant::now());
                self.set_banner(
                    response
                        .message
                        .unwrap_or_else(|| success_message.to_string()),
                    false,
                );
            }
            Ok(response) => {
                let error = response
                    .error
                    .unwrap_or_else(|| "daemon request failed".into());
                self.runtime.connected = false;
                self.runtime.last_error = Some(error.clone());
                self.set_banner(error, true);
            }
            Err(e) => {
                self.runtime.connected = false;
                self.runtime.last_error = Some(e.to_string());
                self.set_banner(format!("Daemon request failed: {}", e), true);
            }
        }
    }

    fn maybe_poll_runtime(&mut self) {
        let should_poll = self
            .runtime
            .last_checked
            .map(|t| t.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if should_poll {
            self.refresh_status();
        }
    }

    fn next_slot(&self) -> Option<u8> {
        let profile = self.profile.as_ref()?;
        (0..=9).find(|slot| !profile.nodes.iter().any(|node| node.slot() == Some(*slot)))
    }

    fn add_non_canvas_node(&mut self, template: NodeTemplate) {
        let Some(profile) = &mut self.profile else {
            self.set_banner("Open or create a profile first", true);
            return;
        };

        let node = match template {
            NodeTemplate::Macro => Node::Macro {
                id: format!("macro_{}", profile.nodes.len() + 1),
                layer: String::new(),
                key: "G".into(),
                sequence: vec![
                    MacroStep {
                        action: MacroAction::Down,
                        pos: Some(RelPos { x: 0.5, y: 0.5 }),
                        slot: 0,
                        delay_ms: 0,
                    },
                    MacroStep {
                        action: MacroAction::Up,
                        pos: None,
                        slot: 0,
                        delay_ms: 30,
                    },
                ],
            },
            NodeTemplate::LayerShift => Node::LayerShift {
                id: format!("layer_{}", profile.nodes.len() + 1),
                key: "LeftAlt".into(),
                layer_name: "combat".into(),
                mode: LayerMode::Hold,
            },
            _ => return,
        };
        profile.nodes.push(node);
        self.selected = Some(profile.nodes.len() - 1);
        self.dirty = true;
    }

    fn place_node(&mut self, template: NodeTemplate, rel: RelPos) {
        let Some(slot) = self.next_slot() else {
            self.set_banner("All 10 touch slots are already assigned", true);
            return;
        };
        let Some(profile) = &mut self.profile else {
            return;
        };
        let node = match template {
            NodeTemplate::Tap => Node::Tap {
                id: format!("tap_{}", slot),
                layer: String::new(),
                slot,
                pos: rel,
                key: "Space".into(),
            },
            NodeTemplate::HoldTap => Node::HoldTap {
                id: format!("hold_{}", slot),
                layer: String::new(),
                slot,
                pos: rel,
                key: "MouseLeft".into(),
            },
            NodeTemplate::ToggleTap => Node::ToggleTap {
                id: format!("toggle_{}", slot),
                layer: String::new(),
                slot,
                pos: rel,
                key: "Q".into(),
            },
            NodeTemplate::Joystick => Node::Joystick {
                id: format!("stick_{}", slot),
                layer: String::new(),
                slot,
                pos: rel,
                radius: 0.08,
                keys: JoystickKeys {
                    up: "W".into(),
                    down: "S".into(),
                    left: "A".into(),
                    right: "D".into(),
                },
            },
            NodeTemplate::MouseLook => {
                let width = 0.55;
                let height = 1.0;
                let x = (rel.x - width / 2.0).clamp(0.0, 1.0 - width);
                let y = 0.0;
                Node::MouseCamera {
                    id: format!("look_{}", slot),
                    layer: String::new(),
                    slot,
                    region: Region {
                        x,
                        y,
                        w: width,
                        h: height,
                    },
                    sensitivity: 1.0,
                    invert_y: false,
                }
            }
            NodeTemplate::RepeatTap => Node::RepeatTap {
                id: format!("rapid_{}", slot),
                layer: String::new(),
                slot,
                pos: rel,
                key: "F".into(),
                interval_ms: 90,
            },
            NodeTemplate::Macro | NodeTemplate::LayerShift => return,
        };
        profile.nodes.push(node);
        self.selected = Some(profile.nodes.len() - 1);
        self.dirty = true;
        self.tool = Tool::Select;
    }

    fn delete_selected(&mut self) {
        let Some(idx) = self.selected else {
            return;
        };
        let Some(profile) = &mut self.profile else {
            return;
        };
        if idx < profile.nodes.len() {
            profile.nodes.remove(idx);
            self.selected = None;
            self.drag_state = None;
            self.pending_binding = None;
            self.dirty = true;
        }
    }

    fn begin_binding_capture(&mut self, target: BindingTarget) {
        self.pending_binding = Some(target);
        self.pending_binding_started_at = Some(Instant::now());
    }

    fn handle_binding_capture(&mut self, ctx: &egui::Context) {
        let Some(target) = self.pending_binding.clone() else {
            return;
        };
        if self
            .pending_binding_started_at
            .is_some_and(|started| started.elapsed() < Duration::from_millis(150))
        {
            return;
        }

        let mut captured = None;
        let mut cancelled = false;
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => {
                        if *key == egui::Key::Escape {
                            cancelled = true;
                            break;
                        }
                        if let Some(name) = egui_key_to_binding(*key) {
                            captured = Some(name.to_string());
                            break;
                        }
                    }
                    egui::Event::PointerButton {
                        button,
                        pressed: true,
                        ..
                    } => {
                        if let Some(name) = pointer_button_to_binding(*button) {
                            captured = Some(name.to_string());
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });

        if cancelled {
            self.pending_binding = None;
            self.pending_binding_started_at = None;
            self.set_banner("Binding capture cancelled", false);
            return;
        }

        if let Some(binding) = captured {
            if self.apply_binding(&target, binding.clone()) {
                self.pending_binding = None;
                self.pending_binding_started_at = None;
                self.dirty = true;
                self.set_banner(format!("Bound {}", binding), false);
            }
        }
    }

    fn apply_binding(&mut self, target: &BindingTarget, binding: String) -> bool {
        let Some(profile) = &mut self.profile else {
            return false;
        };
        match target {
            BindingTarget::Primary(idx) => {
                let Some(node) = profile.nodes.get_mut(*idx) else {
                    return false;
                };
                match node {
                    Node::Tap { key, .. }
                    | Node::HoldTap { key, .. }
                    | Node::ToggleTap { key, .. }
                    | Node::RepeatTap { key, .. }
                    | Node::Macro { key, .. }
                    | Node::LayerShift { key, .. } => {
                        *key = binding;
                        true
                    }
                    Node::Joystick { .. } | Node::MouseCamera { .. } => false,
                }
            }
            BindingTarget::Joystick { idx, dir } => {
                let Some(node) = profile.nodes.get_mut(*idx) else {
                    return false;
                };
                let Node::Joystick { keys, .. } = node else {
                    return false;
                };
                match dir {
                    JoyDir::Up => keys.up = binding,
                    JoyDir::Down => keys.down = binding,
                    JoyDir::Left => keys.left = binding,
                    JoyDir::Right => keys.right = binding,
                }
                true
            }
        }
    }

    fn content_rect(&self, canvas: Rect) -> Rect {
        let aspect = self
            .screenshot
            .as_ref()
            .map(|tex| {
                let size = tex.size();
                size[0] as f32 / size[1].max(1) as f32
            })
            .or_else(|| {
                self.profile.as_ref().and_then(|profile| {
                    profile
                        .screen
                        .as_ref()
                        .map(|screen| screen.width as f32 / screen.height.max(1) as f32)
                })
            })
            .unwrap_or(16.0 / 9.0);

        fit_rect_to_aspect(canvas, aspect)
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_directory(config::profiles_dir())
                .pick_file()
            {
                self.load_profile(&path);
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.new_profile();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.save_profile_as();
            } else {
                self.save_profile();
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R)) {
            self.push_profile_live();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.delete_selected();
        }
    }

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar")
            .exact_height(82.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

                    if ui.button("New").clicked() {
                        self.new_profile();
                    }
                    if ui.button("Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_directory(config::profiles_dir())
                            .pick_file()
                        {
                            self.load_profile(&path);
                        }
                    }
                    if ui.button("Save").clicked() {
                        self.save_profile();
                    }
                    if ui.button("Save As").clicked() {
                        self.save_profile_as();
                    }
                    if ui.button("Screenshot").clicked() {
                        self.load_screenshot(ctx);
                    }
                    ui.separator();

                    tool_button(ui, &mut self.tool, Tool::Select, "Select");
                    tool_button(ui, &mut self.tool, Tool::Place(NodeTemplate::Tap), "Tap");
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::HoldTap),
                        "Hold",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::ToggleTap),
                        "Toggle",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::Joystick),
                        "Left Stick",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::MouseLook),
                        "Mouse Look",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::RepeatTap),
                        "Rapid Tap",
                    );

                    if ui.button("Add Macro").clicked() {
                        self.add_non_canvas_node(NodeTemplate::Macro);
                    }
                    if ui.button("Add Layer").clicked() {
                        self.add_non_canvas_node(NodeTemplate::LayerShift);
                    }
                    ui.separator();

                    if ui.button("Push Live").clicked() {
                        self.push_profile_live();
                    }
                    if ui.button("Refresh").clicked() {
                        self.refresh_status();
                    }
                    if ui.button("Pause").clicked() {
                        self.send_runtime_request(IpcRequest::Pause, "paused");
                    }
                    if ui.button("Resume").clicked() {
                        self.send_runtime_request(IpcRequest::Resume, "resumed");
                    }
                    if ui.button("Enter Capture").clicked() {
                        self.send_runtime_request(IpcRequest::EnterCapture, "capture enabled");
                    }
                    if ui.button("Exit Capture").clicked() {
                        self.send_runtime_request(IpcRequest::ExitCapture, "capture disabled");
                    }
                    if ui.button("Toggle Capture").clicked() {
                        self.send_runtime_request(IpcRequest::ToggleCapture, "capture toggled");
                    }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let profile_name = self
                        .profile
                        .as_ref()
                        .map(|p| p.name.as_str())
                        .unwrap_or("No profile");
                    let banner_text = self
                        .banner
                        .as_ref()
                        .map(|b| b.text.as_str())
                        .unwrap_or("Ctrl+O open, Ctrl+S save, Ctrl+R push live");
                    let banner_color = self
                        .banner
                        .as_ref()
                        .map(|b| {
                            if b.is_error {
                                Color32::from_rgb(255, 180, 180)
                            } else {
                                Color32::from_rgb(182, 255, 216)
                            }
                        })
                        .unwrap_or(Color32::from_gray(180));
                    ui.label(RichText::new(profile_name).strong().size(16.0));
                    if self.dirty {
                        ui.label(RichText::new("unsaved").color(Color32::YELLOW));
                    }
                    ui.separator();
                    ui.label(RichText::new(banner_text).color(banner_color));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        runtime_chip(ui, self.runtime.capture_active, "Capture");
                        runtime_chip(ui, !self.runtime.paused, "Active");
                        runtime_chip(ui, self.runtime.connected, "Daemon");
                    });
                });
            });
    }

    fn draw_left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(290.0)
            .show(ctx, |ui| {
                ui.heading("Profile");
                ui.add_space(6.0);

                let default_screen = self.default_screen();
                if let Some(profile) = &mut self.profile {
                    if ui.text_edit_singleline(&mut profile.name).changed() {
                        self.dirty = true;
                    }

                    let screen = profile.screen.get_or_insert(default_screen);
                    ui.horizontal(|ui| {
                        ui.label("Screen");
                        let mut width = screen.width as f64;
                        let mut height = screen.height as f64;
                        if ui
                            .add(egui::DragValue::new(&mut width).range(320.0..=8000.0))
                            .changed()
                        {
                            screen.width = width as u32;
                            self.dirty = true;
                        }
                        ui.label("x");
                        if ui
                            .add(egui::DragValue::new(&mut height).range(320.0..=8000.0))
                            .changed()
                        {
                            screen.height = height as u32;
                            self.dirty = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Sensitivity");
                        let mut value = profile.global_sensitivity;
                        if ui
                            .add(
                                egui::DragValue::new(&mut value)
                                    .speed(0.01)
                                    .range(0.05..=10.0),
                            )
                            .changed()
                        {
                            profile.global_sensitivity = (value * 100.0).round() / 100.0;
                            self.dirty = true;
                        }
                    });
                    ui.checkbox(&mut self.show_labels, "Show canvas labels");
                    ui.checkbox(&mut self.auto_push_on_save, "Push live after save");

                    ui.separator();
                    ui.heading("Controls");
                    ui.add_space(6.0);

                    let mut clicked = None;
                    for (idx, node) in profile.nodes.iter().enumerate() {
                        let selected = self.selected == Some(idx);
                        let text = format!("{}  {}", display_binding(node), display_type(node));
                        let subtitle = match (node.slot(), node.layer().trim().is_empty()) {
                            (Some(slot), true) => format!("slot {}", slot),
                            (Some(slot), false) => {
                                format!("slot {} • layer {}", slot, node.layer())
                            }
                            (None, true) => "runtime".into(),
                            (None, false) => format!("layer {}", node.layer()),
                        };

                        ui.group(|ui| {
                            let response = ui.selectable_label(
                                selected,
                                RichText::new(text).color(node_color(node)).strong(),
                            );
                            if response.clicked() {
                                clicked = Some(idx);
                            }
                            ui.label(RichText::new(subtitle).small().weak());
                            if !node.id().is_empty() {
                                ui.label(RichText::new(node.id()).small().italics().weak());
                            }
                        });
                        ui.add_space(4.0);
                    }
                    if let Some(idx) = clicked {
                        self.selected = Some(idx);
                    }
                } else {
                    ui.label("Create or open a profile to start mapping.");
                }

                ui.separator();
                ui.heading("Runtime");
                ui.add_space(6.0);
                let daemon_text = if self.runtime.connected {
                    "Connected"
                } else {
                    "Disconnected"
                };
                ui.label(RichText::new(daemon_text).color(if self.runtime.connected {
                    Color32::LIGHT_GREEN
                } else {
                    Color32::LIGHT_RED
                }));
                if let Some((w, h)) = self.runtime.screen {
                    ui.label(format!("Daemon screen: {}x{}", w, h));
                }
                if let Some(profile) = &self.runtime.profile {
                    ui.label(format!("Loaded: {}", profile));
                }
                ui.label(format!(
                    "Capture: {}",
                    if self.runtime.capture_active {
                        "on"
                    } else {
                        "off"
                    }
                ));
                ui.label(format!(
                    "Processing: {}",
                    if self.runtime.paused {
                        "paused"
                    } else {
                        "live"
                    }
                ));
                if let Some(error) = &self.runtime.last_error {
                    ui.label(RichText::new(error).small().color(Color32::LIGHT_RED));
                }
            });
    }

    fn draw_properties_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("properties_panel")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.add_space(6.0);

                let Some(profile) = &mut self.profile else {
                    ui.label("No profile loaded");
                    return;
                };
                let Some(idx) = self.selected else {
                    ui.label("Select a control from the list or canvas");
                    return;
                };
                if idx >= profile.nodes.len() {
                    ui.label("Selection is out of date");
                    return;
                }

                let mut start_binding = None;
                let mut delete_current = false;
                let mut dirty = false;

                let node = &mut profile.nodes[idx];
                ui.label(
                    RichText::new(display_type(node))
                        .strong()
                        .color(node_color(node)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Id");
                    let mut id = node.id().to_string();
                    if ui.text_edit_singleline(&mut id).changed() {
                        set_node_id(node, id);
                        dirty = true;
                    }
                });

                if let Some(slot) = node.slot() {
                    ui.label(format!("Touch slot {}", slot));
                } else {
                    ui.label("No touch slot");
                }

                match node {
                    Node::Tap { layer, pos, .. }
                    | Node::HoldTap { layer, pos, .. }
                    | Node::ToggleTap { layer, pos, .. }
                    | Node::RepeatTap { layer, pos, .. } => {
                        layer_row(ui, layer, &mut dirty);
                        position_editor(ui, pos, &mut dirty);
                    }
                    Node::Joystick {
                        layer, pos, radius, ..
                    } => {
                        layer_row(ui, layer, &mut dirty);
                        position_editor(ui, pos, &mut dirty);
                        ui.horizontal(|ui| {
                            ui.label("Radius");
                            let mut value = *radius;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .speed(0.005)
                                        .range(0.02..=0.4),
                                )
                                .changed()
                            {
                                *radius = (value * 1000.0).round() / 1000.0;
                                dirty = true;
                            }
                        });
                    }
                    Node::MouseCamera { layer, region, .. } => {
                        layer_row(ui, layer, &mut dirty);
                        region_editor(ui, region, &mut dirty);
                    }
                    Node::Macro { layer, .. } => {
                        layer_row(ui, layer, &mut dirty);
                    }
                    Node::LayerShift { .. } => {}
                }

                ui.separator();
                match node {
                    Node::Tap { key, .. }
                    | Node::HoldTap { key, .. }
                    | Node::ToggleTap { key, .. }
                    | Node::RepeatTap { key, .. }
                    | Node::Macro { key, .. }
                    | Node::LayerShift { key, .. } => {
                        binding_picker(
                            ui,
                            "Binding",
                            key,
                            &BindingTarget::Primary(idx),
                            self.pending_binding.as_ref(),
                            &mut start_binding,
                            &mut dirty,
                        );
                    }
                    Node::Joystick { keys, .. } => {
                        joystick_binding_picker(
                            ui,
                            "Up",
                            &mut keys.up,
                            BindingTarget::Joystick {
                                idx,
                                dir: JoyDir::Up,
                            },
                            self.pending_binding.as_ref(),
                            &mut start_binding,
                            &mut dirty,
                        );
                        joystick_binding_picker(
                            ui,
                            "Down",
                            &mut keys.down,
                            BindingTarget::Joystick {
                                idx,
                                dir: JoyDir::Down,
                            },
                            self.pending_binding.as_ref(),
                            &mut start_binding,
                            &mut dirty,
                        );
                        joystick_binding_picker(
                            ui,
                            "Left",
                            &mut keys.left,
                            BindingTarget::Joystick {
                                idx,
                                dir: JoyDir::Left,
                            },
                            self.pending_binding.as_ref(),
                            &mut start_binding,
                            &mut dirty,
                        );
                        joystick_binding_picker(
                            ui,
                            "Right",
                            &mut keys.right,
                            BindingTarget::Joystick {
                                idx,
                                dir: JoyDir::Right,
                            },
                            self.pending_binding.as_ref(),
                            &mut start_binding,
                            &mut dirty,
                        );
                    }
                    Node::MouseCamera {
                        sensitivity,
                        invert_y,
                        ..
                    } => {
                        ui.horizontal(|ui| {
                            ui.label("Sensitivity");
                            let mut value = *sensitivity;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .speed(0.01)
                                        .range(0.05..=5.0),
                                )
                                .changed()
                            {
                                *sensitivity = (value * 100.0).round() / 100.0;
                                dirty = true;
                            }
                        });
                        if ui.checkbox(invert_y, "Invert Y").changed() {
                            dirty = true;
                        }
                    }
                }

                if let Node::RepeatTap { interval_ms, .. } = node {
                    ui.horizontal(|ui| {
                        ui.label("Interval ms");
                        let mut value = *interval_ms as f64;
                        if ui
                            .add(
                                egui::DragValue::new(&mut value)
                                    .speed(1.0)
                                    .range(16.0..=1000.0),
                            )
                            .changed()
                        {
                            *interval_ms = value as u64;
                            dirty = true;
                        }
                    });
                }

                if let Node::Macro { sequence, .. } = node {
                    ui.separator();
                    ui.label(RichText::new("Macro Steps").strong());
                    let mut remove_idx = None;
                    for (step_idx, step) in sequence.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("Step {}", step_idx + 1));
                                egui::ComboBox::from_id_salt(("macro_action", idx, step_idx))
                                    .selected_text(match step.action {
                                        MacroAction::Down => "Down",
                                        MacroAction::Up => "Up",
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                matches!(step.action, MacroAction::Down),
                                                "Down",
                                            )
                                            .clicked()
                                        {
                                            step.action = MacroAction::Down;
                                            if step.pos.is_none() {
                                                step.pos = Some(RelPos { x: 0.5, y: 0.5 });
                                            }
                                            dirty = true;
                                        }
                                        if ui
                                            .selectable_label(
                                                matches!(step.action, MacroAction::Up),
                                                "Up",
                                            )
                                            .clicked()
                                        {
                                            step.action = MacroAction::Up;
                                            step.pos = None;
                                            dirty = true;
                                        }
                                    });
                                if ui.button("Remove").clicked() {
                                    remove_idx = Some(step_idx);
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Slot");
                                let mut slot = step.slot as f64;
                                if ui
                                    .add(egui::DragValue::new(&mut slot).range(0.0..=9.0))
                                    .changed()
                                {
                                    step.slot = slot as u8;
                                    dirty = true;
                                }
                                ui.label("Delay");
                                let mut delay = step.delay_ms as f64;
                                if ui
                                    .add(egui::DragValue::new(&mut delay).range(0.0..=5000.0))
                                    .changed()
                                {
                                    step.delay_ms = delay as u64;
                                    dirty = true;
                                }
                            });
                            if let Some(pos) = &mut step.pos {
                                position_editor(ui, pos, &mut dirty);
                            }
                        });
                    }
                    if let Some(remove_idx) = remove_idx {
                        sequence.remove(remove_idx);
                        dirty = true;
                    }
                    if ui.button("Add Step").clicked() {
                        sequence.push(MacroStep {
                            action: MacroAction::Down,
                            pos: Some(RelPos { x: 0.5, y: 0.5 }),
                            slot: 0,
                            delay_ms: 30,
                        });
                        dirty = true;
                    }
                }

                if let Node::LayerShift {
                    layer_name, mode, ..
                } = node
                {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Target layer");
                        if ui.text_edit_singleline(layer_name).changed() {
                            dirty = true;
                        }
                    });
                    egui::ComboBox::from_label("Mode")
                        .selected_text(match mode {
                            LayerMode::Hold => "Hold",
                            LayerMode::Toggle => "Toggle",
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(matches!(mode, LayerMode::Hold), "Hold")
                                .clicked()
                            {
                                *mode = LayerMode::Hold;
                                dirty = true;
                            }
                            if ui
                                .selectable_label(matches!(mode, LayerMode::Toggle), "Toggle")
                                .clicked()
                            {
                                *mode = LayerMode::Toggle;
                                dirty = true;
                            }
                        });
                }

                ui.separator();
                if ui.button("Delete Control").clicked() {
                    delete_current = true;
                }

                if let Some(target) = start_binding {
                    self.begin_binding_capture(target);
                }
                if delete_current {
                    self.delete_selected();
                    return;
                }
                if dirty {
                    self.dirty = true;
                }
            });
    }

    fn draw_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(_) = self.profile else {
                ui.centered_and_justified(|ui| {
                    ui.label("Create or open a profile to start mapping.");
                });
                return;
            };

            let response = ui.allocate_response(ui.available_size(), Sense::click_and_drag());
            let canvas = response.rect;
            let content = self.content_rect(canvas);
            let painter = ui.painter_at(canvas);

            painter.rect_filled(canvas, 0.0, CANVAS_BG);
            painter.rect_filled(content, 10.0, CONTENT_BG);
            if let Some(texture) = &self.screenshot {
                painter.image(
                    texture.id(),
                    content,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            draw_grid(&painter, content);

            if let Some(profile) = &self.profile {
                if let Some(screen) = &profile.screen {
                    painter.text(
                        Pos2::new(content.left() + 12.0, content.top() + 12.0),
                        Align2::LEFT_TOP,
                        format!("{}x{}", screen.width, screen.height),
                        egui::FontId::proportional(12.0),
                        Color32::from_white_alpha(160),
                    );
                }

                if !matches!(self.tool, Tool::Select) {
                    painter.text(
                        Pos2::new(content.center().x, content.top() + 20.0),
                        Align2::CENTER_TOP,
                        format!("Click on the canvas to place {}", tool_label(self.tool)),
                        egui::FontId::proportional(13.0),
                        Color32::from_rgb(255, 230, 145),
                    );
                }

                for (idx, node) in profile.nodes.iter().enumerate() {
                    draw_node(
                        &painter,
                        content,
                        node,
                        self.selected == Some(idx),
                        self.show_labels,
                    );
                }
            }

            let pointer_pos = response.interact_pointer_pos();

            if response.clicked() {
                if let Some(mouse) = pointer_pos {
                    if content.contains(mouse) {
                        match self.tool {
                            Tool::Place(template) => {
                                if matches!(
                                    template,
                                    NodeTemplate::Macro | NodeTemplate::LayerShift
                                ) {
                                    self.add_non_canvas_node(template);
                                } else {
                                    self.place_node(template, from_canvas_pos(content, mouse));
                                }
                            }
                            Tool::Select => {
                                if let Some(profile) = &self.profile {
                                    self.selected =
                                        hit_test(profile, content, mouse, self.selected)
                                            .map(|hit| hit.idx());
                                }
                            }
                        }
                    }
                }
            }

            if response.drag_started() {
                if let (Some(mouse), Some(profile)) = (pointer_pos, self.profile.as_ref()) {
                    self.drag_state = hit_test(profile, content, mouse, self.selected).and_then(
                        |hit| match hit {
                            HitTarget::Point(idx) => Some(DragState::Point { idx }),
                            HitTarget::Region(idx) => profile.nodes.get(idx).and_then(|node| {
                                let Node::MouseCamera { region, .. } = node else {
                                    return None;
                                };
                                Some(DragState::RegionMove {
                                    idx,
                                    origin: region.clone(),
                                    start_mouse: mouse,
                                })
                            }),
                            HitTarget::RegionHandle(idx, handle) => {
                                profile.nodes.get(idx).and_then(|node| {
                                    let Node::MouseCamera { region, .. } = node else {
                                        return None;
                                    };
                                    Some(DragState::RegionResize {
                                        idx,
                                        handle,
                                        origin: region.clone(),
                                        start_mouse: mouse,
                                    })
                                })
                            }
                        },
                    );
                    if let Some(drag) = &self.drag_state {
                        self.selected = Some(match drag {
                            DragState::Point { idx }
                            | DragState::RegionMove { idx, .. }
                            | DragState::RegionResize { idx, .. } => *idx,
                        });
                    }
                }
            }

            if response.dragged() {
                if let (Some(mouse), Some(profile)) = (pointer_pos, self.profile.as_mut()) {
                    if let Some(drag) = &self.drag_state {
                        match drag {
                            DragState::Point { idx } => {
                                if let Some(node) = profile.nodes.get_mut(*idx) {
                                    if let Some(pos) = node_pos_mut(node) {
                                        let rel = from_canvas_pos(content, mouse);
                                        pos.x = round3(rel.x);
                                        pos.y = round3(rel.y);
                                        self.dirty = true;
                                    }
                                }
                            }
                            DragState::RegionMove {
                                idx,
                                origin,
                                start_mouse,
                            } => {
                                let Some(Node::MouseCamera { region, .. }) =
                                    profile.nodes.get_mut(*idx)
                                else {
                                    return;
                                };
                                let delta = (mouse - *start_mouse)
                                    / Vec2::new(content.width(), content.height());
                                region.x = (origin.x + delta.x as f64).clamp(0.0, 1.0 - origin.w);
                                region.y = (origin.y + delta.y as f64).clamp(0.0, 1.0 - origin.h);
                                region.x = round3(region.x);
                                region.y = round3(region.y);
                                self.dirty = true;
                            }
                            DragState::RegionResize {
                                idx,
                                handle,
                                origin,
                                start_mouse,
                            } => {
                                let Some(Node::MouseCamera { region, .. }) =
                                    profile.nodes.get_mut(*idx)
                                else {
                                    return;
                                };
                                let delta = (mouse - *start_mouse)
                                    / Vec2::new(content.width(), content.height());
                                let mut next = origin.clone();
                                match handle {
                                    RegionHandle::TopLeft => {
                                        next.x = origin.x + delta.x as f64;
                                        next.y = origin.y + delta.y as f64;
                                        next.w = origin.w - delta.x as f64;
                                        next.h = origin.h - delta.y as f64;
                                    }
                                    RegionHandle::TopRight => {
                                        next.y = origin.y + delta.y as f64;
                                        next.w = origin.w + delta.x as f64;
                                        next.h = origin.h - delta.y as f64;
                                    }
                                    RegionHandle::BottomLeft => {
                                        next.x = origin.x + delta.x as f64;
                                        next.w = origin.w - delta.x as f64;
                                        next.h = origin.h + delta.y as f64;
                                    }
                                    RegionHandle::BottomRight => {
                                        next.w = origin.w + delta.x as f64;
                                        next.h = origin.h + delta.y as f64;
                                    }
                                }
                                *region = clamp_region(next);
                                self.dirty = true;
                            }
                        }
                    }
                }
            }

            if response.drag_stopped() {
                self.drag_state = None;
            }

            if let Some(target) = &self.pending_binding {
                let label = format!(
                    "Press a key or mouse button for {}",
                    binding_target_label(target)
                );
                let banner_rect = Rect::from_center_size(
                    Pos2::new(content.center().x, content.top() + 32.0),
                    Vec2::new(320.0, 34.0),
                );
                painter.rect_filled(banner_rect, 8.0, Color32::from_black_alpha(190));
                painter.text(
                    banner_rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(13.0),
                    Color32::WHITE,
                );
            }
        });
    }
}

impl eframe::App for PhantomGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);
        self.handle_binding_capture(ctx);
        self.maybe_poll_runtime();
        ctx.request_repaint_after(Duration::from_millis(250));

        self.draw_top_bar(ctx);
        self.draw_left_panel(ctx);
        self.draw_properties_panel(ctx);
        self.draw_canvas(ctx);
    }
}

#[derive(Clone, Copy)]
enum HitTarget {
    Point(usize),
    Region(usize),
    RegionHandle(usize, RegionHandle),
}

impl HitTarget {
    fn idx(self) -> usize {
        match self {
            HitTarget::Point(idx) | HitTarget::Region(idx) | HitTarget::RegionHandle(idx, _) => idx,
        }
    }
}

fn hit_test(
    profile: &Profile,
    content: Rect,
    mouse: Pos2,
    selected: Option<usize>,
) -> Option<HitTarget> {
    if let Some(idx) = selected {
        if let Some(Node::MouseCamera { region, .. }) = profile.nodes.get(idx) {
            let rect = region_rect(content, region);
            for (handle, handle_rect) in region_handles(rect) {
                if handle_rect.contains(mouse) {
                    return Some(HitTarget::RegionHandle(idx, handle));
                }
            }
            if rect.contains(mouse) {
                return Some(HitTarget::Region(idx));
            }
        }
    }

    let mut best = None;
    for (idx, node) in profile.nodes.iter().enumerate() {
        if let Some(pos) = node_pos(node) {
            let point = to_canvas_pos(content, pos);
            let distance = (point - mouse).length();
            if distance <= 22.0 && best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((idx, distance));
            }
        } else if let Node::MouseCamera { region, .. } = node {
            if region_rect(content, region).contains(mouse) {
                return Some(HitTarget::Region(idx));
            }
        }
    }
    best.map(|(idx, _)| HitTarget::Point(idx))
}

fn draw_node(
    painter: &egui::Painter,
    content: Rect,
    node: &Node,
    selected: bool,
    show_labels: bool,
) {
    let color = node_color(node);
    if let Node::MouseCamera { region, .. } = node {
        let rect = region_rect(content, region);
        painter.rect_filled(rect, 8.0, color.gamma_multiply(0.08));
        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(if selected { 3.0 } else { 2.0 }, color),
            StrokeKind::Outside,
        );
        if selected {
            for (_, handle_rect) in region_handles(rect) {
                painter.rect_filled(handle_rect, 2.0, Color32::WHITE);
            }
        }
        painter.text(
            rect.center_top() + Vec2::new(0.0, 12.0),
            Align2::CENTER_TOP,
            display_binding(node),
            egui::FontId::proportional(13.0),
            color,
        );
        return;
    }

    let Some(pos) = node_pos(node) else {
        return;
    };
    let point = to_canvas_pos(content, pos);

    if let Node::Joystick { radius, .. } = node {
        let radius_px = (*radius as f32 * content.width()).max(16.0);
        painter.circle_stroke(
            point,
            radius_px,
            Stroke::new(2.0, color.gamma_multiply(0.65)),
        );
    }

    let marker_radius = if selected { 16.0 } else { 12.0 };
    painter.circle_filled(point, marker_radius, color);
    if selected {
        painter.circle_stroke(point, marker_radius + 3.0, Stroke::new(2.0, Color32::WHITE));
    }
    painter.text(
        point,
        Align2::CENTER_CENTER,
        marker_glyph(node),
        egui::FontId::proportional(12.0),
        Color32::BLACK,
    );
    if show_labels {
        painter.text(
            point + Vec2::new(0.0, marker_radius + 6.0),
            Align2::CENTER_TOP,
            display_binding(node),
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
}

fn display_type(node: &Node) -> &'static str {
    match node {
        Node::Tap { .. } => "Tap Button",
        Node::HoldTap { .. } => "Hold Button",
        Node::ToggleTap { .. } => "Toggle Button",
        Node::Joystick { .. } => "Left Stick",
        Node::MouseCamera { .. } => "Mouse Look",
        Node::RepeatTap { .. } => "Rapid Tap",
        Node::Macro { .. } => "Macro",
        Node::LayerShift { .. } => "Layer Switch",
    }
}

fn display_binding(node: &Node) -> String {
    match node {
        Node::Tap { key, .. }
        | Node::HoldTap { key, .. }
        | Node::ToggleTap { key, .. }
        | Node::RepeatTap { key, .. }
        | Node::Macro { key, .. }
        | Node::LayerShift { key, .. } => key.clone(),
        Node::Joystick { keys, .. } => {
            format!("{}/{}/{}/{}", keys.up, keys.left, keys.down, keys.right)
        }
        Node::MouseCamera { .. } => "Mouse Look".into(),
    }
}

fn node_color(node: &Node) -> Color32 {
    match node {
        Node::Tap { .. } => COLOR_TAP,
        Node::HoldTap { .. } => COLOR_HOLD,
        Node::ToggleTap { .. } => COLOR_TOGGLE,
        Node::Joystick { .. } => COLOR_JOYSTICK,
        Node::MouseCamera { .. } => COLOR_LOOK,
        Node::RepeatTap { .. } => COLOR_REPEAT,
        Node::Macro { .. } => COLOR_MACRO,
        Node::LayerShift { .. } => COLOR_LAYER,
    }
}

fn marker_glyph(node: &Node) -> &'static str {
    match node {
        Node::Tap { .. } => "T",
        Node::HoldTap { .. } => "H",
        Node::ToggleTap { .. } => "G",
        Node::Joystick { .. } => "J",
        Node::MouseCamera { .. } => "M",
        Node::RepeatTap { .. } => "R",
        Node::Macro { .. } => "C",
        Node::LayerShift { .. } => "L",
    }
}

fn node_pos(node: &Node) -> Option<&RelPos> {
    match node {
        Node::Tap { pos, .. }
        | Node::HoldTap { pos, .. }
        | Node::ToggleTap { pos, .. }
        | Node::Joystick { pos, .. }
        | Node::RepeatTap { pos, .. } => Some(pos),
        Node::MouseCamera { .. } | Node::Macro { .. } | Node::LayerShift { .. } => None,
    }
}

fn node_pos_mut(node: &mut Node) -> Option<&mut RelPos> {
    match node {
        Node::Tap { pos, .. }
        | Node::HoldTap { pos, .. }
        | Node::ToggleTap { pos, .. }
        | Node::Joystick { pos, .. }
        | Node::RepeatTap { pos, .. } => Some(pos),
        Node::MouseCamera { .. } | Node::Macro { .. } | Node::LayerShift { .. } => None,
    }
}

fn set_node_id(node: &mut Node, id: String) {
    match node {
        Node::Tap { id: field, .. }
        | Node::HoldTap { id: field, .. }
        | Node::ToggleTap { id: field, .. }
        | Node::Joystick { id: field, .. }
        | Node::MouseCamera { id: field, .. }
        | Node::RepeatTap { id: field, .. }
        | Node::Macro { id: field, .. }
        | Node::LayerShift { id: field, .. } => *field = id,
    }
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Select",
        Tool::Place(NodeTemplate::Tap) => "tap button",
        Tool::Place(NodeTemplate::HoldTap) => "hold button",
        Tool::Place(NodeTemplate::ToggleTap) => "toggle button",
        Tool::Place(NodeTemplate::Joystick) => "left stick",
        Tool::Place(NodeTemplate::MouseLook) => "mouse look region",
        Tool::Place(NodeTemplate::RepeatTap) => "rapid tap button",
        Tool::Place(NodeTemplate::Macro) => "macro",
        Tool::Place(NodeTemplate::LayerShift) => "layer switch",
    }
}

fn binding_target_label(target: &BindingTarget) -> &'static str {
    match target {
        BindingTarget::Primary(_) => "control",
        BindingTarget::Joystick { dir, .. } => match dir {
            JoyDir::Up => "stick up",
            JoyDir::Down => "stick down",
            JoyDir::Left => "stick left",
            JoyDir::Right => "stick right",
        },
    }
}

fn tool_button(ui: &mut egui::Ui, current: &mut Tool, tool: Tool, label: &str) {
    let selected = *current == tool;
    if ui.selectable_label(selected, label).clicked() {
        *current = tool;
    }
}

fn runtime_chip(ui: &mut egui::Ui, active: bool, label: &str) {
    let color = if active {
        Color32::from_rgb(88, 214, 141)
    } else {
        Color32::from_rgb(231, 76, 60)
    };
    ui.label(
        RichText::new(label)
            .background_color(color.gamma_multiply(0.22))
            .color(color),
    );
}

fn draw_grid(painter: &egui::Painter, rect: Rect) {
    for i in 1..10 {
        let factor = i as f32 / 10.0;
        let color = Color32::from_white_alpha(18);
        painter.line_segment(
            [
                to_canvas_pos(
                    rect,
                    &RelPos {
                        x: factor as f64,
                        y: 0.0,
                    },
                ),
                to_canvas_pos(
                    rect,
                    &RelPos {
                        x: factor as f64,
                        y: 1.0,
                    },
                ),
            ],
            Stroke::new(1.0, color),
        );
        painter.line_segment(
            [
                to_canvas_pos(
                    rect,
                    &RelPos {
                        x: 0.0,
                        y: factor as f64,
                    },
                ),
                to_canvas_pos(
                    rect,
                    &RelPos {
                        x: 1.0,
                        y: factor as f64,
                    },
                ),
            ],
            Stroke::new(1.0, color),
        );
    }
}

fn position_editor(ui: &mut egui::Ui, pos: &mut RelPos, dirty: &mut bool) {
    ui.horizontal(|ui| {
        ui.label("X");
        let mut x = pos.x;
        if ui
            .add(egui::DragValue::new(&mut x).speed(0.001).range(0.0..=1.0))
            .changed()
        {
            pos.x = round3(x);
            *dirty = true;
        }
        ui.label("Y");
        let mut y = pos.y;
        if ui
            .add(egui::DragValue::new(&mut y).speed(0.001).range(0.0..=1.0))
            .changed()
        {
            pos.y = round3(y);
            *dirty = true;
        }
    });
}

fn region_editor(ui: &mut egui::Ui, region: &mut Region, dirty: &mut bool) {
    for (label, value) in [
        ("X", &mut region.x),
        ("Y", &mut region.y),
        ("W", &mut region.w),
        ("H", &mut region.h),
    ] {
        ui.horizontal(|ui| {
            ui.label(label);
            if ui
                .add(egui::DragValue::new(value).speed(0.001).range(0.0..=1.0))
                .changed()
            {
                *dirty = true;
            }
        });
    }
    *region = clamp_region(region.clone());
}

fn layer_row(ui: &mut egui::Ui, layer: &mut String, dirty: &mut bool) {
    ui.horizontal(|ui| {
        ui.label("Layer");
        if ui.text_edit_singleline(layer).changed() {
            *dirty = true;
        }
    });
}

fn binding_picker(
    ui: &mut egui::Ui,
    label: &str,
    key: &mut String,
    target: &BindingTarget,
    pending: Option<&BindingTarget>,
    start_binding: &mut Option<BindingTarget>,
    dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let capture_text = if pending == Some(target) {
            "Press key..."
        } else {
            "Capture"
        };
        if ui.button(capture_text).clicked() {
            *start_binding = Some(target.clone());
        }
        egui::ComboBox::from_id_salt(("binding", label, key.as_str()))
            .selected_text(key.as_str())
            .show_ui(ui, |ui| {
                for candidate in BINDABLE_KEYS {
                    if ui
                        .selectable_label(*key == *candidate, *candidate)
                        .clicked()
                    {
                        *key = (*candidate).to_string();
                        *dirty = true;
                    }
                }
            });
    });
}

fn joystick_binding_picker(
    ui: &mut egui::Ui,
    label: &str,
    key: &mut String,
    target: BindingTarget,
    pending: Option<&BindingTarget>,
    start_binding: &mut Option<BindingTarget>,
    dirty: &mut bool,
) {
    binding_picker(ui, label, key, &target, pending, start_binding, dirty);
}

fn to_canvas_pos(rect: Rect, pos: &RelPos) -> Pos2 {
    Pos2::new(
        rect.left() + rect.width() * pos.x as f32,
        rect.top() + rect.height() * pos.y as f32,
    )
}

fn from_canvas_pos(rect: Rect, pos: Pos2) -> RelPos {
    RelPos {
        x: ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64,
        y: ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0) as f64,
    }
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

fn region_handles(rect: Rect) -> [(RegionHandle, Rect); 4] {
    [
        (
            RegionHandle::TopLeft,
            Rect::from_center_size(rect.left_top(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::TopRight,
            Rect::from_center_size(rect.right_top(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::BottomLeft,
            Rect::from_center_size(rect.left_bottom(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::BottomRight,
            Rect::from_center_size(rect.right_bottom(), Vec2::splat(HANDLE_SIZE)),
        ),
    ]
}

fn clamp_region(mut region: Region) -> Region {
    region.w = region.w.clamp(0.05, 1.0);
    region.h = region.h.clamp(0.05, 1.0);
    region.x = region.x.clamp(0.0, 1.0 - region.w);
    region.y = region.y.clamp(0.0, 1.0 - region.h);
    region.x = round3(region.x);
    region.y = round3(region.y);
    region.w = round3(region.w);
    region.h = round3(region.h);
    region
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn fit_rect_to_aspect(container: Rect, aspect: f32) -> Rect {
    let container_aspect = container.width() / container.height().max(1.0);
    if (container_aspect - aspect).abs() < f32::EPSILON {
        return container;
    }

    if container_aspect > aspect {
        let width = container.height() * aspect;
        let x = container.center().x - width / 2.0;
        Rect::from_min_size(
            Pos2::new(x, container.top()),
            Vec2::new(width, container.height()),
        )
    } else {
        let height = container.width() / aspect.max(0.0001);
        let y = container.center().y - height / 2.0;
        Rect::from_min_size(
            Pos2::new(container.left(), y),
            Vec2::new(container.width(), height),
        )
    }
}

fn pointer_button_to_binding(button: egui::PointerButton) -> Option<&'static str> {
    match button {
        egui::PointerButton::Primary => Some("MouseLeft"),
        egui::PointerButton::Secondary => Some("MouseRight"),
        egui::PointerButton::Middle => Some("MouseMiddle"),
        egui::PointerButton::Extra1 => Some("MouseBack"),
        egui::PointerButton::Extra2 => Some("MouseForward"),
    }
}

fn egui_key_to_binding(key: egui::Key) -> Option<&'static str> {
    match key {
        egui::Key::A => Some("A"),
        egui::Key::B => Some("B"),
        egui::Key::C => Some("C"),
        egui::Key::D => Some("D"),
        egui::Key::E => Some("E"),
        egui::Key::F => Some("F"),
        egui::Key::G => Some("G"),
        egui::Key::H => Some("H"),
        egui::Key::I => Some("I"),
        egui::Key::J => Some("J"),
        egui::Key::K => Some("K"),
        egui::Key::L => Some("L"),
        egui::Key::M => Some("M"),
        egui::Key::N => Some("N"),
        egui::Key::O => Some("O"),
        egui::Key::P => Some("P"),
        egui::Key::Q => Some("Q"),
        egui::Key::R => Some("R"),
        egui::Key::S => Some("S"),
        egui::Key::T => Some("T"),
        egui::Key::U => Some("U"),
        egui::Key::V => Some("V"),
        egui::Key::W => Some("W"),
        egui::Key::X => Some("X"),
        egui::Key::Y => Some("Y"),
        egui::Key::Z => Some("Z"),
        egui::Key::Num0 => Some("0"),
        egui::Key::Num1 => Some("1"),
        egui::Key::Num2 => Some("2"),
        egui::Key::Num3 => Some("3"),
        egui::Key::Num4 => Some("4"),
        egui::Key::Num5 => Some("5"),
        egui::Key::Num6 => Some("6"),
        egui::Key::Num7 => Some("7"),
        egui::Key::Num8 => Some("8"),
        egui::Key::Num9 => Some("9"),
        egui::Key::ArrowDown => Some("Down"),
        egui::Key::ArrowLeft => Some("Left"),
        egui::Key::ArrowRight => Some("Right"),
        egui::Key::ArrowUp => Some("Up"),
        egui::Key::Escape => Some("Esc"),
        egui::Key::Tab => Some("Tab"),
        egui::Key::Backspace => Some("Backspace"),
        egui::Key::Enter => Some("Enter"),
        egui::Key::Space => Some("Space"),
        egui::Key::Insert => Some("Insert"),
        egui::Key::Delete => Some("Delete"),
        egui::Key::Home => Some("Home"),
        egui::Key::End => Some("End"),
        egui::Key::PageUp => Some("PageUp"),
        egui::Key::PageDown => Some("PageDown"),
        egui::Key::F1 => Some("F1"),
        egui::Key::F2 => Some("F2"),
        egui::Key::F3 => Some("F3"),
        egui::Key::F4 => Some("F4"),
        egui::Key::F5 => Some("F5"),
        egui::Key::F6 => Some("F6"),
        egui::Key::F7 => Some("F7"),
        egui::Key::F8 => Some("F8"),
        egui::Key::F9 => Some("F9"),
        egui::Key::F10 => Some("F10"),
        egui::Key::F11 => Some("F11"),
        egui::Key::F12 => Some("F12"),
        _ => None,
    }
}

fn main() -> eframe::Result<()> {
    let cfg = config::load_config();
    let default_level = if cfg.log_level.trim().is_empty() {
        "info"
    } else {
        cfg.log_level.as_str()
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level)),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1200.0, 760.0])
            .with_title("Phantom — Fullscreen Mapper"),
        ..Default::default()
    };

    eframe::run_native(
        "phantom-gui",
        options,
        Box::new(|cc| Ok(Box::new(PhantomGui::new(cc)))),
    )
}

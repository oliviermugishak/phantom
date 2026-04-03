mod overlay;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};

use phantom::config;
use phantom::input::Key;
use phantom::ipc::{self, IpcRequest, IpcResponse};
use phantom::profile::{
    JoystickKeys, JoystickMode, LayerMode, MacroAction, MacroStep, MouseCameraActivationMode, Node,
    Profile, Region, RelPos, ScreenOverride,
};

const COLOR_TAP: Color32 = Color32::from_rgb(66, 133, 244);
const COLOR_HOLD: Color32 = Color32::from_rgb(234, 67, 53);
const COLOR_TOGGLE: Color32 = Color32::from_rgb(0, 172, 193);
const COLOR_JOYSTICK: Color32 = Color32::from_rgb(52, 168, 83);
const COLOR_DRAG: Color32 = Color32::from_rgb(0, 200, 140);
const COLOR_LOOK: Color32 = Color32::from_rgb(251, 188, 4);
const COLOR_REPEAT: Color32 = Color32::from_rgb(171, 71, 188);
const COLOR_MACRO: Color32 = Color32::from_rgb(255, 112, 67);
const COLOR_LAYER: Color32 = Color32::from_rgb(158, 158, 158);
const CANVAS_BG: Color32 = Color32::from_rgb(19, 21, 26);
const CONTENT_BG: Color32 = Color32::from_rgb(26, 29, 36);
const HANDLE_SIZE: f32 = 12.0;
const REGION_PICK_MARGIN: f32 = 6.0;
const REGION_BORDER_PICK_WIDTH: f32 = 14.0;
const DASH_LENGTH: f32 = 10.0;
const DASH_GAP: f32 = 6.0;
const LEFT_PANEL_WIDTH: f32 = 290.0;
const LEFT_PANEL_RUNTIME_RESERVE: f32 = 200.0;
const RIGHT_PANEL_WIDTH: f32 = 390.0;
const CONTROL_CARD_ACTION_BUTTON_WIDTH: f32 = 26.0;
const MAX_HISTORY: usize = 128;
const RUNTIME_POLL_INTERVAL_ACTIVE: Duration = Duration::from_millis(250);
const RUNTIME_POLL_INTERVAL_CONNECTED: Duration = Duration::from_secs(1);
const RUNTIME_POLL_INTERVAL_IDLE: Duration = Duration::from_secs(5);
const MIN_CANVAS_ZOOM: f32 = 0.75;
const MAX_CANVAS_ZOOM: f32 = 3.0;
const SNAP_GRID_STEP: f64 = 0.05;
const SNAP_THRESHOLD: f64 = 0.012;
const TOOLBAR_HEIGHT: f32 = 126.0;
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

const MAX_LOGICAL_TOUCH_SLOT: u8 = u8::MAX - 1;

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
    Drag,
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
    MouseLookActivation(usize),
}

#[derive(Clone, Copy)]
enum RegionHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    Left,
}

enum DragState {
    Point {
        idx: usize,
    },
    DragEnd {
        idx: usize,
    },
    Pan {
        origin_pan: Vec2,
        start_mouse: Pos2,
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

#[derive(Clone, Copy)]
enum ControlListAction {
    Select(usize),
    MoveUp(usize),
    MoveDown(usize),
    Duplicate(usize),
    Delete(usize),
}

#[derive(Default)]
struct RuntimeState {
    connected: bool,
    profile: Option<String>,
    paused: bool,
    capture_active: bool,
    mouse_grabbed: bool,
    mouse_touch_active: bool,
    keyboard_grabbed: bool,
    screen: Option<(u32, u32)>,
    active_layers: Vec<String>,
    last_error: Option<String>,
    last_checked: Option<Instant>,
}

struct Banner {
    text: String,
    is_error: bool,
}

struct OverlapPicker {
    anchor: Pos2,
    candidates: Vec<usize>,
}

#[derive(Clone, PartialEq)]
struct EditorSnapshot {
    profile: Option<Profile>,
    profile_path: Option<PathBuf>,
    selected: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
enum LayerFilter {
    #[default]
    All,
    Base,
    Named(String),
}

impl LayerFilter {
    fn label(&self) -> String {
        match self {
            LayerFilter::All => "All".into(),
            LayerFilter::Base => "Base".into(),
            LayerFilter::Named(name) => name.clone(),
        }
    }

    fn matches_node(&self, node: &Node) -> bool {
        match self {
            LayerFilter::All => true,
            LayerFilter::Base => node.layer().trim().is_empty(),
            LayerFilter::Named(name) => {
                if node.layer().trim() == name {
                    return true;
                }
                matches!(
                    node,
                    Node::LayerShift { layer_name, .. } if layer_name.trim() == name
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LayerSummary {
    filter: LayerFilter,
    node_count: usize,
    active: bool,
    has_switch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct StudioPrefs {
    show_labels: bool,
    auto_push_on_save: bool,
    snap_to_grid: bool,
    last_profile_path: Option<PathBuf>,
    layer_filter: LayerFilter,
    right_panel_tab: RightPanelTab,
}

impl Default for StudioPrefs {
    fn default() -> Self {
        Self {
            show_labels: true,
            auto_push_on_save: true,
            snap_to_grid: true,
            last_profile_path: None,
            layer_filter: LayerFilter::All,
            right_panel_tab: RightPanelTab::Overview,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum RightPanelTab {
    #[default]
    Overview,
    Inspect,
    Runtime,
    Settings,
}

impl RightPanelTab {
    fn label(self) -> &'static str {
        match self {
            RightPanelTab::Overview => "Overview",
            RightPanelTab::Inspect => "Inspect",
            RightPanelTab::Runtime => "Runtime",
            RightPanelTab::Settings => "Settings",
        }
    }
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
    snap_to_grid: bool,
    layer_filter: LayerFilter,
    right_panel_tab: RightPanelTab,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    clipboard: Option<Node>,
    overlap_picker: Option<OverlapPicker>,
    canvas_zoom: f32,
    canvas_pan: Vec2,
}

impl PhantomGui {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = config::load_config();
        let prefs = load_studio_prefs();
        let mut gui = Self {
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
            show_labels: prefs.show_labels,
            auto_push_on_save: prefs.auto_push_on_save,
            snap_to_grid: prefs.snap_to_grid,
            layer_filter: prefs.layer_filter,
            right_panel_tab: prefs.right_panel_tab,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clipboard: None,
            overlap_picker: None,
            canvas_zoom: 1.0,
            canvas_pan: Vec2::ZERO,
        };
        if let Some(path) = prefs.last_profile_path.filter(|path| path.exists()) {
            gui.load_profile(&path);
            gui.set_banner(format!("Restored {}", path.display()), false);
        } else if let Some(path) = config::default_profile_path().filter(|path| path.exists()) {
            gui.load_profile(&path);
            gui.set_banner(format!("Loaded default {}", path.display()), false);
        }
        gui
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

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            profile: self.profile.clone(),
            profile_path: self.profile_path.clone(),
            selected: self.selected,
        }
    }

    fn reset_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn begin_edit(&mut self) {
        let snapshot = self.snapshot();
        self.push_history_snapshot(snapshot);
    }

    fn push_history_snapshot(&mut self, snapshot: EditorSnapshot) {
        if self.undo_stack.last() != Some(&snapshot) {
            self.undo_stack.push(snapshot);
            if self.undo_stack.len() > MAX_HISTORY {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
    }

    fn restore_snapshot(&mut self, snapshot: EditorSnapshot) {
        self.profile = snapshot.profile;
        self.profile_path = snapshot.profile_path;
        self.selected = snapshot.selected;
        self.drag_state = None;
        self.pending_binding = None;
        self.pending_binding_started_at = None;
        self.overlap_picker = None;
        self.dirty = true;
    }

    fn undo(&mut self) {
        let Some(snapshot) = self.undo_stack.pop() else {
            return;
        };
        let current = self.snapshot();
        if self.redo_stack.last() != Some(&current) {
            self.redo_stack.push(current);
        }
        self.restore_snapshot(snapshot);
        self.set_banner("Undo", false);
    }

    fn redo(&mut self) {
        let Some(snapshot) = self.redo_stack.pop() else {
            return;
        };
        let current = self.snapshot();
        if self.undo_stack.last() != Some(&current) {
            self.undo_stack.push(current);
        }
        self.restore_snapshot(snapshot);
        self.set_banner("Redo", false);
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn studio_prefs(&self) -> StudioPrefs {
        StudioPrefs {
            show_labels: self.show_labels,
            auto_push_on_save: self.auto_push_on_save,
            snap_to_grid: self.snap_to_grid,
            last_profile_path: self.profile_path.clone(),
            layer_filter: self.layer_filter.clone(),
            right_panel_tab: self.right_panel_tab,
        }
    }

    fn persist_studio_prefs(&self) {
        let path = studio_prefs_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("cannot create studio prefs dir {}: {}", parent.display(), e);
                return;
            }
        }
        match toml::to_string_pretty(&self.studio_prefs()) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!("cannot write studio prefs {}: {}", path.display(), e);
                }
            }
            Err(e) => tracing::warn!("cannot serialize studio prefs: {}", e),
        }
    }

    fn focus_selection(&mut self, idx: usize) {
        self.selected = Some(idx);
        self.overlap_picker = None;
        self.right_panel_tab = RightPanelTab::Inspect;
    }

    fn layer_summaries(&self) -> Vec<LayerSummary> {
        let Some(profile) = &self.profile else {
            return Vec::new();
        };

        let mut layers: BTreeMap<String, (usize, bool)> = BTreeMap::new();
        let mut base_count = 0usize;

        for node in &profile.nodes {
            let layer = node.layer().trim();
            if layer.is_empty() {
                base_count += 1;
            } else {
                layers
                    .entry(layer.to_string())
                    .and_modify(|entry| entry.0 += 1)
                    .or_insert((1, false));
            }

            if let Node::LayerShift { layer_name, .. } = node {
                let layer_name = layer_name.trim();
                if !layer_name.is_empty() {
                    layers
                        .entry(layer_name.to_string())
                        .and_modify(|entry| entry.1 = true)
                        .or_insert((0, true));
                }
            }
        }

        let mut summaries = vec![LayerSummary {
            filter: LayerFilter::Base,
            node_count: base_count,
            active: false,
            has_switch: false,
        }];

        summaries.extend(layers.into_iter().map(|(name, (node_count, has_switch))| {
            LayerSummary {
                active: self
                    .runtime
                    .active_layers
                    .iter()
                    .any(|layer| layer == &name),
                filter: LayerFilter::Named(name),
                node_count,
                has_switch,
            }
        }));

        summaries
    }

    fn known_layer_names(&self) -> Vec<String> {
        let Some(profile) = &self.profile else {
            return Vec::new();
        };

        let mut names = Vec::new();
        for summary in self.layer_summaries() {
            if let LayerFilter::Named(name) = summary.filter {
                names.push(name);
            }
        }

        for node in &profile.nodes {
            if let Node::LayerShift { layer_name, .. } = node {
                let layer_name = layer_name.trim();
                if !layer_name.is_empty() && !names.iter().any(|name| name == layer_name) {
                    names.push(layer_name.to_string());
                }
            }
        }

        names.sort();
        names.dedup();
        names
    }

    fn filtered_controls_count(&self) -> usize {
        self.profile
            .as_ref()
            .map(|profile| {
                profile
                    .nodes
                    .iter()
                    .filter(|node| self.layer_filter.matches_node(node))
                    .count()
            })
            .unwrap_or(0)
    }

    fn draw_layer_manager(&mut self, ui: &mut egui::Ui) {
        let Some(profile) = &self.profile else {
            return;
        };

        ui.separator();
        ui.heading("Layers");
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Base is always active. Named layers turn on through Layer Switch nodes.",
            )
            .small()
            .color(Color32::from_gray(155)),
        );
        ui.add_space(4.0);

        let all_selected = matches!(self.layer_filter, LayerFilter::All);
        if ui
            .selectable_label(all_selected, format!("All ({})", profile.nodes.len()))
            .clicked()
        {
            self.layer_filter = LayerFilter::All;
        }

        for summary in self.layer_summaries() {
            let mut line = format!("{} ({})", summary.filter.label(), summary.node_count);
            if summary.has_switch {
                line.push_str(" • switch");
            }
            if summary.active {
                line.push_str(" • active");
            }
            let text = if summary.active {
                RichText::new(line).color(Color32::from_rgb(255, 218, 121))
            } else {
                RichText::new(line)
            };
            if ui
                .selectable_label(self.layer_filter == summary.filter, text)
                .clicked()
            {
                self.layer_filter = summary.filter.clone();
            }
        }
    }

    fn draw_runtime_hotkeys(&self, ui: &mut egui::Ui) {
        let hotkeys = config::resolved_runtime_hotkeys(&self.config);
        ui.label(RichText::new("Daemon Hotkeys").strong());
        ui.label(format!(
            "Mouse routing: {}",
            hotkey_label(hotkeys.mouse_toggle)
        ));
        ui.label(format!("Capture: {}", hotkey_label(hotkeys.capture_toggle)));
        ui.label(format!("Pause: {}", hotkey_label(hotkeys.pause_toggle)));
        ui.label(format!("Overlay: {}", hotkey_label(hotkeys.overlay_toggle)));
        ui.label(format!("Shutdown: {}", hotkey_label(hotkeys.shutdown)));
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Phantom keeps the keyboard grabbed while the daemon is running so these hotkeys stay reliable even when gameplay capture is off.",
            )
            .small()
            .color(Color32::from_gray(170)),
        );
        ui.label(
            RichText::new(
                "When capture is on, releasing the mouse leaves Phantom in menu-touch mode. Grab the mouse to switch back into gameplay aim.",
            )
            .small()
            .color(Color32::from_gray(170)),
        );
        ui.label(
            RichText::new(
                "Pause is mainly a debug shortcut. Set pause_toggle = \"none\" in config if it is not part of your workflow.",
            )
            .small()
            .color(Color32::from_gray(170)),
        );
    }

    fn draw_studio_overview(&self, ui: &mut egui::Ui) {
        let Some(profile) = &self.profile else {
            ui.label("No profile loaded");
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Open a profile or create a new one. The studio remembers the last saved profile you loaded.",
                )
                .small(),
            );
            ui.add_space(8.0);
            self.draw_runtime_hotkeys(ui);
            return;
        };

        ui.label(RichText::new("Studio Overview").strong());
        ui.add_space(6.0);

        let used_slots = profile.nodes.iter().filter_map(Node::slot).count();
        let mouse_look_nodes: Vec<_> = profile
            .nodes
            .iter()
            .filter_map(|node| match node {
                Node::MouseCamera {
                    id,
                    activation_mode,
                    activation_key,
                    ..
                } => Some((id, activation_mode, activation_key.as_deref())),
                _ => None,
            })
            .collect();

        ui.group(|ui| {
            ui.label(RichText::new("Profile").strong());
            ui.label(format!("Name: {}", profile.name));
            ui.label(format!("Controls: {}", profile.nodes.len()));
            ui.label(format!("Touch slots in use: {}", used_slots));
            let (source_text, source_color) = profile_source_badge(self.profile_path.as_deref());
            ui.label(RichText::new(format!("Source: {}", source_text)).color(source_color));
            ui.label(format!(
                "Current list filter: {}",
                self.layer_filter.label()
            ));
            if let Some(path) = &self.profile_path {
                ui.label(format!("Path: {}", path.display()));
            } else {
                ui.label("Path: unsaved profile");
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Aim").strong());
            if mouse_look_nodes.is_empty() {
                ui.label("No Aim node in this profile yet.");
                ui.label(
                    RichText::new(
                        "Add an Aim control, then select it to edit activation mode, anchor, and activation key.",
                    )
                    .small(),
                );
            } else {
                for (id, activation_mode, activation_key) in mouse_look_nodes {
                    let mode = match activation_mode {
                        MouseCameraActivationMode::AlwaysOn => "always_on",
                        MouseCameraActivationMode::WhileHeld => "while_held",
                        MouseCameraActivationMode::Toggle => "toggle",
                    };
                    let key_suffix = activation_key
                        .map(|key| format!(" · key {}", key))
                        .unwrap_or_default();
                    ui.label(format!("{id}: {mode}{key_suffix}"));
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Aim is engine-managed touch-drag camera emulation. It is not a desktop cursor or native raw mouse path.",
                    )
                    .small()
                    .color(Color32::from_gray(170)),
                );
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Layers").strong());
            for summary in self.layer_summaries() {
                let mut line = format!("{} ({})", summary.filter.label(), summary.node_count);
                if summary.has_switch {
                    line.push_str(" • switch");
                }
                if summary.active {
                    line.push_str(" • active");
                }
                ui.label(line);
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            self.draw_runtime_hotkeys(ui);
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Select a control from the list or canvas to edit bindings, position, aim behavior, and layer settings.",
            )
            .small()
            .color(Color32::from_gray(170)),
        );
    }

    fn draw_runtime_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Runtime").strong());
        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(RichText::new("Daemon State").strong());
            ui.label(format!(
                "Connection: {}",
                if self.runtime.connected {
                    "connected"
                } else {
                    "disconnected"
                }
            ));
            if let Some(profile) = &self.runtime.profile {
                ui.label(format!("Loaded profile: {}", profile));
            }
            if let Some((w, h)) = self.runtime.screen {
                ui.label(format!("Screen: {}x{}", w, h));
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
                "Mouse routing: {}",
                if self.runtime.mouse_grabbed {
                    "gameplay aim"
                } else {
                    "released"
                }
            ));
            ui.label(format!(
                "Menu touch: {}",
                if self.runtime.mouse_touch_active {
                    "active"
                } else {
                    "inactive"
                }
            ));
            ui.label(format!(
                "Keyboard ownership: {}",
                if self.runtime.keyboard_grabbed {
                    "daemon holds keyboard for hotkeys"
                } else {
                    "released"
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
            if self.runtime.active_layers.is_empty() {
                ui.label("Active layers: none");
            } else {
                ui.label(format!(
                    "Active layers: {}",
                    self.runtime.active_layers.join(", ")
                ));
            }
            if let Some(error) = &self.runtime.last_error {
                ui.label(RichText::new(error).small().color(Color32::LIGHT_RED));
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Daemon Control").strong());
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.runtime.connected, egui::Button::new("Start Daemon"))
                    .clicked()
                {
                    self.start_daemon();
                }
                if ui
                    .add_enabled(self.runtime.connected, egui::Button::new("Shutdown Daemon"))
                    .clicked()
                {
                    self.send_runtime_request(IpcRequest::Shutdown, "shutting down");
                }
                if ui.button("Refresh").clicked() {
                    self.refresh_status();
                }
            });
            ui.label(
                RichText::new(format!("Daemon log: {}", studio_daemon_log_path().display()))
                    .small()
                    .color(Color32::from_gray(170)),
            );
            let launch_note = if command_exists("pkexec") && !running_as_root() {
                "Studio will prefer pkexec for daemon launch on systems that still need elevated input access."
            } else {
                "If daemon launch fails here, start `phantom --daemon` manually in a terminal and return to the studio."
            };
            ui.label(
                RichText::new(launch_note)
                    .small()
                    .color(Color32::from_gray(170)),
            );
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Runtime Actions").strong());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Push Live").clicked() {
                    self.push_profile_live();
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
                let mouse_label = if self.runtime.mouse_grabbed {
                    "Release To Menu"
                } else {
                    "Grab For Aim"
                };
                if ui
                    .add_enabled(self.runtime.capture_active, egui::Button::new(mouse_label))
                    .clicked()
                {
                    self.send_runtime_request(IpcRequest::ToggleMouse, "mouse routing toggled");
                }
            });
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            self.draw_runtime_hotkeys(ui);
        });
    }

    fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Studio Settings").strong());
        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(RichText::new("Studio Preferences").strong());
            ui.checkbox(&mut self.show_labels, "Show canvas labels");
            ui.checkbox(&mut self.snap_to_grid, "Snap placement to grid");
            ui.checkbox(&mut self.auto_push_on_save, "Push live after save");
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Config Paths").strong());
            ui.monospace(config::config_path().display().to_string());
            ui.monospace(config::profiles_dir().display().to_string());
            ui.monospace(studio_prefs_path().display().to_string());
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Profiles Directory").strong());
            let profiles = available_profile_paths();
            ui.label(format!("Discovered profiles: {}", profiles.len()));
            if profiles.is_empty() {
                ui.label(
                    RichText::new(
                        "No profiles found in ~/.config/phantom/profiles yet. Run install.sh or save a profile into that directory.",
                    )
                    .small()
                    .color(Color32::from_gray(170)),
                );
            } else {
                for path in profiles.iter().take(8) {
                    ui.monospace(path.file_name().unwrap_or_default().to_string_lossy());
                }
                if profiles.len() > 8 {
                    ui.label(
                        RichText::new(format!("…and {} more", profiles.len() - 8))
                            .small()
                            .color(Color32::from_gray(170)),
                    );
                }
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Product Language").strong());
            ui.label("This UI is the Phantom GUI.");
            ui.label(
                RichText::new(
                    "The GUI edits profiles, pushes them live, and helps you inspect runtime state. It is not an in-game overlay system.",
                )
                .small()
                .color(Color32::from_gray(170)),
            );
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            self.draw_runtime_hotkeys(ui);
        });
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
        self.screenshot = None;
        self.selected = None;
        self.overlap_picker = None;
        self.dirty = true;
        self.tool = Tool::Select;
        self.right_panel_tab = RightPanelTab::Overview;
        self.reset_history();
        self.canvas_zoom = 1.0;
        self.canvas_pan = Vec2::ZERO;
        self.set_banner("New profile created", false);
    }

    fn load_profile(&mut self, path: &Path) {
        match Profile::load(path) {
            Ok(profile) => {
                self.profile = Some(profile);
                self.profile_path = Some(path.to_path_buf());
                self.screenshot = None;
                self.selected = None;
                self.overlap_picker = None;
                self.dirty = false;
                self.tool = Tool::Select;
                self.right_panel_tab = RightPanelTab::Overview;
                self.reset_history();
                self.canvas_zoom = 1.0;
                self.canvas_pan = Vec2::ZERO;
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
                let should_lock_screen = self
                    .profile
                    .as_ref()
                    .is_some_and(|profile| profile.screen.is_none());
                if should_lock_screen {
                    self.begin_edit();
                }
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
                            self.mark_dirty();
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

    fn start_daemon(&mut self) {
        if self.runtime.connected {
            self.set_banner("Daemon already running", false);
            return;
        }

        let Some(binary) = find_phantom_binary() else {
            self.set_banner(
                "Could not locate the phantom daemon binary. Install Phantom or build target/release/phantom first.",
                true,
            );
            return;
        };

        let log_path = studio_daemon_log_path();
        if let Some(parent) = log_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                self.set_banner(
                    format!(
                        "Cannot create daemon log directory {}: {}",
                        parent.display(),
                        e
                    ),
                    true,
                );
                return;
            }
        }

        let stdout_log = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(file) => file,
            Err(e) => {
                self.set_banner(
                    format!("Cannot open daemon log {}: {}", log_path.display(), e),
                    true,
                );
                return;
            }
        };
        let stderr_log = match stdout_log.try_clone() {
            Ok(file) => file,
            Err(e) => {
                self.set_banner(
                    format!(
                        "Cannot prepare daemon stderr log {}: {}",
                        log_path.display(),
                        e
                    ),
                    true,
                );
                return;
            }
        };

        let use_pkexec = !running_as_root() && command_exists("pkexec");
        let mut command = if use_pkexec {
            let mut command = Command::new("pkexec");
            command.arg(&binary);
            command
        } else {
            Command::new(&binary)
        };
        command
            .arg("--daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));

        match command.spawn() {
            Ok(_) => {
                self.runtime.connected = false;
                self.runtime.last_error = None;
                self.runtime.last_checked = None;
                if use_pkexec {
                    self.set_banner(
                        format!(
                            "Daemon launch requested via pkexec. Authenticate if prompted. Log: {}",
                            log_path.display()
                        ),
                        false,
                    );
                } else {
                    self.set_banner(
                        format!(
                            "Daemon launch requested. Studio will refresh automatically. Log: {}",
                            log_path.display()
                        ),
                        false,
                    );
                }
            }
            Err(e) => {
                self.runtime.connected = false;
                self.runtime.last_error = Some(e.to_string());
                self.set_banner(format!("Daemon launch failed: {}", e), true);
            }
        }
    }

    fn apply_status(&mut self, response: &IpcResponse) {
        self.runtime.profile = response.profile.clone();
        self.runtime.paused = response.paused.unwrap_or(false);
        self.runtime.capture_active = response.capture_active.unwrap_or(false);
        self.runtime.mouse_grabbed = response.mouse_grabbed.unwrap_or(false);
        self.runtime.mouse_touch_active = response
            .mouse_touch_active
            .unwrap_or(self.runtime.capture_active && !self.runtime.mouse_grabbed);
        self.runtime.keyboard_grabbed = response.keyboard_grabbed.unwrap_or(false);
        self.runtime.active_layers = response.active_layers.clone().unwrap_or_default();
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
        let interval = self.runtime_poll_interval();
        let should_poll = self
            .runtime
            .last_checked
            .map(|t| t.elapsed() >= interval)
            .unwrap_or(true);
        if should_poll {
            self.refresh_status();
        }
    }

    fn runtime_poll_interval(&self) -> Duration {
        if self.runtime.last_checked.is_none()
            || matches!(self.right_panel_tab, RightPanelTab::Runtime)
        {
            RUNTIME_POLL_INTERVAL_ACTIVE
        } else if self.runtime.connected {
            RUNTIME_POLL_INTERVAL_CONNECTED
        } else {
            RUNTIME_POLL_INTERVAL_IDLE
        }
    }

    fn background_tick_interval(&self) -> Duration {
        self.runtime_poll_interval()
    }

    fn next_slot(&self) -> Option<u8> {
        let profile = self.profile.as_ref()?;
        let used_slots: std::collections::HashSet<u8> =
            profile.nodes.iter().filter_map(Node::slot).collect();
        (0..=MAX_LOGICAL_TOUCH_SLOT).find(|slot| !used_slots.contains(slot))
    }

    fn unique_node_id(&self, prefix: &str) -> String {
        let Some(profile) = &self.profile else {
            return format!("{prefix}_1");
        };
        let mut next = 1;
        loop {
            let candidate = format!("{prefix}_{next}");
            if !profile.nodes.iter().any(|node| node.id() == candidate) {
                return candidate;
            }
            next += 1;
        }
    }

    fn duplicate_node_for_insert(&self, node: &Node) -> Option<Node> {
        let mut node = node.clone();
        let prefix = node_id_prefix(&node);
        set_node_id(&mut node, self.unique_node_id(prefix));
        match &mut node {
            Node::Tap { slot, pos, .. }
            | Node::HoldTap { slot, pos, .. }
            | Node::ToggleTap { slot, pos, .. }
            | Node::RepeatTap { slot, pos, .. } => {
                *slot = self.next_slot()?;
                *pos = offset_rel_pos(*pos);
            }
            Node::Drag {
                slot, start, end, ..
            } => {
                *slot = self.next_slot()?;
                *start = offset_rel_pos(*start);
                *end = offset_rel_pos(*end);
            }
            Node::Joystick {
                slot,
                pos,
                mode,
                region,
                ..
            } => {
                *slot = self.next_slot()?;
                *pos = offset_rel_pos(*pos);
                if matches!(mode, JoystickMode::Floating) {
                    if let Some(zone) = region.as_mut() {
                        *zone = offset_region(*zone);
                        *pos = region_center(zone);
                    }
                }
            }
            Node::MouseCamera { slot, anchor, .. } => {
                *slot = self.next_slot()?;
                *anchor = offset_rel_pos(*anchor);
            }
            Node::Macro { .. } | Node::LayerShift { .. } => {}
        }
        Some(node)
    }

    fn copy_selected(&mut self) {
        let Some(profile) = &self.profile else {
            return;
        };
        let Some(idx) = self.selected else {
            return;
        };
        if let Some(node) = profile.nodes.get(idx) {
            self.clipboard = Some(node.clone());
            self.set_banner("Copied control", false);
        }
    }

    fn paste_clipboard(&mut self) {
        let Some(node) = self.clipboard.clone() else {
            return;
        };
        let Some(node) = self.duplicate_node_for_insert(&node) else {
            self.set_banner("No free logical touch slot for paste", true);
            return;
        };
        self.begin_edit();
        let Some(profile) = &mut self.profile else {
            return;
        };
        profile.nodes.push(node);
        let idx = profile.nodes.len() - 1;
        self.selected = Some(idx);
        self.right_panel_tab = RightPanelTab::Inspect;
        self.mark_dirty();
        self.set_banner("Pasted control", false);
    }

    fn duplicate_selected(&mut self) {
        let Some(idx) = self.selected else {
            return;
        };
        let Some(source) = self
            .profile
            .as_ref()
            .and_then(|profile| profile.nodes.get(idx))
            .cloned()
        else {
            return;
        };
        let Some(node) = self.duplicate_node_for_insert(&source) else {
            self.set_banner("No free logical touch slot for duplicate", true);
            return;
        };
        self.begin_edit();
        let Some(profile) = &mut self.profile else {
            return;
        };
        profile.nodes.insert(idx + 1, node);
        self.selected = Some(idx + 1);
        self.right_panel_tab = RightPanelTab::Inspect;
        self.mark_dirty();
        self.set_banner("Duplicated control", false);
    }

    fn duplicate_selected_into_layer(&mut self, target_layer: String) {
        let Some(idx) = self.selected else {
            return;
        };
        let Some(source) = self
            .profile
            .as_ref()
            .and_then(|profile| profile.nodes.get(idx))
            .cloned()
        else {
            return;
        };
        let Some(mut node) = self.duplicate_node_for_insert(&source) else {
            self.set_banner("No free logical touch slot for duplicate", true);
            return;
        };

        match &mut node {
            Node::Tap { layer, .. }
            | Node::HoldTap { layer, .. }
            | Node::ToggleTap { layer, .. }
            | Node::Joystick { layer, .. }
            | Node::Drag { layer, .. }
            | Node::MouseCamera { layer, .. }
            | Node::RepeatTap { layer, .. }
            | Node::Macro { layer, .. } => *layer = target_layer.clone(),
            Node::LayerShift { .. } => {
                self.set_banner("Layer switch nodes are not duplicated into layers", true);
                return;
            }
        }

        self.begin_edit();
        let Some(profile) = &mut self.profile else {
            return;
        };
        profile.nodes.insert(idx + 1, node);
        self.selected = Some(idx + 1);
        self.right_panel_tab = RightPanelTab::Inspect;
        self.mark_dirty();

        let banner = if target_layer.trim().is_empty() {
            "Duplicated control into base layer".to_string()
        } else {
            format!("Duplicated control into layer {}", target_layer)
        };
        self.set_banner(banner, false);
    }

    fn move_selected(&mut self, delta: isize) {
        let Some(len) = self.profile.as_ref().map(|profile| profile.nodes.len()) else {
            return;
        };
        let Some(idx) = self.selected else {
            return;
        };
        let target = idx as isize + delta;
        if !(0..len as isize).contains(&target) {
            return;
        }
        self.begin_edit();
        let Some(profile) = &mut self.profile else {
            return;
        };
        profile.nodes.swap(idx, target as usize);
        self.selected = Some(target as usize);
        self.right_panel_tab = RightPanelTab::Inspect;
        self.mark_dirty();
    }

    fn add_non_canvas_node(&mut self, template: NodeTemplate) {
        if self.profile.is_none() {
            self.set_banner("Open or create a profile first", true);
            return;
        }
        self.begin_edit();
        let Some(profile) = &mut self.profile else {
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
        self.right_panel_tab = RightPanelTab::Inspect;
        self.mark_dirty();
    }

    fn place_node(&mut self, template: NodeTemplate, rel: RelPos) {
        let Some(slot) = self.next_slot() else {
            self.set_banner("All logical touch slot ids are already assigned", true);
            return;
        };
        if self.profile.is_none() {
            return;
        }
        self.begin_edit();
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
                mode: JoystickMode::Fixed,
                region: None,
                keys: JoystickKeys {
                    up: "W".into(),
                    down: "S".into(),
                    left: "A".into(),
                    right: "D".into(),
                },
            },
            NodeTemplate::Drag => Node::Drag {
                id: format!("drag_{}", slot),
                layer: String::new(),
                slot,
                start: rel,
                end: RelPos {
                    x: rel.x,
                    y: (rel.y - 0.18).clamp(0.0, 1.0),
                },
                key: "Up".into(),
                duration_ms: 90,
            },
            NodeTemplate::MouseLook => Node::MouseCamera {
                id: format!("aim_{}", slot),
                layer: String::new(),
                slot,
                anchor: rel,
                reach: 0.18,
                sensitivity: 1.0,
                activation_mode: MouseCameraActivationMode::AlwaysOn,
                activation_key: None,
                invert_y: false,
                legacy_region: None,
            },
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
        self.right_panel_tab = RightPanelTab::Inspect;
        self.mark_dirty();
        self.tool = Tool::Select;
    }

    fn delete_selected(&mut self) {
        let Some(idx) = self.selected else {
            return;
        };
        let Some(len) = self.profile.as_ref().map(|profile| profile.nodes.len()) else {
            return;
        };
        if idx < len {
            self.begin_edit();
            let Some(profile) = &mut self.profile else {
                return;
            };
            profile.nodes.remove(idx);
            self.selected = None;
            self.overlap_picker = None;
            self.drag_state = None;
            self.pending_binding = None;
            self.mark_dirty();
        }
    }

    fn begin_binding_capture(&mut self, target: BindingTarget) {
        self.pending_binding = Some(target);
        self.pending_binding_started_at = Some(Instant::now());
        self.right_panel_tab = RightPanelTab::Inspect;
        self.set_banner("Binding mode active. Press Esc to cancel.", false);
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
                self.mark_dirty();
                self.set_banner(format!("Bound {}", binding), false);
            }
        }
    }

    fn apply_binding(&mut self, target: &BindingTarget, binding: String) -> bool {
        if self.profile.is_none() {
            return false;
        }
        self.begin_edit();
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
                    | Node::Drag { key, .. }
                    | Node::RepeatTap { key, .. }
                    | Node::Macro { key, .. }
                    | Node::LayerShift { key, .. } => {
                        *key = binding;
                        true
                    }
                    Node::Joystick { .. } => false,
                    Node::MouseCamera { activation_key, .. } => {
                        *activation_key = Some(binding);
                        true
                    }
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
            BindingTarget::MouseLookActivation(idx) => {
                let Some(node) = profile.nodes.get_mut(*idx) else {
                    return false;
                };
                let Node::MouseCamera { activation_key, .. } = node else {
                    return false;
                };
                *activation_key = Some(binding);
                true
            }
        }
    }

    fn base_content_rect(&self, canvas: Rect) -> Rect {
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

    fn content_rect(&self, canvas: Rect) -> Rect {
        zoom_rect(
            self.base_content_rect(canvas),
            self.canvas_zoom,
            self.canvas_pan,
        )
    }

    fn zoom_canvas(&mut self, canvas: Rect, pointer: Option<Pos2>, zoom_delta: f32) {
        let old_zoom = self.canvas_zoom;
        let next_zoom = (self.canvas_zoom * zoom_delta).clamp(MIN_CANVAS_ZOOM, MAX_CANVAS_ZOOM);
        if (next_zoom - old_zoom).abs() < f32::EPSILON {
            return;
        }

        let base = self.base_content_rect(canvas);
        if let Some(pointer) = pointer {
            let old_rect = zoom_rect(base, old_zoom, self.canvas_pan);
            let next_rect = zoom_rect(base, next_zoom, self.canvas_pan);
            let old_rel = from_canvas_pos(old_rect, pointer);
            let new_pointer = to_canvas_pos(next_rect, &old_rel);
            self.canvas_pan += pointer - new_pointer;
        }

        self.canvas_zoom = next_zoom;
        self.clamp_canvas_pan(canvas);
    }

    fn clamp_canvas_pan(&mut self, canvas: Rect) {
        let base = self.base_content_rect(canvas);
        let scaled = zoom_rect(base, self.canvas_zoom, Vec2::ZERO);
        let max_x = ((scaled.width() - canvas.width()).max(0.0) / 2.0).max(0.0);
        let max_y = ((scaled.height() - canvas.height()).max(0.0) / 2.0).max(0.0);
        if max_x == 0.0 {
            self.canvas_pan.x = 0.0;
        }
        if max_y == 0.0 {
            self.canvas_pan.y = 0.0;
        }
        self.canvas_pan.x = self.canvas_pan.x.clamp(-max_x, max_x);
        self.canvas_pan.y = self.canvas_pan.y.clamp(-max_y, max_y);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.pending_binding.is_some() {
            return;
        }
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
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift) {
            self.undo();
        }
        if ctx.input(|i| {
            (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
                || (i.modifiers.command && i.key_pressed(egui::Key::Y))
        }) {
            self.redo();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
            self.duplicate_selected();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) {
            self.copy_selected();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::V)) {
            self.paste_clipboard();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.delete_selected();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
            self.tool = Tool::Select;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
            self.tool = Tool::Place(NodeTemplate::Tap);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
            self.tool = Tool::Place(NodeTemplate::HoldTap);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num4)) {
            self.tool = Tool::Place(NodeTemplate::ToggleTap);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num5)) {
            self.tool = Tool::Place(NodeTemplate::Joystick);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num6)) {
            self.tool = Tool::Place(NodeTemplate::Drag);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num7)) {
            self.tool = Tool::Place(NodeTemplate::MouseLook);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num8)) {
            self.tool = Tool::Place(NodeTemplate::RepeatTap);
        }
    }

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar")
            .exact_height(TOOLBAR_HEIGHT)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
                    ui.label(RichText::new("Project").strong().small());
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
                    ui.menu_button("Profiles", |ui| {
                        let profiles = available_profile_paths();
                        if profiles.is_empty() {
                            ui.label(
                                RichText::new("No profiles found in ~/.config/phantom/profiles")
                                    .small()
                                    .color(Color32::from_gray(170)),
                            );
                        } else {
                            for path in profiles {
                                let label = profile_menu_label(&path);
                                if ui.button(label).clicked() {
                                    self.load_profile(&path);
                                    ui.close_menu();
                                }
                            }
                        }
                    });
                    if ui.button("Screenshot").clicked() {
                        self.load_screenshot(ctx);
                    }
                    if ui.button("Undo").clicked() {
                        self.undo();
                    }
                    if ui.button("Redo").clicked() {
                        self.redo();
                    }
                    ui.separator();
                    ui.label(RichText::new("Edit").strong().small());
                    if ui.button("Copy").clicked() {
                        self.copy_selected();
                    }
                    if ui.button("Paste").clicked() {
                        self.paste_clipboard();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_selected();
                    }
                    if ui.button("Delete").clicked() {
                        self.delete_selected();
                    }
                });

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
                    ui.label(RichText::new("Place").strong().small());
                    tool_button(ui, &mut self.tool, Tool::Select, "Select (1)");
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::Tap),
                        "Tap (2)",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::HoldTap),
                        "Hold (3)",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::ToggleTap),
                        "Toggle (4)",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::Joystick),
                        "Left Stick (5)",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::Drag),
                        "Drag (6)",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::MouseLook),
                        "Aim (7)",
                    );
                    tool_button(
                        ui,
                        &mut self.tool,
                        Tool::Place(NodeTemplate::RepeatTap),
                        "Rapid Tap (8)",
                    );
                    if ui.button("Add Macro").clicked() {
                        self.add_non_canvas_node(NodeTemplate::Macro);
                    }
                    if ui.button("Add Layer").clicked() {
                        self.add_non_canvas_node(NodeTemplate::LayerShift);
                    }
                    ui.separator();
                    ui.label(RichText::new("View").strong().small());
                    if ui.button("Zoom -").clicked() {
                        self.canvas_zoom =
                            (self.canvas_zoom - 0.1).clamp(MIN_CANVAS_ZOOM, MAX_CANVAS_ZOOM);
                    }
                    ui.label(format!("{:.0}%", self.canvas_zoom * 100.0));
                    if ui.button("Zoom +").clicked() {
                        self.canvas_zoom =
                            (self.canvas_zoom + 0.1).clamp(MIN_CANVAS_ZOOM, MAX_CANVAS_ZOOM);
                    }
                    if ui.button("Reset View").clicked() {
                        self.canvas_zoom = 1.0;
                        self.canvas_pan = Vec2::ZERO;
                    }
                    ui.checkbox(&mut self.snap_to_grid, "Snap");
                    ui.checkbox(&mut self.show_labels, "Labels");
                });

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
                    ui.label(RichText::new("Runtime").strong().small());
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
                    let mouse_label = if self.runtime.mouse_grabbed {
                        "Release To Menu"
                    } else {
                        "Grab For Aim"
                    };
                    if ui
                        .add_enabled(self.runtime.capture_active, egui::Button::new(mouse_label))
                        .clicked()
                    {
                        self.send_runtime_request(IpcRequest::ToggleMouse, "mouse routing toggled");
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
                        if !self.runtime.active_layers.is_empty() {
                            ui.label(
                                RichText::new(format!(
                                    "Layers: {}",
                                    self.runtime.active_layers.join(", ")
                                ))
                                .color(Color32::from_rgb(255, 218, 121)),
                            );
                        }
                        runtime_chip(ui, self.runtime.mouse_grabbed, "Mouse");
                        runtime_chip(ui, self.runtime.mouse_touch_active, "MenuTouch");
                        runtime_chip(ui, self.runtime.keyboard_grabbed, "Keyboard");
                        runtime_chip(ui, self.runtime.capture_active, "Capture");
                        runtime_chip(ui, !self.runtime.paused, "Active");
                        runtime_chip(ui, self.runtime.connected, "Daemon");
                    });
                });
            });
    }

    fn draw_left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .resizable(false)
            .exact_width(LEFT_PANEL_WIDTH)
            .show(ctx, |ui| {
                let snapshot_before = self.snapshot();
                let mut profile_changed = false;
                ui.heading("Studio");
                ui.label(RichText::new("Profile").strong());
                ui.add_space(6.0);

                let default_screen = self.default_screen();
                let has_profile = self.profile.is_some();
                if let Some(profile) = &mut self.profile {
                    if ui.text_edit_singleline(&mut profile.name).changed() {
                        profile_changed = true;
                    }
                    let (source_text, source_color) =
                        profile_source_badge(self.profile_path.as_deref());
                    ui.label(RichText::new(source_text).small().color(source_color));

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
                            profile_changed = true;
                        }
                        ui.label("x");
                        if ui
                            .add(egui::DragValue::new(&mut height).range(320.0..=8000.0))
                            .changed()
                        {
                            screen.height = height as u32;
                            profile_changed = true;
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
                            profile_changed = true;
                        }
                    });
                    ui.checkbox(&mut self.auto_push_on_save, "Push live after save");
                } else {
                    ui.label("Create or open a profile to start mapping.");
                }

                if has_profile {
                    self.draw_layer_manager(ui);
                    ui.separator();
                    let total_controls = self
                        .profile
                        .as_ref()
                        .map(|profile| profile.nodes.len())
                        .unwrap_or(0);
                    ui.heading(format!(
                        "Controls ({}/{})",
                        self.filtered_controls_count(),
                        total_controls
                    ));
                    ui.add_space(6.0);

                    ui.label(
                        RichText::new(
                            "Select a card to inspect it. Reorder, duplicate, and delete actions appear on the selected control only.",
                        )
                        .small()
                        .color(Color32::from_gray(155)),
                    );
                    ui.add_space(4.0);

                    let controls_height =
                        (ui.available_height() - LEFT_PANEL_RUNTIME_RESERVE).max(180.0);
                    let mut action = None;

                    egui::ScrollArea::vertical()
                        .id_salt("controls_list")
                        .max_height(controls_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(profile) = &self.profile {
                                let mut visible_count = 0usize;
                                for (idx, node) in profile.nodes.iter().enumerate() {
                                    if !self.layer_filter.matches_node(node) {
                                        continue;
                                    }
                                    visible_count += 1;
                                    let selected = self.selected == Some(idx);
                                    let is_layer_active = !node.layer().trim().is_empty()
                                        && self
                                            .runtime
                                            .active_layers
                                            .iter()
                                            .any(|layer| layer == node.layer());
                                    if action.is_none() {
                                        action = draw_control_card(
                                            ui,
                                            idx,
                                            node,
                                            selected,
                                            is_layer_active,
                                        );
                                    } else {
                                        draw_control_card(ui, idx, node, selected, is_layer_active);
                                    }
                                    ui.add_space(6.0);
                                }

                                if visible_count == 0 {
                                    ui.group(|ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(
                                            RichText::new("No controls in the current layer filter.")
                                                .strong(),
                                        );
                                        ui.label(
                                            RichText::new(
                                                "Switch the layer filter back to All or Base, or add controls into this layer.",
                                            )
                                            .small()
                                            .color(Color32::from_gray(160)),
                                        );
                                    });
                                }
                            }
                        });

                    if let Some(action) = action {
                        match action {
                            ControlListAction::Select(idx) => self.focus_selection(idx),
                            ControlListAction::MoveUp(idx) => {
                                self.focus_selection(idx);
                                self.move_selected(-1);
                            }
                            ControlListAction::MoveDown(idx) => {
                                self.focus_selection(idx);
                                self.move_selected(1);
                            }
                            ControlListAction::Duplicate(idx) => {
                                self.focus_selection(idx);
                                self.duplicate_selected();
                            }
                            ControlListAction::Delete(idx) => {
                                self.focus_selection(idx);
                                self.delete_selected();
                            }
                        }
                    }
                }

                ui.separator();
                ui.heading("Runtime Snapshot");
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
                    "Menu touch: {}",
                    if self.runtime.mouse_touch_active {
                        "active"
                    } else {
                        "inactive"
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
                ui.label(
                    RichText::new("Open the Runtime tab on the right for daemon control and full runtime actions.")
                        .small()
                        .color(Color32::from_gray(150)),
                );
                ui.horizontal_wrapped(|ui| {
                    if !self.runtime.connected {
                        if ui.button("Start Daemon").clicked() {
                            self.start_daemon();
                        }
                    } else if ui.button("Shutdown Daemon").clicked() {
                        self.send_runtime_request(IpcRequest::Shutdown, "shutting down");
                    }
                    if ui.button("Open Runtime").clicked() {
                        self.right_panel_tab = RightPanelTab::Runtime;
                    }
                });
                if profile_changed {
                    self.push_history_snapshot(snapshot_before);
                    self.mark_dirty();
                }
            });
    }

    fn draw_properties_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("properties_panel")
            .resizable(false)
            .exact_width(RIGHT_PANEL_WIDTH)
            .show(ctx, |ui| {
                let snapshot_before = self.snapshot();
                ui.heading("Phantom GUI");
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    for tab in [
                        RightPanelTab::Overview,
                        RightPanelTab::Inspect,
                        RightPanelTab::Runtime,
                        RightPanelTab::Settings,
                    ] {
                        if ui
                            .selectable_label(self.right_panel_tab == tab, tab.label())
                            .clicked()
                        {
                            self.right_panel_tab = tab;
                        }
                    }
                });
                ui.separator();

                match self.right_panel_tab {
                    RightPanelTab::Overview => {
                        self.draw_studio_overview(ui);
                        return;
                    }
                    RightPanelTab::Runtime => {
                        self.draw_runtime_panel(ui);
                        return;
                    }
                    RightPanelTab::Settings => {
                        self.draw_settings_panel(ui);
                        return;
                    }
                    RightPanelTab::Inspect => {}
                }

                let layer_choices = self.known_layer_names();
                let suggested_layer = match &self.layer_filter {
                    LayerFilter::Named(name) => Some(name.as_str()),
                    _ => None,
                };

                let Some(profile) = &mut self.profile else {
                    self.draw_studio_overview(ui);
                    return;
                };
                let Some(idx) = self.selected else {
                    ui.label("No control selected");
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "Select a control from the list or canvas, or switch to Overview, Runtime, or Settings.",
                        )
                        .small()
                        .color(Color32::from_gray(170)),
                    );
                    ui.add_space(10.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("Quick Starts").strong());
                        ui.label(
                            RichText::new(
                                "Use Aim for camera drag, Drag for one-shot swipes, and Layer Switch for context-specific remaps.",
                            )
                            .small()
                            .color(Color32::from_gray(165)),
                        );
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Add Aim").clicked() {
                                self.tool = Tool::Place(NodeTemplate::MouseLook);
                            }
                            if ui.button("Add Drag").clicked() {
                                self.tool = Tool::Place(NodeTemplate::Drag);
                            }
                            if ui.button("Add Layer Switch").clicked() {
                                self.add_non_canvas_node(NodeTemplate::LayerShift);
                            }
                        });
                    });
                    return;
                };
                if idx >= profile.nodes.len() {
                    ui.label("Selection is out of date");
                    return;
                }

                let mut start_binding = None;
                let mut delete_current = false;
                let mut dirty = false;
                let screen = profile.screen.clone();
                let mut duplicate_into_layer = None;

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
                        layer_row(
                            ui,
                            layer,
                            ("layer_row", idx),
                            &layer_choices,
                            suggested_layer,
                            &mut dirty,
                        );
                        position_editor(ui, pos, screen.as_ref(), &mut dirty);
                    }
                    Node::Joystick {
                        layer,
                        pos,
                        radius,
                        mode,
                        region,
                        ..
                    } => {
                        layer_row(
                            ui,
                            layer,
                            ("layer_row", idx),
                            &layer_choices,
                            suggested_layer,
                            &mut dirty,
                        );
                        ui.horizontal(|ui| {
                            ui.label("Mode");
                            egui::ComboBox::from_id_salt(("joystick_mode", idx))
                                .selected_text(match mode {
                                    JoystickMode::Fixed => "Fixed Center",
                                    JoystickMode::Floating => "Floating Zone",
                                })
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            matches!(mode, JoystickMode::Fixed),
                                            "Fixed Center",
                                        )
                                        .clicked()
                                    {
                                        if let Some(zone) = region.as_ref() {
                                            *pos = region_center(zone);
                                        }
                                        *mode = JoystickMode::Fixed;
                                        *region = None;
                                        dirty = true;
                                    }
                                    if ui
                                        .selectable_label(
                                            matches!(mode, JoystickMode::Floating),
                                            "Floating Zone",
                                        )
                                        .clicked()
                                    {
                                        *mode = JoystickMode::Floating;
                                        if region.is_none() {
                                            *region = Some(default_joystick_region(*pos));
                                        }
                                        dirty = true;
                                    }
                                });
                        });
                        match mode {
                            JoystickMode::Fixed => {
                                position_editor(ui, pos, screen.as_ref(), &mut dirty);
                            }
                            JoystickMode::Floating => {
                                let zone =
                                    region.get_or_insert_with(|| default_joystick_region(*pos));
                                region_editor(ui, zone, screen.as_ref(), &mut dirty);
                                *pos = region_center(zone);
                            }
                        }
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
                        let mode_hint = match mode {
                            JoystickMode::Fixed => {
                                "Fixed Center: always drag from one exact center point. Best for games with a visible static stick."
                            }
                            JoystickMode::Floating => {
                                "Floating Zone: touch starts inside the zone when movement begins, then drags from that runtime origin. Best for floating sticks and football-style movement zones."
                            }
                        };
                        ui.label(
                            RichText::new(mode_hint)
                                .small()
                                .color(Color32::from_gray(170)),
                        );
                    }
                    Node::Drag {
                        layer,
                        start,
                        end,
                        duration_ms,
                        ..
                    } => {
                        layer_row(
                            ui,
                            layer,
                            ("layer_row", idx),
                            &layer_choices,
                            suggested_layer,
                            &mut dirty,
                        );
                        ui.label(RichText::new("Start").small().strong());
                        position_editor(ui, start, screen.as_ref(), &mut dirty);
                        ui.label(RichText::new("End").small().strong());
                        position_editor(ui, end, screen.as_ref(), &mut dirty);
                        ui.horizontal(|ui| {
                            ui.label("Duration ms");
                            let mut value = *duration_ms as f64;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .speed(1.0)
                                        .range(16.0..=1500.0),
                                )
                                .changed()
                            {
                                *duration_ms = value as u64;
                                dirty = true;
                            }
                        });
                        ui.label(
                            RichText::new(
                                "Drag runs once from Start to End when the bound key is pressed. Use it for swipe games and sprint-latch style drags.",
                            )
                            .small()
                            .color(Color32::from_gray(170)),
                        );
                    }
                    Node::MouseCamera {
                        layer,
                        anchor,
                        reach,
                        ..
                    } => {
                        layer_row(
                            ui,
                            layer,
                            ("layer_row", idx),
                            &layer_choices,
                            suggested_layer,
                            &mut dirty,
                        );
                        position_editor(ui, anchor, screen.as_ref(), &mut dirty);
                        ui.horizontal(|ui| {
                            ui.label("Reach");
                            let mut value = *reach;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .speed(0.005)
                                        .range(0.05..=0.45),
                                )
                                .changed()
                            {
                                *reach = round3(value);
                                dirty = true;
                            }
                        });
                        ui.label(
                            RichText::new(
                                "Aim is engine-managed camera drag. The anchor is where Phantom re-centers the hidden look touch.",
                            )
                            .small()
                            .color(Color32::from_gray(170)),
                        );
                    }
                    Node::Macro { layer, .. } => {
                        layer_row(
                            ui,
                            layer,
                            ("layer_row", idx),
                            &layer_choices,
                            suggested_layer,
                            &mut dirty,
                        );
                    }
                    Node::LayerShift { .. } => {}
                }

                if !matches!(node, Node::LayerShift { .. }) {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Duplicate Into Base").clicked() {
                            duplicate_into_layer = Some(String::new());
                        }
                        if let Some(layer_name) = suggested_layer {
                            let button = format!("Duplicate Into {}", layer_name);
                            if ui.button(button).clicked() {
                                duplicate_into_layer = Some(layer_name.to_string());
                            }
                        }
                    });
                    ui.label(
                        RichText::new(
                            "Use layer duplicates to reuse a control in a new context without rebuilding it from scratch.",
                        )
                        .small()
                        .color(Color32::from_gray(160)),
                    );
                }

                ui.separator();
                match node {
                    Node::Tap { key, .. }
                    | Node::HoldTap { key, .. }
                    | Node::ToggleTap { key, .. }
                    | Node::Drag { key, .. }
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
                        activation_mode,
                        activation_key,
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
                        ui.horizontal(|ui| {
                            ui.label("Activation");
                            let selected = match activation_mode {
                                MouseCameraActivationMode::AlwaysOn => "Always On",
                                MouseCameraActivationMode::WhileHeld => "While Held",
                                MouseCameraActivationMode::Toggle => "Toggle",
                            };
                            egui::ComboBox::from_id_salt(("mouse_activation_mode", idx))
                                .selected_text(selected)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            matches!(
                                                activation_mode,
                                                MouseCameraActivationMode::AlwaysOn
                                            ),
                                            "Always On",
                                        )
                                        .clicked()
                                    {
                                        *activation_mode = MouseCameraActivationMode::AlwaysOn;
                                        *activation_key = None;
                                        dirty = true;
                                    }
                                    if ui
                                        .selectable_label(
                                            matches!(
                                                activation_mode,
                                                MouseCameraActivationMode::WhileHeld
                                            ),
                                            "While Held",
                                        )
                                        .clicked()
                                    {
                                        *activation_mode = MouseCameraActivationMode::WhileHeld;
                                        if activation_key.is_none() {
                                            *activation_key = Some("MouseRight".into());
                                        }
                                        dirty = true;
                                    }
                                    if ui
                                        .selectable_label(
                                            matches!(
                                                activation_mode,
                                                MouseCameraActivationMode::Toggle
                                            ),
                                            "Toggle",
                                        )
                                        .clicked()
                                    {
                                        *activation_mode = MouseCameraActivationMode::Toggle;
                                        if activation_key.is_none() {
                                            *activation_key = Some("MouseRight".into());
                                        }
                                        dirty = true;
                                    }
                                });
                        });
                        let mode_hint = match activation_mode {
                            MouseCameraActivationMode::AlwaysOn => {
                                "Always On: mouse movement drives camera drag whenever capture and mouse routing are active."
                            }
                            MouseCameraActivationMode::WhileHeld => {
                                "While Held: hold the activation key to enable camera drag. Good for ADS-style workflows."
                            }
                            MouseCameraActivationMode::Toggle => {
                                "Toggle: press the activation key once to enable aim, then again to disable it."
                            }
                        };
                        ui.label(
                            RichText::new(mode_hint)
                                .small()
                                .color(Color32::from_gray(170)),
                        );
                        if !matches!(activation_mode, MouseCameraActivationMode::AlwaysOn) {
                            let key = activation_key.get_or_insert_with(|| "MouseRight".into());
                            binding_picker(
                                ui,
                                "Activation Key",
                                key,
                                &BindingTarget::MouseLookActivation(idx),
                                self.pending_binding.as_ref(),
                                &mut start_binding,
                                &mut dirty,
                            );
                        }
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
                                    .add(
                                        egui::DragValue::new(&mut slot)
                                            .range(0.0..=MAX_LOGICAL_TOUCH_SLOT as f64),
                                    )
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
                                position_editor(ui, pos, screen.as_ref(), &mut dirty);
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
                if let Some(target_layer) = duplicate_into_layer {
                    self.duplicate_selected_into_layer(target_layer);
                    return;
                }
                if dirty {
                    self.push_history_snapshot(snapshot_before);
                    self.mark_dirty();
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
            let canvas = response.rect.shrink(4.0);
            if response.hovered() {
                let zoom_scroll = ctx.input(|input| input.raw_scroll_delta.y);
                if zoom_scroll.abs() > 0.0 {
                    let factor = (1.0 + zoom_scroll * 0.0015).clamp(0.85, 1.2);
                    self.zoom_canvas(canvas, response.hover_pos(), factor);
                }
            }
            self.clamp_canvas_pan(canvas);
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

            let pointer_pos = response.interact_pointer_pos();
            let hovered_hit = pointer_pos
                .filter(|mouse| content.contains(*mouse))
                .and_then(|mouse| {
                    self.profile
                        .as_ref()
                        .and_then(|profile| hit_test(profile, content, mouse, self.selected))
                });

            if let Some(hit) = hovered_hit {
                match hit {
                    HitTarget::RegionHandle(_, handle) => {
                        ctx.set_cursor_icon(region_handle_cursor(handle));
                    }
                    HitTarget::Region(_) => ctx.set_cursor_icon(egui::CursorIcon::Grab),
                    HitTarget::Point(_) | HitTarget::DragEnd(_) => {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
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
                        hovered_hit.is_some_and(|hit| {
                            hit.idx() == idx
                                && matches!(
                                    hit,
                                    HitTarget::Region(_) | HitTarget::RegionHandle(_, _)
                                )
                        }),
                        !node.layer().trim().is_empty()
                            && self
                                .runtime
                                .active_layers
                                .iter()
                                .any(|layer| layer == node.layer()),
                        self.show_labels,
                    );
                }
            }
            let hovered_summary = hovered_hit.and_then(|hit| {
                self.profile.as_ref().and_then(|profile| {
                    profile.nodes.get(hit.idx()).map(|node| {
                        (
                            display_type(node).to_string(),
                            node.primary_binding().is_some(),
                        )
                    })
                })
            });

            if response.secondary_clicked() {
                if let Some(hit) = hovered_hit {
                    self.focus_selection(hit.idx());
                }
            }

            let mut context_duplicate = false;
            let mut context_delete = false;
            let mut context_copy = false;
            let mut context_paste = false;
            let mut context_move_up = false;
            let mut context_move_down = false;
            let mut context_rebind = None;

            response.context_menu(|ui| {
                if let Some(hit) = hovered_hit {
                    self.focus_selection(hit.idx());
                    if let Some((label, has_primary_binding)) = hovered_summary.as_ref() {
                        ui.label(RichText::new(label).strong());
                        if *has_primary_binding && ui.button("Bind").clicked() {
                            context_rebind = Some(BindingTarget::Primary(hit.idx()));
                            ui.close_menu();
                        }
                    }
                    if ui.button("Copy").clicked() {
                        context_copy = true;
                        ui.close_menu();
                    }
                    if ui.button("Duplicate").clicked() {
                        context_duplicate = true;
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        context_delete = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Move Up").clicked() {
                        context_move_up = true;
                        ui.close_menu();
                    }
                    if ui.button("Move Down").clicked() {
                        context_move_down = true;
                        ui.close_menu();
                    }
                } else {
                    ui.label("Canvas");
                    if self.clipboard.is_some() && ui.button("Paste").clicked() {
                        context_paste = true;
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Reset View").clicked() {
                    self.canvas_zoom = 1.0;
                    self.canvas_pan = Vec2::ZERO;
                    ui.close_menu();
                }
            });

            if context_copy {
                self.copy_selected();
            }
            if context_duplicate {
                self.duplicate_selected();
            }
            if context_delete {
                self.delete_selected();
            }
            if context_paste {
                self.paste_clipboard();
            }
            if context_move_up {
                self.move_selected(-1);
            }
            if context_move_down {
                self.move_selected(1);
            }
            if let Some(target) = context_rebind {
                self.begin_binding_capture(target);
            }

            if let (Some(mouse), Some(hit)) = (pointer_pos, hovered_hit) {
                if self.pending_binding.is_none() {
                    if let Some(profile) = &self.profile {
                        let node = &profile.nodes[hit.idx()];
                        draw_hover_card(&painter, mouse, node, profile.screen.as_ref());
                    }
                }
            }

            let mut overlap_pick = None;
            let mut close_overlap_picker = false;
            if let (Some(picker), Some(profile)) = (&self.overlap_picker, &self.profile) {
                egui::Area::new(egui::Id::new("overlap_picker"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(picker.anchor + Vec2::new(18.0, 18.0))
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(260.0);
                            ui.label(RichText::new("Overlapping controls").strong());
                            ui.label(
                                RichText::new("Choose the control you want to inspect.")
                                    .small()
                                    .color(Color32::from_gray(165)),
                            );
                            ui.add_space(4.0);
                            for idx in &picker.candidates {
                                if let Some(node) = profile.nodes.get(*idx) {
                                    let selected = self.selected == Some(*idx);
                                    if ui
                                        .selectable_label(
                                            selected,
                                            RichText::new(overlap_picker_line(node))
                                                .color(node_color(node)),
                                        )
                                        .clicked()
                                    {
                                        overlap_pick = Some(*idx);
                                    }
                                }
                            }
                            ui.add_space(4.0);
                            if ui.button("Dismiss").clicked() {
                                close_overlap_picker = true;
                            }
                        });
                    });
            }

            if let Some(idx) = overlap_pick {
                self.focus_selection(idx);
            }
            if close_overlap_picker {
                self.overlap_picker = None;
            }

            if let Some(target) = self.pending_binding.as_ref() {
                draw_binding_overlay(&painter, content, target);
                return;
            }

            if response.clicked() {
                if let Some(mouse) = pointer_pos {
                    if content.contains(mouse) {
                        match self.tool {
                            Tool::Place(template) => {
                                self.overlap_picker = None;
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
                                    let point_hits = point_hit_candidates(profile, content, mouse);
                                    if point_hits.len() > 1 {
                                        self.overlap_picker = Some(OverlapPicker {
                                            anchor: mouse,
                                            candidates: point_hits,
                                        });
                                    } else if let Some(hit) =
                                        hit_test(profile, content, mouse, self.selected)
                                    {
                                        self.overlap_picker = None;
                                        self.focus_selection(hit.idx());
                                    } else {
                                        self.overlap_picker = None;
                                        self.selected = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if response.drag_started() {
                if let (Some(mouse), Some(profile)) = (pointer_pos, self.profile.as_ref()) {
                    if ctx.input(|input| {
                        input.key_down(egui::Key::Space) || input.pointer.middle_down()
                    }) {
                        self.overlap_picker = None;
                        self.drag_state = Some(DragState::Pan {
                            origin_pan: self.canvas_pan,
                            start_mouse: mouse,
                        });
                    } else {
                        self.overlap_picker = None;
                        self.drag_state = hit_test(profile, content, mouse, self.selected)
                            .and_then(|hit| match hit {
                                HitTarget::Point(idx) => Some(DragState::Point { idx }),
                                HitTarget::DragEnd(idx) => Some(DragState::DragEnd { idx }),
                                HitTarget::Region(idx) => profile.nodes.get(idx).and_then(|node| {
                                    let region = node_region(node)?;
                                    Some(DragState::RegionMove {
                                        idx,
                                        origin: *region,
                                        start_mouse: mouse,
                                    })
                                }),
                                HitTarget::RegionHandle(idx, handle) => {
                                    profile.nodes.get(idx).and_then(|node| {
                                        let region = node_region(node)?;
                                        Some(DragState::RegionResize {
                                            idx,
                                            handle,
                                            origin: *region,
                                            start_mouse: mouse,
                                        })
                                    })
                                }
                            });
                    }
                    let dragged_idx = match self.drag_state.as_ref() {
                        Some(DragState::Point { idx })
                        | Some(DragState::DragEnd { idx })
                        | Some(DragState::RegionMove { idx, .. })
                        | Some(DragState::RegionResize { idx, .. }) => Some(*idx),
                        _ => None,
                    };
                    if let Some(idx) = dragged_idx {
                        self.begin_edit();
                        self.selected = Some(idx);
                    }
                }
            }

            if response.dragged() {
                if let Some(mouse) = pointer_pos {
                    if let Some(drag) = &self.drag_state {
                        match drag {
                            DragState::Pan {
                                origin_pan,
                                start_mouse,
                            } => {
                                self.canvas_pan = *origin_pan + (mouse - *start_mouse);
                                self.clamp_canvas_pan(canvas);
                            }
                            DragState::Point { idx } => {
                                let mut rel = from_canvas_pos(content, mouse);
                                rel = snap_rel_pos(
                                    self.profile.as_ref(),
                                    *idx,
                                    rel,
                                    self.snap_to_grid,
                                );
                                if let Some(node) = self
                                    .profile
                                    .as_mut()
                                    .and_then(|profile| profile.nodes.get_mut(*idx))
                                {
                                    if let Some(pos) = node_pos_mut(node) {
                                        *pos = rel;
                                        self.mark_dirty();
                                    }
                                }
                            }
                            DragState::DragEnd { idx } => {
                                let mut rel = from_canvas_pos(content, mouse);
                                rel = snap_rel_pos(
                                    self.profile.as_ref(),
                                    *idx,
                                    rel,
                                    self.snap_to_grid,
                                );
                                if let Some(Node::Drag { end, .. }) = self
                                    .profile
                                    .as_mut()
                                    .and_then(|profile| profile.nodes.get_mut(*idx))
                                {
                                    *end = rel;
                                    self.mark_dirty();
                                }
                            }
                            DragState::RegionMove {
                                idx,
                                origin,
                                start_mouse,
                            } => {
                                if let Some(node) = self
                                    .profile
                                    .as_mut()
                                    .and_then(|profile| profile.nodes.get_mut(*idx))
                                {
                                    if let Some(region) = node_region_mut(node) {
                                        let delta = (mouse - *start_mouse)
                                            / Vec2::new(content.width(), content.height());
                                        let next = Region {
                                            x: (origin.x + delta.x as f64)
                                                .clamp(0.0, 1.0 - origin.w),
                                            y: (origin.y + delta.y as f64)
                                                .clamp(0.0, 1.0 - origin.h),
                                            w: origin.w,
                                            h: origin.h,
                                        };
                                        *region = snap_region(next, self.snap_to_grid);
                                        sync_node_pos_from_region(node);
                                        self.mark_dirty();
                                    }
                                }
                            }
                            DragState::RegionResize {
                                idx,
                                handle,
                                origin,
                                start_mouse,
                            } => {
                                if let Some(node) = self
                                    .profile
                                    .as_mut()
                                    .and_then(|profile| profile.nodes.get_mut(*idx))
                                {
                                    if let Some(region) = node_region_mut(node) {
                                        let delta = (mouse - *start_mouse)
                                            / Vec2::new(content.width(), content.height());
                                        let mut next = *origin;
                                        match handle {
                                            RegionHandle::TopLeft => {
                                                next.x = origin.x + delta.x as f64;
                                                next.y = origin.y + delta.y as f64;
                                                next.w = origin.w - delta.x as f64;
                                                next.h = origin.h - delta.y as f64;
                                            }
                                            RegionHandle::Top => {
                                                next.y = origin.y + delta.y as f64;
                                                next.h = origin.h - delta.y as f64;
                                            }
                                            RegionHandle::TopRight => {
                                                next.y = origin.y + delta.y as f64;
                                                next.w = origin.w + delta.x as f64;
                                                next.h = origin.h - delta.y as f64;
                                            }
                                            RegionHandle::Right => {
                                                next.w = origin.w + delta.x as f64;
                                            }
                                            RegionHandle::BottomLeft => {
                                                next.x = origin.x + delta.x as f64;
                                                next.w = origin.w - delta.x as f64;
                                                next.h = origin.h + delta.y as f64;
                                            }
                                            RegionHandle::Bottom => {
                                                next.h = origin.h + delta.y as f64;
                                            }
                                            RegionHandle::BottomRight => {
                                                next.w = origin.w + delta.x as f64;
                                                next.h = origin.h + delta.y as f64;
                                            }
                                            RegionHandle::Left => {
                                                next.x = origin.x + delta.x as f64;
                                                next.w = origin.w - delta.x as f64;
                                            }
                                        }
                                        *region =
                                            snap_region(clamp_region(next), self.snap_to_grid);
                                        sync_node_pos_from_region(node);
                                        self.mark_dirty();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if response.drag_stopped() {
                self.drag_state = None;
            }

            painter.text(
                Pos2::new(content.right() - 12.0, content.bottom() - 12.0),
                Align2::RIGHT_BOTTOM,
                "Mouse wheel zoom • Space+drag pan • Right-click actions",
                egui::FontId::proportional(11.0),
                Color32::from_white_alpha(140),
            );
        });
    }
}

impl eframe::App for PhantomGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let prefs_before = self.studio_prefs();
        self.handle_shortcuts(ctx);
        self.handle_binding_capture(ctx);
        self.maybe_poll_runtime();
        ctx.request_repaint_after(self.background_tick_interval());

        self.draw_top_bar(ctx);
        self.draw_left_panel(ctx);
        self.draw_properties_panel(ctx);
        self.draw_canvas(ctx);
        if self.studio_prefs() != prefs_before {
            self.persist_studio_prefs();
        }
    }
}

fn studio_prefs_path() -> PathBuf {
    config::config_dir().join("studio.toml")
}

fn studio_daemon_log_path() -> PathBuf {
    config::config_dir().join("daemon.log")
}

fn load_studio_prefs() -> StudioPrefs {
    let path = studio_prefs_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(prefs) => prefs,
            Err(e) => {
                tracing::warn!("invalid studio prefs at {}: {}", path.display(), e);
                StudioPrefs::default()
            }
        },
        Err(_) => StudioPrefs::default(),
    }
}

fn available_profile_paths() -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let dir = config::profiles_dir();
    let Ok(read_dir) = std::fs::read_dir(&dir) else {
        return entries;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") && path.is_file() {
            entries.push(path);
        }
    }

    entries.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    entries
}

fn profile_menu_label(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().replace('-', " "))
        .unwrap_or_else(|| path.display().to_string())
}

fn hotkey_label(value: Option<Key>) -> String {
    match value {
        Some(key) => format!("{:?}", key),
        None => "disabled".into(),
    }
}

fn running_as_root() -> bool {
    std::env::var("USER").ok().as_deref() == Some("root")
}

fn command_exists(name: &str) -> bool {
    find_binary_in_path(name).is_some()
}

fn find_binary_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_phantom_binary() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("phantom"));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("target/release/phantom"));
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/bin/phantom"));
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    find_binary_in_path("phantom")
}

#[derive(Clone, Copy)]
enum HitTarget {
    Point(usize),
    DragEnd(usize),
    Region(usize),
    RegionHandle(usize, RegionHandle),
}

impl HitTarget {
    fn idx(self) -> usize {
        match self {
            HitTarget::Point(idx)
            | HitTarget::DragEnd(idx)
            | HitTarget::Region(idx)
            | HitTarget::RegionHandle(idx, _) => idx,
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
        if let Some(Node::Drag { end, .. }) = profile.nodes.get(idx) {
            let end_point = to_canvas_pos(content, end);
            if (end_point - mouse).length() <= 14.0 {
                return Some(HitTarget::DragEnd(idx));
            }
        }
        if let Some(region) = profile.nodes.get(idx).and_then(node_region) {
            let rect = region_rect(content, region);
            for (handle, handle_rect) in region_handles(rect) {
                if handle_rect.contains(mouse) {
                    return Some(HitTarget::RegionHandle(idx, handle));
                }
            }
            if region_border_contains(rect, mouse) {
                return Some(HitTarget::Region(idx));
            }
        }
    }

    for (idx, node) in profile.nodes.iter().enumerate().rev() {
        if Some(idx) == selected {
            continue;
        }
        if let Some(region) = node_region(node) {
            let rect = region_rect(content, region);
            for (handle, handle_rect) in region_handles(rect) {
                if handle_rect.contains(mouse) {
                    return Some(HitTarget::RegionHandle(idx, handle));
                }
            }
        }
    }

    let mut best_point = None;
    for (idx, node) in profile.nodes.iter().enumerate() {
        if let Some(pos) = node_pos(node) {
            let point = to_canvas_pos(content, pos);
            let distance = (point - mouse).length();
            if distance <= 22.0
                && best_point.is_none_or(|(_, best_distance)| distance < best_distance)
            {
                best_point = Some((idx, distance));
            }
        }
    }
    if let Some((idx, _)) = best_point {
        return Some(HitTarget::Point(idx));
    }

    for (idx, node) in profile.nodes.iter().enumerate() {
        if let Some(region) = node_region(node) {
            if region_border_contains(region_rect(content, region), mouse) {
                return Some(HitTarget::Region(idx));
            }
        }
    }

    None
}

fn point_hit_candidates(profile: &Profile, content: Rect, mouse: Pos2) -> Vec<usize> {
    let mut hits = Vec::new();
    for (idx, node) in profile.nodes.iter().enumerate() {
        let Some(pos) = node_pos(node) else {
            continue;
        };
        let point = to_canvas_pos(content, pos);
        let distance = (point - mouse).length();
        if distance <= 22.0 {
            hits.push((idx, distance));
        }
    }
    hits.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.0.cmp(&a.0))
    });
    hits.into_iter().map(|(idx, _)| idx).collect()
}

fn overlap_picker_line(node: &Node) -> String {
    let mut line = format!("{} • {}", display_binding(node), display_type(node));
    match (node.slot(), node.layer().trim().is_empty()) {
        (Some(slot), true) => line.push_str(&format!(" • slot {}", slot)),
        (Some(slot), false) => line.push_str(&format!(" • slot {} • {}", slot, node.layer())),
        (None, false) => line.push_str(&format!(" • {}", node.layer())),
        (None, true) => {}
    }
    line
}

fn draw_node(
    painter: &egui::Painter,
    content: Rect,
    node: &Node,
    selected: bool,
    hovered_region: bool,
    layer_active: bool,
    show_labels: bool,
) {
    let color = node_color(node);
    let accent = if layer_active {
        Color32::from_rgb(255, 218, 121)
    } else {
        Color32::WHITE
    };
    if let Some(region) = node_region(node) {
        let rect = region_rect(content, region);
        draw_dashed_rect(
            painter,
            rect,
            Stroke::new(
                if selected || hovered_region { 3.0 } else { 2.0 },
                if selected || hovered_region || layer_active {
                    accent
                } else {
                    color.gamma_multiply(0.85)
                },
            ),
        );
        if let Node::Joystick { radius, .. } = node {
            let center = to_canvas_pos(content, &region_center(region));
            let radius_px = (*radius as f32 * content.width()).max(16.0);
            painter.circle_stroke(
                center,
                radius_px,
                Stroke::new(2.0, color.gamma_multiply(0.65)),
            );
            let marker_radius = if selected { 16.0 } else { 12.0 };
            painter.circle_filled(center, marker_radius, color);
            if selected || layer_active {
                painter.circle_stroke(center, marker_radius + 3.0, Stroke::new(2.0, accent));
            }
            painter.text(
                center,
                Align2::CENTER_CENTER,
                marker_glyph(node),
                egui::FontId::proportional(12.0),
                Color32::BLACK,
            );
            if show_labels {
                draw_joystick_compass_labels(
                    painter,
                    center,
                    radius_px,
                    node,
                    accent,
                    layer_active,
                );
            }
        }
        if selected || hovered_region {
            for (_, handle_rect) in region_handles(rect) {
                painter.rect_filled(
                    handle_rect,
                    2.0,
                    if selected {
                        accent
                    } else {
                        accent.gamma_multiply(0.72)
                    },
                );
            }
        }
        if show_labels {
            draw_region_badge(
                painter,
                rect,
                region_badge_text(node),
                if layer_active { accent } else { color },
            );
        }
        return;
    }

    let Some(pos) = node_pos(node) else {
        return;
    };
    let point = to_canvas_pos(content, pos);

    if let Node::Drag { end, .. } = node {
        let end_point = to_canvas_pos(content, end);
        painter.line_segment(
            [point, end_point],
            Stroke::new(if selected { 3.0 } else { 2.0 }, color.gamma_multiply(0.8)),
        );
        painter.circle_stroke(end_point, 8.0, Stroke::new(2.0, color.gamma_multiply(0.9)));
    }

    if let Node::Joystick { radius, .. } = node {
        let radius_px = (*radius as f32 * content.width()).max(16.0);
        painter.circle_stroke(
            point,
            radius_px,
            Stroke::new(2.0, color.gamma_multiply(0.65)),
        );
        if show_labels {
            draw_joystick_compass_labels(painter, point, radius_px, node, accent, layer_active);
        }
    }

    let marker_radius = if selected { 16.0 } else { 12.0 };
    painter.circle_filled(point, marker_radius, color);
    if selected || layer_active {
        painter.circle_stroke(point, marker_radius + 3.0, Stroke::new(2.0, accent));
    }
    painter.text(
        point,
        Align2::CENTER_CENTER,
        marker_glyph(node),
        egui::FontId::proportional(12.0),
        Color32::BLACK,
    );
    if show_labels && !matches!(node, Node::Joystick { .. }) {
        painter.text(
            point + Vec2::new(0.0, marker_radius + 6.0),
            Align2::CENTER_TOP,
            display_binding(node),
            egui::FontId::proportional(11.0),
            if layer_active { accent } else { Color32::WHITE },
        );
    }
}

fn display_type(node: &Node) -> &'static str {
    match node {
        Node::Tap { .. } => "Tap Button",
        Node::HoldTap { .. } => "Hold Button",
        Node::ToggleTap { .. } => "Toggle Button",
        Node::Joystick { .. } => "Left Stick",
        Node::Drag { .. } => "Drag Gesture",
        Node::MouseCamera { .. } => "Aim",
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
        | Node::Drag { key, .. }
        | Node::RepeatTap { key, .. }
        | Node::Macro { key, .. }
        | Node::LayerShift { key, .. } => key.clone(),
        Node::Joystick { keys, .. } => {
            format!("{}/{}/{}/{}", keys.up, keys.left, keys.down, keys.right)
        }
        Node::MouseCamera {
            activation_mode,
            activation_key,
            ..
        } => match activation_mode {
            MouseCameraActivationMode::AlwaysOn => "Aim".into(),
            MouseCameraActivationMode::WhileHeld => {
                format!("Aim: hold {}", activation_key.as_deref().unwrap_or("?"))
            }
            MouseCameraActivationMode::Toggle => {
                format!("Aim: toggle {}", activation_key.as_deref().unwrap_or("?"))
            }
        },
    }
}

fn region_badge_text(node: &Node) -> String {
    match node {
        Node::Joystick { .. } => display_type(node).into(),
        _ => display_binding(node),
    }
}

fn profile_source_badge(path: Option<&Path>) -> (String, Color32) {
    match path {
        None => ("Unsaved draft".into(), Color32::from_rgb(255, 218, 121)),
        Some(path) if path.starts_with(config::profiles_dir()) => {
            ("Library profile".into(), Color32::from_rgb(182, 255, 216))
        }
        Some(_) => ("External profile".into(), Color32::from_rgb(166, 208, 255)),
    }
}

fn draw_control_card(
    ui: &mut egui::Ui,
    idx: usize,
    node: &Node,
    selected: bool,
    is_layer_active: bool,
) -> Option<ControlListAction> {
    let title = display_binding(node);
    let kind = display_type(node);
    let color = node_color(node);
    let accent = if is_layer_active {
        Color32::from_rgb(255, 218, 121)
    } else {
        Color32::from_gray(150)
    };
    let metadata = match (node.slot(), node.layer().trim().is_empty()) {
        (Some(slot), true) => format!("slot {}", slot),
        (Some(slot), false) => format!("slot {} • layer {}", slot, node.layer()),
        (None, true) => "runtime".into(),
        (None, false) => format!("layer {}", node.layer()),
    };

    let mut action = None;
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::Frame::group(ui.style())
                .stroke(Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        color.gamma_multiply(0.95)
                    } else {
                        Color32::from_gray(60)
                    },
                ))
                .fill(if selected {
                    Color32::from_white_alpha(10)
                } else {
                    Color32::from_black_alpha(16)
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let response = ui.add_sized(
                        [ui.available_width(), 24.0],
                        egui::SelectableLabel::new(
                            selected,
                            RichText::new(title.as_str()).strong().color(color),
                        ),
                    );
                    if response.clicked() {
                        action = Some(ControlListAction::Select(idx));
                    }

                    if selected {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if action_button(ui, "Del", "Delete").clicked() {
                                        action = Some(ControlListAction::Delete(idx));
                                    }
                                    if action_button(ui, "Dup", "Duplicate").clicked() {
                                        action = Some(ControlListAction::Duplicate(idx));
                                    }
                                    if action_button(ui, "Dn", "Move down").clicked() {
                                        action = Some(ControlListAction::MoveDown(idx));
                                    }
                                    if action_button(ui, "Up", "Move up").clicked() {
                                        action = Some(ControlListAction::MoveUp(idx));
                                    }
                                },
                            );
                        });
                    }

                    ui.label(RichText::new(kind).small().color(Color32::from_gray(205)));
                    ui.label(RichText::new(metadata).small().color(accent));
                    if !node.id().is_empty() {
                        ui.label(RichText::new(node.id()).small().italics().weak());
                    }
                });
        },
    );
    action
}

fn action_button<'a>(ui: &'a mut egui::Ui, text: &'a str, tooltip: &'a str) -> egui::Response {
    ui.add_sized(
        [CONTROL_CARD_ACTION_BUTTON_WIDTH, 20.0],
        egui::Button::new(RichText::new(text).small()),
    )
    .on_hover_text(tooltip)
}

fn node_color(node: &Node) -> Color32 {
    match node {
        Node::Tap { .. } => COLOR_TAP,
        Node::HoldTap { .. } => COLOR_HOLD,
        Node::ToggleTap { .. } => COLOR_TOGGLE,
        Node::Joystick { .. } => COLOR_JOYSTICK,
        Node::Drag { .. } => COLOR_DRAG,
        Node::MouseCamera { .. } => COLOR_LOOK,
        Node::RepeatTap { .. } => COLOR_REPEAT,
        Node::Macro { .. } => COLOR_MACRO,
        Node::LayerShift { .. } => COLOR_LAYER,
    }
}

fn node_region(node: &Node) -> Option<&Region> {
    match node {
        Node::Joystick {
            mode: JoystickMode::Floating,
            region: Some(region),
            ..
        } => Some(region),
        _ => None,
    }
}

fn node_region_mut(node: &mut Node) -> Option<&mut Region> {
    match node {
        Node::Joystick {
            mode: JoystickMode::Floating,
            region: Some(region),
            ..
        } => Some(region),
        _ => None,
    }
}

fn sync_node_pos_from_region(node: &mut Node) {
    if let Node::Joystick {
        mode: JoystickMode::Floating,
        pos,
        region: Some(region),
        ..
    } = node
    {
        *pos = region_center(region);
    }
}

fn marker_glyph(node: &Node) -> &'static str {
    match node {
        Node::Tap { .. } => "T",
        Node::HoldTap { .. } => "H",
        Node::ToggleTap { .. } => "G",
        Node::Joystick { .. } => "J",
        Node::Drag { .. } => "D",
        Node::MouseCamera { .. } => "A",
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
        | Node::Drag { start: pos, .. }
        | Node::RepeatTap { pos, .. } => Some(pos),
        Node::Joystick {
            mode: JoystickMode::Fixed,
            pos,
            ..
        }
        | Node::MouseCamera { anchor: pos, .. } => Some(pos),
        Node::Joystick {
            mode: JoystickMode::Floating,
            ..
        }
        | Node::Macro { .. }
        | Node::LayerShift { .. } => None,
    }
}

fn node_pos_mut(node: &mut Node) -> Option<&mut RelPos> {
    match node {
        Node::Tap { pos, .. }
        | Node::HoldTap { pos, .. }
        | Node::ToggleTap { pos, .. }
        | Node::Drag { start: pos, .. }
        | Node::RepeatTap { pos, .. } => Some(pos),
        Node::Joystick {
            mode: JoystickMode::Fixed,
            pos,
            ..
        }
        | Node::MouseCamera { anchor: pos, .. } => Some(pos),
        Node::Joystick {
            mode: JoystickMode::Floating,
            ..
        }
        | Node::Macro { .. }
        | Node::LayerShift { .. } => None,
    }
}

fn set_node_id(node: &mut Node, id: String) {
    match node {
        Node::Tap { id: field, .. }
        | Node::HoldTap { id: field, .. }
        | Node::ToggleTap { id: field, .. }
        | Node::Joystick { id: field, .. }
        | Node::Drag { id: field, .. }
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
        Tool::Place(NodeTemplate::Drag) => "drag gesture",
        Tool::Place(NodeTemplate::MouseLook) => "aim anchor",
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
        BindingTarget::MouseLookActivation(_) => "aim activator",
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

fn position_editor(
    ui: &mut egui::Ui,
    pos: &mut RelPos,
    screen: Option<&ScreenOverride>,
    dirty: &mut bool,
) {
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
    if let Some(screen) = screen {
        let (px, py) = rel_to_pixels(pos, screen);
        ui.label(
            RichText::new(format!("Pixels: {}, {}", px, py))
                .small()
                .color(Color32::from_gray(170)),
        );
    }
}

fn region_editor(
    ui: &mut egui::Ui,
    region: &mut Region,
    screen: Option<&ScreenOverride>,
    dirty: &mut bool,
) {
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
    *region = clamp_region(*region);
    if let Some(screen) = screen {
        let origin = RelPos {
            x: region.x,
            y: region.y,
        };
        let size = (
            (region.w * screen.width as f64).round() as i32,
            (region.h * screen.height as f64).round() as i32,
        );
        let (px, py) = rel_to_pixels(&origin, screen);
        ui.label(
            RichText::new(format!(
                "Pixels: origin {}, {} • size {}x{}",
                px, py, size.0, size.1
            ))
            .small()
            .color(Color32::from_gray(170)),
        );
    }
}

fn layer_row(
    ui: &mut egui::Ui,
    layer: &mut String,
    id_source: impl std::hash::Hash,
    layer_choices: &[String],
    suggested_layer: Option<&str>,
    dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Layer");
        if ui.text_edit_singleline(layer).changed() {
            *dirty = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(layer.trim().is_empty(), "Base")
            .on_hover_text("No layer name means this control is always active.")
            .clicked()
        {
            layer.clear();
            *dirty = true;
        }

        if let Some(suggested) = suggested_layer {
            if !suggested.trim().is_empty()
                && ui
                    .selectable_label(layer.trim() == suggested, format!("Use {}", suggested))
                    .on_hover_text("Apply the currently filtered layer name.")
                    .clicked()
            {
                *layer = suggested.to_string();
                *dirty = true;
            }
        }

        if !layer_choices.is_empty() {
            egui::ComboBox::from_id_salt(("layer_choice", id_source))
                .selected_text("Existing layers")
                .show_ui(ui, |ui| {
                    for choice in layer_choices {
                        let selected = layer.trim() == choice;
                        if ui.selectable_label(selected, choice).clicked() {
                            *layer = choice.clone();
                            *dirty = true;
                        }
                    }
                });
        }
    });
    ui.label(
        RichText::new(
            "Base controls are always live. Named layers only activate through Layer Switch nodes.",
        )
        .small()
        .color(Color32::from_gray(160)),
    );
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
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.monospace(key.as_str());
            if pending == Some(target) {
                ui.label(
                    RichText::new("Listening")
                        .strong()
                        .color(Color32::from_rgb(255, 218, 121)),
                );
            }
        });
        let capture_text = if pending == Some(target) {
            "Press key or click mouse"
        } else {
            "Bind"
        };
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [140.0, 28.0],
                    egui::Button::new(capture_text).fill(if pending == Some(target) {
                        Color32::from_rgb(77, 62, 25)
                    } else {
                        Color32::from_rgb(36, 57, 87)
                    }),
                )
                .clicked()
            {
                *start_binding = Some(target.clone());
            }
            ui.menu_button("More keys", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for candidate in BINDABLE_KEYS {
                            if ui
                                .selectable_label(*key == *candidate, *candidate)
                                .clicked()
                            {
                                *key = (*candidate).to_string();
                                *dirty = true;
                                ui.close_menu();
                            }
                        }
                    });
            });
        });
        ui.label(
            RichText::new("Bind is the primary path. The key list is only for rare cases.")
                .small()
                .color(Color32::from_gray(150)),
        );
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

fn rel_to_pixels(pos: &RelPos, screen: &ScreenOverride) -> (i32, i32) {
    (
        (pos.x * screen.width as f64).round() as i32,
        (pos.y * screen.height as f64).round() as i32,
    )
}

fn region_center(region: &Region) -> RelPos {
    RelPos {
        x: round3(region.x + region.w / 2.0),
        y: round3(region.y + region.h / 2.0),
    }
}

fn default_joystick_region(center: RelPos) -> Region {
    clamp_region(Region {
        x: center.x - 0.18,
        y: center.y - 0.18,
        w: 0.36,
        h: 0.36,
    })
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

fn region_border_contains(rect: Rect, mouse: Pos2) -> bool {
    let outer = rect.expand(REGION_PICK_MARGIN);
    if !outer.contains(mouse) {
        return false;
    }

    let inset = REGION_BORDER_PICK_WIDTH.min((rect.width().min(rect.height()) / 2.0) - 2.0);
    if inset <= 0.0 {
        return rect.contains(mouse);
    }

    !rect.shrink(inset).contains(mouse)
}

fn region_handles(rect: Rect) -> [(RegionHandle, Rect); 8] {
    let mid_top = Pos2::new(rect.center().x, rect.top());
    let mid_right = Pos2::new(rect.right(), rect.center().y);
    let mid_bottom = Pos2::new(rect.center().x, rect.bottom());
    let mid_left = Pos2::new(rect.left(), rect.center().y);
    [
        (
            RegionHandle::TopLeft,
            Rect::from_center_size(rect.left_top(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::Top,
            Rect::from_center_size(mid_top, Vec2::new(HANDLE_SIZE * 1.35, HANDLE_SIZE)),
        ),
        (
            RegionHandle::TopRight,
            Rect::from_center_size(rect.right_top(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::Right,
            Rect::from_center_size(mid_right, Vec2::new(HANDLE_SIZE, HANDLE_SIZE * 1.35)),
        ),
        (
            RegionHandle::BottomLeft,
            Rect::from_center_size(rect.left_bottom(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::Bottom,
            Rect::from_center_size(mid_bottom, Vec2::new(HANDLE_SIZE * 1.35, HANDLE_SIZE)),
        ),
        (
            RegionHandle::BottomRight,
            Rect::from_center_size(rect.right_bottom(), Vec2::splat(HANDLE_SIZE)),
        ),
        (
            RegionHandle::Left,
            Rect::from_center_size(mid_left, Vec2::new(HANDLE_SIZE, HANDLE_SIZE * 1.35)),
        ),
    ]
}

fn region_handle_cursor(handle: RegionHandle) -> egui::CursorIcon {
    match handle {
        RegionHandle::TopLeft | RegionHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
        RegionHandle::TopRight | RegionHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
        RegionHandle::Top | RegionHandle::Bottom => egui::CursorIcon::ResizeVertical,
        RegionHandle::Left | RegionHandle::Right => egui::CursorIcon::ResizeHorizontal,
    }
}

fn draw_dashed_rect(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    draw_dashed_line(painter, rect.left_top(), rect.right_top(), stroke);
    draw_dashed_line(painter, rect.right_top(), rect.right_bottom(), stroke);
    draw_dashed_line(painter, rect.right_bottom(), rect.left_bottom(), stroke);
    draw_dashed_line(painter, rect.left_bottom(), rect.left_top(), stroke);
}

fn draw_dashed_line(painter: &egui::Painter, start: Pos2, end: Pos2, stroke: Stroke) {
    let delta = end - start;
    let length = delta.length();
    if length <= 0.0 {
        return;
    }
    let direction = delta / length;
    let mut traveled = 0.0;
    while traveled < length {
        let dash_end = (traveled + DASH_LENGTH).min(length);
        let from = start + direction * traveled;
        let to = start + direction * dash_end;
        painter.line_segment([from, to], stroke);
        traveled += DASH_LENGTH + DASH_GAP;
    }
}

fn draw_region_badge(painter: &egui::Painter, rect: Rect, text: String, color: Color32) {
    let width = (text.len() as f32 * 7.2).clamp(52.0, 180.0);
    let badge = Rect::from_min_size(
        rect.left_top() + Vec2::new(10.0, 10.0),
        Vec2::new(width, 22.0),
    );
    painter.rect_filled(badge, 999.0, Color32::from_black_alpha(168));
    painter.rect_stroke(
        badge,
        999.0,
        Stroke::new(1.5, color.gamma_multiply(0.95)),
        StrokeKind::Outside,
    );
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(11.0),
        color,
    );
}

fn draw_joystick_compass_labels(
    painter: &egui::Painter,
    center: Pos2,
    radius_px: f32,
    node: &Node,
    accent: Color32,
    layer_active: bool,
) {
    let Node::Joystick { keys, .. } = node else {
        return;
    };
    let text_color = if layer_active { accent } else { Color32::WHITE };
    let label_radius = radius_px + 20.0;
    draw_key_chip(
        painter,
        center + Vec2::new(0.0, -label_radius),
        &keys.up,
        text_color,
    );
    draw_key_chip(
        painter,
        center + Vec2::new(-label_radius, 0.0),
        &keys.left,
        text_color,
    );
    draw_key_chip(
        painter,
        center + Vec2::new(label_radius, 0.0),
        &keys.right,
        text_color,
    );
    draw_key_chip(
        painter,
        center + Vec2::new(0.0, label_radius),
        &keys.down,
        text_color,
    );
}

fn draw_key_chip(painter: &egui::Painter, center: Pos2, text: &str, text_color: Color32) {
    let width = (text.len() as f32 * 7.5).clamp(24.0, 68.0);
    let rect = Rect::from_center_size(center, Vec2::new(width, 22.0));
    painter.rect_filled(rect, 999.0, Color32::from_black_alpha(178));
    painter.rect_stroke(
        rect,
        999.0,
        Stroke::new(1.0, text_color.gamma_multiply(0.8)),
        StrokeKind::Outside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(11.0),
        text_color,
    );
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

fn snap_rel_pos(
    profile: Option<&Profile>,
    current_idx: usize,
    mut pos: RelPos,
    enabled: bool,
) -> RelPos {
    if !enabled {
        pos.x = round3(pos.x);
        pos.y = round3(pos.y);
        return pos;
    }

    pos.x = snap_scalar(pos.x);
    pos.y = snap_scalar(pos.y);

    if let Some(profile) = profile {
        for (idx, node) in profile.nodes.iter().enumerate() {
            if idx == current_idx {
                continue;
            }
            if let Some(other) = node_pos(node) {
                if (pos.x - other.x).abs() <= SNAP_THRESHOLD {
                    pos.x = other.x;
                }
                if (pos.y - other.y).abs() <= SNAP_THRESHOLD {
                    pos.y = other.y;
                }
            }
        }
    }

    pos.x = round3(pos.x.clamp(0.0, 1.0));
    pos.y = round3(pos.y.clamp(0.0, 1.0));
    pos
}

fn snap_region(region: Region, enabled: bool) -> Region {
    if !enabled {
        return clamp_region(region);
    }
    clamp_region(Region {
        x: snap_scalar(region.x),
        y: snap_scalar(region.y),
        w: snap_scalar(region.w),
        h: snap_scalar(region.h),
    })
}

fn snap_scalar(value: f64) -> f64 {
    let snapped = (value / SNAP_GRID_STEP).round() * SNAP_GRID_STEP;
    if (value - snapped).abs() <= SNAP_THRESHOLD {
        snapped
    } else {
        value
    }
}

fn offset_rel_pos(pos: RelPos) -> RelPos {
    RelPos {
        x: round3((pos.x + 0.02).clamp(0.0, 1.0)),
        y: round3((pos.y + 0.02).clamp(0.0, 1.0)),
    }
}

fn offset_region(region: Region) -> Region {
    clamp_region(Region {
        x: region.x + 0.02,
        y: region.y + 0.02,
        w: region.w,
        h: region.h,
    })
}

fn node_id_prefix(node: &Node) -> &'static str {
    match node {
        Node::Tap { .. } => "tap",
        Node::HoldTap { .. } => "hold",
        Node::ToggleTap { .. } => "toggle",
        Node::Joystick { .. } => "stick",
        Node::Drag { .. } => "drag",
        Node::MouseCamera { .. } => "aim",
        Node::RepeatTap { .. } => "rapid",
        Node::Macro { .. } => "macro",
        Node::LayerShift { .. } => "layer",
    }
}

fn zoom_rect(rect: Rect, zoom: f32, pan: Vec2) -> Rect {
    let size = rect.size() * zoom;
    Rect::from_center_size(rect.center() + pan, size)
}

fn draw_hover_card(
    painter: &egui::Painter,
    mouse: Pos2,
    node: &Node,
    screen: Option<&ScreenOverride>,
) {
    let mut lines = vec![
        display_type(node).to_string(),
        format!("Binding: {}", display_binding(node)),
    ];
    if let Some(slot) = node.slot() {
        lines.push(format!("Slot: {}", slot));
    }
    if !node.layer().trim().is_empty() {
        lines.push(format!("Layer: {}", node.layer()));
    }
    match node {
        Node::Tap { pos, .. }
        | Node::HoldTap { pos, .. }
        | Node::ToggleTap { pos, .. }
        | Node::RepeatTap { pos, .. } => {
            lines.push(format!("Pos: {:.3}, {:.3}", pos.x, pos.y));
            if let Some(screen) = screen {
                let (px, py) = rel_to_pixels(pos, screen);
                lines.push(format!("Pixels: {}, {}", px, py));
            }
        }
        Node::Joystick {
            pos,
            radius,
            mode,
            region,
            ..
        } => {
            lines.push(format!(
                "Mode: {}",
                match mode {
                    JoystickMode::Fixed => "fixed",
                    JoystickMode::Floating => "floating",
                }
            ));
            lines.push(format!("Radius: {:.3}", radius));
            match (mode, region) {
                (JoystickMode::Fixed, _) => {
                    lines.push(format!("Pos: {:.3}, {:.3}", pos.x, pos.y));
                    if let Some(screen) = screen {
                        let (px, py) = rel_to_pixels(pos, screen);
                        lines.push(format!("Pixels: {}, {}", px, py));
                    }
                }
                (JoystickMode::Floating, Some(region)) => {
                    lines.push(format!(
                        "Zone: {:.3}, {:.3}, {:.3}, {:.3}",
                        region.x, region.y, region.w, region.h
                    ));
                }
                (JoystickMode::Floating, None) => {
                    lines.push("Zone: missing".into());
                }
            }
        }
        Node::Drag {
            start,
            end,
            duration_ms,
            ..
        } => {
            lines.push(format!("Start: {:.3}, {:.3}", start.x, start.y));
            lines.push(format!("End: {:.3}, {:.3}", end.x, end.y));
            lines.push(format!("Duration: {} ms", duration_ms));
            if let Some(screen) = screen {
                let (start_x, start_y) = rel_to_pixels(start, screen);
                let (end_x, end_y) = rel_to_pixels(end, screen);
                lines.push(format!(
                    "Pixels: {} , {} -> {} , {}",
                    start_x, start_y, end_x, end_y
                ));
            }
        }
        Node::MouseCamera {
            anchor,
            reach,
            activation_mode,
            activation_key,
            sensitivity,
            ..
        } => {
            lines.push(format!("Anchor: {:.3}, {:.3}", anchor.x, anchor.y));
            lines.push(format!("Reach: {:.3}", reach));
            lines.push(format!("Sensitivity: {:.2}", sensitivity));
            lines.push(format!(
                "Mode: {}",
                match activation_mode {
                    MouseCameraActivationMode::AlwaysOn => "always_on",
                    MouseCameraActivationMode::WhileHeld => "while_held",
                    MouseCameraActivationMode::Toggle => "toggle",
                }
            ));
            if let Some(key) = activation_key {
                lines.push(format!("Activation: {}", key));
            }
        }
        Node::Macro { sequence, .. } => lines.push(format!("Steps: {}", sequence.len())),
        Node::LayerShift {
            layer_name, mode, ..
        } => lines.push(format!(
            "Target: {} ({})",
            layer_name,
            match mode {
                LayerMode::Hold => "hold",
                LayerMode::Toggle => "toggle",
            }
        )),
    }

    let line_height = 16.0;
    let width = 220.0;
    let height = 12.0 + line_height * lines.len() as f32;
    let rect = Rect::from_min_size(mouse + Vec2::new(16.0, 16.0), Vec2::new(width, height));
    painter.rect_filled(rect, 8.0, Color32::from_black_alpha(220));
    for (idx, line) in lines.iter().enumerate() {
        painter.text(
            rect.left_top() + Vec2::new(10.0, 8.0 + idx as f32 * line_height),
            Align2::LEFT_TOP,
            line,
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}

fn draw_binding_overlay(painter: &egui::Painter, content: Rect, target: &BindingTarget) {
    let label = format!(
        "Binding {}. Press a key or mouse button. Esc cancels.",
        binding_target_label(target)
    );
    let banner_rect = Rect::from_center_size(
        Pos2::new(content.center().x, content.top() + 34.0),
        Vec2::new(420.0, 40.0),
    );
    painter.rect_filled(content, 10.0, Color32::from_black_alpha(90));
    painter.rect_filled(banner_rect, 10.0, Color32::from_black_alpha(220));
    painter.text(
        banner_rect.center(),
        Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
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
    if let Some(profile_path) = overlay_profile_arg() {
        return overlay::run_overlay(&profile_path);
    }
    if should_print_help() {
        print_help();
        return Ok(());
    }
    if should_print_version() {
        print_version();
        return Ok(());
    }

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
            .with_title(format!(
                "Phantom GUI {} — Mapping GUI",
                env!("CARGO_PKG_VERSION")
            )),
        ..Default::default()
    };

    eframe::run_native(
        "phantom-gui",
        options,
        Box::new(|cc| Ok(Box::new(PhantomGui::new(cc)))),
    )
}

fn should_print_help() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    args.first().map(|arg| arg.as_str()) == Some("help")
        || args.iter().any(|arg| arg == "-h" || arg == "--help")
}

fn should_print_version() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    args.first().map(|arg| arg.as_str()) == Some("version")
        || args.iter().any(|arg| arg == "-V" || arg == "--version")
}

fn print_help() {
    let binary = current_gui_binary_name();
    println!(
        r#"Phantom GUI {version}

Fullscreen mapping GUI and runtime control surface for Phantom.

USAGE:
    {binary}
    {binary} --overlay <profile.json>
    {binary} version

FLAGS:
    -h, --help       Show this help
    -V, --version    Show version

INTERNAL:
    --overlay <profile.json>    Launch the experimental debug overlay preview"#,
        binary = binary,
        version = env!("CARGO_PKG_VERSION"),
    );
}

fn print_version() {
    println!(
        "Phantom GUI {} ({})",
        env!("CARGO_PKG_VERSION"),
        current_gui_binary_name()
    );
}

fn current_gui_binary_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|path| {
            PathBuf::from(path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string())
}

fn overlay_profile_arg() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--overlay" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

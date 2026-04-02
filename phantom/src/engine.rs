use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::input::{InputEvent, Key};
use crate::profile::{
    JoystickMode, LayerMode, MacroAction, MouseCameraActivationMode, Node, Profile, Region,
};

const MOUSE_LOOK_IDLE_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub enum TouchCommand {
    TouchDown { slot: u8, x: f64, y: f64 },
    TouchMove { slot: u8, x: f64, y: f64 },
    TouchUp { slot: u8 },
}

#[derive(Debug)]
enum NodeState {
    Tap {
        active: bool,
    },
    HoldTap {
        held: bool,
    },
    ToggleTap {
        active: bool,
    },
    Joystick {
        up: bool,
        down: bool,
        left: bool,
        right: bool,
        finger_active: bool,
        origin_x: f64,
        origin_y: f64,
    },
    Drag {
        running: bool,
        started_at: Instant,
        last_progress: f64,
    },
    MouseCamera {
        enabled: bool,
        finger_active: bool,
        current_x: f64,
        current_y: f64,
        last_motion: Instant,
    },
    RepeatTap {
        active: bool,
        last_toggle: Instant,
        finger_down: bool,
    },
    Macro {
        running: bool,
        step_index: usize,
        step_start: Instant,
        active_slots: Vec<u8>,
    },
    LayerShift {
        held: bool,
    },
}

pub struct KeymapEngine {
    profile: Profile,
    key_bindings: HashMap<Key, Vec<usize>>,
    joystick_bindings: Vec<Option<JoystickBinding>>,
    states: Vec<NodeState>,
    sensitivity: f64,
    paused: bool,
    active_layers: HashSet<String>,
}

impl KeymapEngine {
    pub fn new(profile: Profile) -> Self {
        let sensitivity = profile.global_sensitivity;
        let states: Vec<NodeState> = profile.nodes.iter().map(Self::init_state).collect();
        let joystick_bindings: Vec<Option<JoystickBinding>> = profile
            .nodes
            .iter()
            .map(Self::build_joystick_binding)
            .collect();

        let mut key_bindings: HashMap<Key, Vec<usize>> = HashMap::new();
        for (idx, node) in profile.nodes.iter().enumerate() {
            for key_str in node.bound_keys() {
                if let Ok(key) = key_str.parse::<Key>() {
                    key_bindings.entry(key).or_default().push(idx);
                } else {
                    tracing::warn!("unknown key '{}' in node '{}'", key_str, node.id());
                }
            }
        }

        Self {
            sensitivity,
            profile,
            key_bindings,
            joystick_bindings,
            states,
            paused: false,
            active_layers: HashSet::new(),
        }
    }

    fn mouse_camera_state(region: &Region, enabled: bool) -> NodeState {
        // Mouse-look has two separate concepts:
        // 1. whether the mode is enabled at all
        // 2. whether a synthetic finger is currently down
        //
        // Keeping them separate is what makes `while_held` and `toggle` behave
        // correctly without conflating "mode active" with "finger still moving".
        NodeState::MouseCamera {
            enabled,
            finger_active: false,
            current_x: region.x + region.w / 2.0,
            current_y: region.y + region.h / 2.0,
            last_motion: Instant::now(),
        }
    }

    fn set_mouse_camera_enabled(&mut self, idx: usize, enabled: bool) -> Vec<TouchCommand> {
        let Node::MouseCamera { slot, region, .. } = &self.profile.nodes[idx] else {
            return Vec::new();
        };

        let (was_enabled, finger_active) = match &self.states[idx] {
            NodeState::MouseCamera {
                enabled,
                finger_active,
                ..
            } => (*enabled, *finger_active),
            _ => return Vec::new(),
        };

        if was_enabled == enabled {
            return Vec::new();
        }

        let mut cmds = Vec::new();
        // Turning the mode off must explicitly lift the synthetic finger so
        // the game never sees a stuck look/drag touch after a mode change.
        if !enabled && finger_active {
            cmds.push(TouchCommand::TouchUp { slot: *slot });
        }

        self.states[idx] = Self::mouse_camera_state(region, enabled);
        cmds
    }

    fn build_joystick_binding(node: &Node) -> Option<JoystickBinding> {
        let Node::Joystick { id, keys, .. } = node else {
            return None;
        };

        let parse = |name: &str| name.parse::<Key>().ok();
        match (
            parse(&keys.up),
            parse(&keys.down),
            parse(&keys.left),
            parse(&keys.right),
        ) {
            (Some(up), Some(down), Some(left), Some(right)) => Some(JoystickBinding {
                up,
                down,
                left,
                right,
            }),
            _ => {
                tracing::warn!("invalid joystick binding in node '{}'", id);
                None
            }
        }
    }

    fn init_state(node: &Node) -> NodeState {
        match node {
            Node::Tap { .. } => NodeState::Tap { active: false },
            Node::HoldTap { .. } => NodeState::HoldTap { held: false },
            Node::ToggleTap { .. } => NodeState::ToggleTap { active: false },
            Node::Joystick { .. } => NodeState::Joystick {
                up: false,
                down: false,
                left: false,
                right: false,
                finger_active: false,
                origin_x: 0.0,
                origin_y: 0.0,
            },
            Node::Drag { .. } => NodeState::Drag {
                running: false,
                started_at: Instant::now(),
                last_progress: 0.0,
            },
            Node::MouseCamera {
                region,
                activation_mode,
                ..
            } => Self::mouse_camera_state(
                region,
                matches!(activation_mode, MouseCameraActivationMode::AlwaysOn),
            ),
            Node::RepeatTap { .. } => NodeState::RepeatTap {
                active: false,
                last_toggle: Instant::now(),
                finger_down: false,
            },
            Node::Macro { .. } => NodeState::Macro {
                running: false,
                step_index: 0,
                step_start: Instant::now(),
                active_slots: Vec::new(),
            },
            Node::LayerShift { .. } => NodeState::LayerShift { held: false },
        }
    }

    pub fn set_sensitivity(&mut self, s: f64) {
        self.sensitivity = s;
    }

    pub fn pause(&mut self) -> Vec<TouchCommand> {
        self.paused = true;
        self.release_all()
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn profile_name(&self) -> &str {
        &self.profile.name
    }

    pub fn node_count(&self) -> usize {
        self.profile.nodes.len()
    }

    pub fn slots(&self) -> Vec<u8> {
        let mut slots: Vec<u8> = self.profile.nodes.iter().filter_map(Node::slot).collect();
        slots.sort_unstable();
        slots
    }

    pub fn active_layers(&self) -> impl Iterator<Item = &str> {
        self.active_layers.iter().map(String::as_str)
    }

    pub fn profile_clone(&self) -> Profile {
        self.profile.clone()
    }

    pub fn has_mouse_camera(&self) -> bool {
        self.profile
            .nodes
            .iter()
            .any(|node| matches!(node, Node::MouseCamera { .. }))
    }

    pub fn process(&mut self, event: &InputEvent) -> Vec<TouchCommand> {
        if self.paused {
            return vec![];
        }
        let cmds = match event {
            InputEvent::KeyPress(key) => self.handle_key_press(*key),
            InputEvent::KeyRelease(key) => self.handle_key_release(*key),
            InputEvent::MouseMove { dx, dy } => self.handle_mouse_move(*dx, *dy),
        };
        tracing::trace!(
            event = ?event,
            commands = ?cmds,
            paused = self.paused,
            active_layers = ?self.active_layers,
            "engine processed input"
        );
        cmds
    }

    pub fn tick(&mut self) -> Vec<TouchCommand> {
        if self.paused {
            return vec![];
        }

        let mut cmds = Vec::new();
        let now = Instant::now();

        for idx in 0..self.profile.nodes.len() {
            let node = &self.profile.nodes[idx];
            if !self.is_node_active(node) {
                continue;
            }

            if let Node::MouseCamera { slot, .. } = node {
                if let NodeState::MouseCamera {
                    enabled,
                    finger_active,
                    last_motion,
                    ..
                } = &self.states[idx]
                {
                    if *enabled
                        && *finger_active
                        && now.duration_since(*last_motion) >= MOUSE_LOOK_IDLE_TIMEOUT
                    {
                        cmds.push(TouchCommand::TouchUp { slot: *slot });
                        let region = match node {
                            Node::MouseCamera { region, .. } => region,
                            _ => unreachable!(),
                        };
                        self.states[idx] = Self::mouse_camera_state(region, *enabled);
                    }
                }
            }

            if let Node::RepeatTap { interval_ms, .. } = node {
                let state = &self.states[idx];
                if let NodeState::RepeatTap {
                    active,
                    last_toggle,
                    finger_down,
                } = state
                {
                    if *active {
                        let interval = Duration::from_millis(*interval_ms);
                        let target = if *finger_down { interval } else { interval / 2 };
                        if now.duration_since(*last_toggle) >= target {
                            let was_down = *finger_down;
                            if let Some(slot) = node.slot() {
                                let pos = match node {
                                    Node::RepeatTap { pos, .. } => *pos,
                                    _ => unreachable!(),
                                };
                                self.states[idx] = NodeState::RepeatTap {
                                    active: true,
                                    last_toggle: now,
                                    finger_down: !was_down,
                                };
                                if was_down {
                                    cmds.push(TouchCommand::TouchUp { slot });
                                } else {
                                    cmds.push(TouchCommand::TouchDown {
                                        slot,
                                        x: pos.x,
                                        y: pos.y,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            if let Node::Drag {
                slot,
                start,
                end,
                duration_ms,
                ..
            } = node
            {
                if let NodeState::Drag {
                    running,
                    started_at,
                    last_progress,
                } = &self.states[idx]
                {
                    if *running {
                        let duration = Duration::from_millis(*duration_ms);
                        let progress = if duration.is_zero() {
                            1.0
                        } else {
                            (now.duration_since(*started_at).as_secs_f64() / duration.as_secs_f64())
                                .clamp(0.0, 1.0)
                        };

                        if progress > *last_progress {
                            cmds.push(TouchCommand::TouchMove {
                                slot: *slot,
                                x: lerp(start.x, end.x, progress),
                                y: lerp(start.y, end.y, progress),
                            });
                        }

                        if progress >= 1.0 {
                            cmds.push(TouchCommand::TouchUp { slot: *slot });
                            self.states[idx] = NodeState::Drag {
                                running: false,
                                started_at: now,
                                last_progress: 0.0,
                            };
                        } else {
                            self.states[idx] = NodeState::Drag {
                                running: true,
                                started_at: *started_at,
                                last_progress: progress,
                            };
                        }
                    }
                }
            }

            if let Node::Macro { sequence, .. } = node {
                let state = &self.states[idx];
                if let NodeState::Macro {
                    running,
                    step_index,
                    step_start,
                    ..
                } = state
                {
                    if *running && *step_index < sequence.len() {
                        let step = &sequence[*step_index];
                        let delay = Duration::from_millis(step.delay_ms);
                        if now.duration_since(*step_start) >= delay {
                            let si = *step_index;
                            let step = step.clone();

                            match &step.action {
                                MacroAction::Down => {
                                    if let Some(pos) = &step.pos {
                                        cmds.push(TouchCommand::TouchDown {
                                            slot: step.slot,
                                            x: pos.x,
                                            y: pos.y,
                                        });
                                    }
                                }
                                MacroAction::Up => {
                                    cmds.push(TouchCommand::TouchUp { slot: step.slot });
                                }
                            }

                            let next_idx = si + 1;
                            if next_idx >= sequence.len() {
                                let mut slots_to_release = Vec::new();
                                for s in sequence {
                                    if !slots_to_release.contains(&s.slot) {
                                        slots_to_release.push(s.slot);
                                    }
                                }
                                for s in slots_to_release {
                                    cmds.push(TouchCommand::TouchUp { slot: s });
                                }
                                self.states[idx] = NodeState::Macro {
                                    running: false,
                                    step_index: 0,
                                    step_start: now,
                                    active_slots: Vec::new(),
                                };
                            } else {
                                let mut slots = Vec::new();
                                for s in sequence {
                                    if !slots.contains(&s.slot) {
                                        slots.push(s.slot);
                                    }
                                }
                                self.states[idx] = NodeState::Macro {
                                    running: true,
                                    step_index: next_idx,
                                    step_start: now,
                                    active_slots: slots,
                                };
                            }
                        }
                    }
                }
            }
        }

        if !cmds.is_empty() {
            tracing::trace!(
                commands = ?cmds,
                active_layers = ?self.active_layers,
                "engine tick produced commands"
            );
        }

        cmds
    }

    fn handle_key_press(&mut self, key: Key) -> Vec<TouchCommand> {
        let indices = match self.key_bindings.get(&key) {
            Some(v) => v.clone(),
            None => return vec![],
        };

        let mut cmds = Vec::new();
        for idx in indices {
            if let Node::LayerShift {
                layer_name, mode, ..
            } = &self.profile.nodes[idx]
            {
                let layer_name = layer_name.clone();
                let mode = mode.clone();
                cmds.extend(self.handle_layer_shift_press(idx, &layer_name, &mode));
            } else {
                let node = &self.profile.nodes[idx];
                if !self.is_node_active(node) {
                    continue;
                }
                cmds.extend(self.handle_action_press(idx, key));
            }
        }
        cmds
    }

    fn handle_key_release(&mut self, key: Key) -> Vec<TouchCommand> {
        let indices = match self.key_bindings.get(&key) {
            Some(v) => v.clone(),
            None => return vec![],
        };

        let mut cmds = Vec::new();
        for idx in indices {
            if let Node::LayerShift {
                layer_name, mode, ..
            } = &self.profile.nodes[idx]
            {
                let layer_name = layer_name.clone();
                let mode = mode.clone();
                cmds.extend(self.handle_layer_shift_release(idx, &layer_name, &mode));
            } else {
                let node = &self.profile.nodes[idx];
                if !self.is_node_active(node) {
                    continue;
                }
                cmds.extend(self.handle_action_release(idx, key));
            }
        }
        cmds
    }

    fn handle_action_press(&mut self, idx: usize, key: Key) -> Vec<TouchCommand> {
        let node = &self.profile.nodes[idx];
        let mut cmds = Vec::new();
        match node {
            Node::Tap { slot, pos, .. } => {
                if let NodeState::Tap { active } = &self.states[idx] {
                    if !*active {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: pos.x,
                            y: pos.y,
                        });
                        self.states[idx] = NodeState::Tap { active: true };
                    }
                }
            }
            Node::HoldTap { slot, pos, .. } => {
                if let NodeState::HoldTap { held } = &self.states[idx] {
                    if !*held {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: pos.x,
                            y: pos.y,
                        });
                        self.states[idx] = NodeState::HoldTap { held: true };
                    }
                }
            }
            Node::ToggleTap { slot, pos, .. } => {
                if let NodeState::ToggleTap { active } = &self.states[idx] {
                    if *active {
                        cmds.push(TouchCommand::TouchUp { slot: *slot });
                    } else {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: pos.x,
                            y: pos.y,
                        });
                    }
                    self.states[idx] = NodeState::ToggleTap { active: !*active };
                }
            }
            Node::Joystick {
                slot,
                pos,
                radius,
                mode,
                region,
                ..
            } => {
                if let Some(d) = self.joystick_direction(idx, key) {
                    if let NodeState::Joystick {
                        up,
                        down,
                        left,
                        right,
                        finger_active,
                        origin_x,
                        origin_y,
                    } = &self.states[idx]
                    {
                        let mut u = *up;
                        let mut dn = *down;
                        let mut l = *left;
                        let mut r = *right;
                        let mut fa = *finger_active;
                        let mut ox = *origin_x;
                        let mut oy = *origin_y;
                        match d {
                            Dir::Up => u = true,
                            Dir::Down => dn = true,
                            Dir::Left => l = true,
                            Dir::Right => r = true,
                        }

                        if !fa {
                            let (dir_x, dir_y) = joystick_direction_vector(u, dn, l, r);
                            let (start_x, start_y) =
                                joystick_origin(mode, pos, region.as_ref(), *radius, dir_x, dir_y);
                            ox = start_x;
                            oy = start_y;
                            cmds.push(TouchCommand::TouchDown {
                                slot: *slot,
                                x: start_x,
                                y: start_y,
                            });
                            fa = true;
                        }

                        let (offset_x, offset_y) = joystick_offset(u, dn, l, r, *radius);
                        let (move_x, move_y) =
                            joystick_target(ox, oy, offset_x, offset_y, region.as_ref());

                        cmds.push(TouchCommand::TouchMove {
                            slot: *slot,
                            x: move_x,
                            y: move_y,
                        });

                        self.states[idx] = NodeState::Joystick {
                            up: u,
                            down: dn,
                            left: l,
                            right: r,
                            finger_active: fa,
                            origin_x: ox,
                            origin_y: oy,
                        };
                    }
                }
            }
            Node::RepeatTap { slot, pos, .. } => {
                if let NodeState::RepeatTap { active, .. } = &self.states[idx] {
                    if !*active {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: pos.x,
                            y: pos.y,
                        });
                        self.states[idx] = NodeState::RepeatTap {
                            active: true,
                            last_toggle: Instant::now(),
                            finger_down: true,
                        };
                    }
                }
            }
            Node::Drag { slot, start, .. } => {
                if let NodeState::Drag { running, .. } = &self.states[idx] {
                    if !*running {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: start.x,
                            y: start.y,
                        });
                        self.states[idx] = NodeState::Drag {
                            running: true,
                            started_at: Instant::now(),
                            last_progress: 0.0,
                        };
                    }
                }
            }
            Node::Macro { sequence, .. } => {
                if let NodeState::Macro { running, .. } = &self.states[idx] {
                    if !*running {
                        let mut slots = Vec::new();
                        for s in sequence {
                            if !slots.contains(&s.slot) {
                                slots.push(s.slot);
                            }
                        }
                        self.states[idx] = NodeState::Macro {
                            running: true,
                            step_index: 0,
                            step_start: Instant::now(),
                            active_slots: slots,
                        };
                    }
                }
            }
            Node::MouseCamera {
                activation_mode, ..
            } => match activation_mode {
                MouseCameraActivationMode::AlwaysOn => {}
                MouseCameraActivationMode::WhileHeld => {
                    cmds.extend(self.set_mouse_camera_enabled(idx, true));
                }
                MouseCameraActivationMode::Toggle => {
                    let enabled = match &self.states[idx] {
                        NodeState::MouseCamera { enabled, .. } => *enabled,
                        _ => false,
                    };
                    cmds.extend(self.set_mouse_camera_enabled(idx, !enabled));
                }
            },
            Node::LayerShift { .. } => {}
        }
        cmds
    }

    fn handle_action_release(&mut self, idx: usize, key: Key) -> Vec<TouchCommand> {
        let node = &self.profile.nodes[idx];
        let mut cmds = Vec::new();
        match node {
            Node::Tap { slot, .. } => {
                if let NodeState::Tap { active } = &self.states[idx] {
                    if *active {
                        cmds.push(TouchCommand::TouchUp { slot: *slot });
                        self.states[idx] = NodeState::Tap { active: false };
                    }
                }
            }
            Node::HoldTap { slot, .. } => {
                if let NodeState::HoldTap { held } = &self.states[idx] {
                    if *held {
                        cmds.push(TouchCommand::TouchUp { slot: *slot });
                        self.states[idx] = NodeState::HoldTap { held: false };
                    }
                }
            }
            Node::ToggleTap { .. } => {}
            Node::Joystick {
                slot,
                radius,
                region,
                ..
            } => {
                if let Some(d) = self.joystick_direction(idx, key) {
                    if let NodeState::Joystick {
                        up,
                        down,
                        left,
                        right,
                        finger_active,
                        origin_x,
                        origin_y,
                    } = &self.states[idx]
                    {
                        let mut u = *up;
                        let mut dn = *down;
                        let mut l = *left;
                        let mut r = *right;
                        let fa = *finger_active;
                        match d {
                            Dir::Up => u = false,
                            Dir::Down => dn = false,
                            Dir::Left => l = false,
                            Dir::Right => r = false,
                        }
                        if !u && !dn && !l && !r && fa {
                            cmds.push(TouchCommand::TouchUp { slot: *slot });
                            self.states[idx] = NodeState::Joystick {
                                up: false,
                                down: false,
                                left: false,
                                right: false,
                                finger_active: false,
                                origin_x: 0.0,
                                origin_y: 0.0,
                            };
                        } else if fa {
                            let (offset_x, offset_y) = joystick_offset(u, dn, l, r, *radius);
                            let (move_x, move_y) = joystick_target(
                                *origin_x,
                                *origin_y,
                                offset_x,
                                offset_y,
                                region.as_ref(),
                            );
                            cmds.push(TouchCommand::TouchMove {
                                slot: *slot,
                                x: move_x,
                                y: move_y,
                            });
                            self.states[idx] = NodeState::Joystick {
                                up: u,
                                down: dn,
                                left: l,
                                right: r,
                                finger_active: fa,
                                origin_x: *origin_x,
                                origin_y: *origin_y,
                            };
                        }
                    }
                }
            }
            Node::RepeatTap { slot, .. } => {
                if let NodeState::RepeatTap {
                    active,
                    finger_down,
                    ..
                } = &self.states[idx]
                {
                    if *active && *finger_down {
                        cmds.push(TouchCommand::TouchUp { slot: *slot });
                    }
                    self.states[idx] = NodeState::RepeatTap {
                        active: false,
                        last_toggle: Instant::now(),
                        finger_down: false,
                    };
                }
            }
            Node::Drag { .. } => {}
            Node::Macro { .. } => {
                if let NodeState::Macro { running, .. } = &self.states[idx] {
                    if *running {
                        let slots = match &self.states[idx] {
                            NodeState::Macro { active_slots, .. } => active_slots.clone(),
                            _ => vec![],
                        };
                        for s in &slots {
                            cmds.push(TouchCommand::TouchUp { slot: *s });
                        }
                        self.states[idx] = NodeState::Macro {
                            running: false,
                            step_index: 0,
                            step_start: Instant::now(),
                            active_slots: Vec::new(),
                        };
                    }
                }
            }
            Node::MouseCamera {
                activation_mode, ..
            } => {
                if matches!(activation_mode, MouseCameraActivationMode::WhileHeld) {
                    cmds.extend(self.set_mouse_camera_enabled(idx, false));
                }
            }
            Node::LayerShift { .. } => {}
        }
        cmds
    }

    fn handle_layer_shift_press(
        &mut self,
        idx: usize,
        layer_name: &str,
        mode: &LayerMode,
    ) -> Vec<TouchCommand> {
        match mode {
            LayerMode::Hold => {
                if let NodeState::LayerShift { held } = &self.states[idx] {
                    if !*held {
                        self.active_layers.insert(layer_name.to_string());
                        self.states[idx] = NodeState::LayerShift { held: true };
                    }
                }
                vec![]
            }
            LayerMode::Toggle => {
                let mut cmds = Vec::new();
                if self.active_layers.remove(layer_name) {
                    cmds.extend(self.release_layer(layer_name));
                } else {
                    self.active_layers.insert(layer_name.to_string());
                }
                cmds
            }
        }
    }

    fn handle_layer_shift_release(
        &mut self,
        idx: usize,
        layer_name: &str,
        mode: &LayerMode,
    ) -> Vec<TouchCommand> {
        match mode {
            LayerMode::Hold => {
                if let NodeState::LayerShift { held } = &self.states[idx] {
                    if *held {
                        let mut cmds = self.release_layer(layer_name);
                        self.active_layers.remove(layer_name);
                        self.states[idx] = NodeState::LayerShift { held: false };
                        return std::mem::take(&mut cmds);
                    }
                }
                vec![]
            }
            LayerMode::Toggle => vec![],
        }
    }

    fn handle_mouse_move(&mut self, dx: i32, dy: i32) -> Vec<TouchCommand> {
        let mut cmds = Vec::new();

        for idx in 0..self.profile.nodes.len() {
            let node = &self.profile.nodes[idx];
            if !self.is_node_active(node) {
                continue;
            }

            if let Node::MouseCamera {
                slot,
                region,
                sensitivity,
                invert_y,
                ..
            } = node
            {
                if let NodeState::MouseCamera {
                    enabled,
                    finger_active,
                    current_x,
                    current_y,
                    ..
                } = &self.states[idx]
                {
                    if !*enabled {
                        continue;
                    }

                    let delta_x = dx as f64 * sensitivity * self.sensitivity;
                    let delta_y = if *invert_y { -(dy as f64) } else { dy as f64 }
                        * sensitivity
                        * self.sensitivity;

                    let scale = 1.0 / 500.0;
                    let fa = *finger_active;
                    let start_x = if fa {
                        *current_x
                    } else {
                        region.x + region.w / 2.0
                    };
                    let start_y = if fa {
                        *current_y
                    } else {
                        region.y + region.h / 2.0
                    };
                    let mut cx = start_x;
                    let mut cy = start_y;

                    cx = (cx + delta_x * scale).clamp(region.x, region.x + region.w);
                    cy = (cy + delta_y * scale).clamp(region.y, region.y + region.h);

                    if !fa {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: start_x,
                            y: start_y,
                        });
                    }

                    cmds.push(TouchCommand::TouchMove {
                        slot: *slot,
                        x: cx,
                        y: cy,
                    });
                    self.states[idx] = NodeState::MouseCamera {
                        enabled: *enabled,
                        finger_active: true,
                        current_x: cx,
                        current_y: cy,
                        last_motion: Instant::now(),
                    };
                }
            }
        }
        cmds
    }

    pub fn release_all(&mut self) -> Vec<TouchCommand> {
        let mut cmds = Vec::new();
        for idx in 0..self.profile.nodes.len() {
            cmds.extend(self.release_node(idx));
        }
        self.active_layers.clear();
        cmds
    }

    pub fn release_mouse_inputs(&mut self) -> Vec<TouchCommand> {
        let mut cmds = Vec::new();
        for idx in 0..self.profile.nodes.len() {
            if self.node_uses_mouse_input(&self.profile.nodes[idx]) {
                cmds.extend(self.release_node(idx));
            }
        }
        cmds
    }

    fn release_layer(&mut self, layer_name: &str) -> Vec<TouchCommand> {
        let mut cmds = Vec::new();
        for idx in 0..self.profile.nodes.len() {
            if self.profile.nodes[idx].layer() == layer_name {
                cmds.extend(self.release_node(idx));
            }
        }
        cmds
    }

    fn release_node(&mut self, idx: usize) -> Vec<TouchCommand> {
        let mut cmds = Vec::new();
        let node = &self.profile.nodes[idx];
        let slot = node.slot();
        match &self.states[idx] {
            NodeState::Tap { active: true } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::Tap { active: false };
            }
            NodeState::HoldTap { held: true } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::HoldTap { held: false };
            }
            NodeState::ToggleTap { active: true } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::ToggleTap { active: false };
            }
            NodeState::Joystick {
                finger_active: true,
                ..
            } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::Joystick {
                    up: false,
                    down: false,
                    left: false,
                    right: false,
                    finger_active: false,
                    origin_x: 0.0,
                    origin_y: 0.0,
                };
            }
            NodeState::Drag { running: true, .. } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::Drag {
                    running: false,
                    started_at: Instant::now(),
                    last_progress: 0.0,
                };
            }
            NodeState::MouseCamera { finger_active, .. } => {
                if let Some(s) = slot {
                    if *finger_active {
                        cmds.push(TouchCommand::TouchUp { slot: s });
                    }
                }
                self.states[idx] = Self::init_state(node);
            }
            NodeState::RepeatTap {
                active: true,
                finger_down: true,
                ..
            } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::RepeatTap {
                    active: false,
                    last_toggle: Instant::now(),
                    finger_down: false,
                };
            }
            NodeState::RepeatTap { .. } => {
                self.states[idx] = NodeState::RepeatTap {
                    active: false,
                    last_toggle: Instant::now(),
                    finger_down: false,
                };
            }
            NodeState::Macro {
                running: true,
                active_slots,
                ..
            } => {
                for &s in active_slots {
                    cmds.push(TouchCommand::TouchUp { slot: s });
                }
                self.states[idx] = NodeState::Macro {
                    running: false,
                    step_index: 0,
                    step_start: Instant::now(),
                    active_slots: Vec::new(),
                };
            }
            NodeState::LayerShift { held: true } => {
                self.states[idx] = NodeState::LayerShift { held: false };
            }
            _ => {}
        }
        cmds
    }

    fn is_node_active(&self, node: &Node) -> bool {
        let layer = node.layer().trim();
        layer.is_empty() || self.active_layers.contains(layer)
    }

    fn node_uses_mouse_input(&self, node: &Node) -> bool {
        if matches!(node, Node::MouseCamera { .. }) {
            return true;
        }

        node.bound_keys()
            .into_iter()
            .filter_map(|name| name.parse::<Key>().ok())
            .any(Key::is_mouse)
    }

    fn joystick_direction(&self, idx: usize, key: Key) -> Option<Dir> {
        let binding = self
            .joystick_bindings
            .get(idx)
            .and_then(|binding| binding.as_ref())?;
        if binding.up == key {
            Some(Dir::Up)
        } else if binding.down == key {
            Some(Dir::Down)
        } else if binding.left == key {
            Some(Dir::Left)
        } else if binding.right == key {
            Some(Dir::Right)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct JoystickBinding {
    up: Key,
    down: Key,
    left: Key,
    right: Key,
}

#[derive(Debug, Clone, Copy)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

fn joystick_offset(up: bool, down: bool, left: bool, right: bool, radius: f64) -> (f64, f64) {
    let (dir_x, dir_y) = joystick_direction_vector(up, down, left, right);
    (dir_x * radius, dir_y * radius)
}

fn joystick_direction_vector(up: bool, down: bool, left: bool, right: bool) -> (f64, f64) {
    let mut dx = 0.0;
    let mut dy = 0.0;
    if up {
        dy -= 1.0;
    }
    if down {
        dy += 1.0;
    }
    if left {
        dx -= 1.0;
    }
    if right {
        dx += 1.0;
    }
    if up && down {
        dy = 0.0;
    }
    if left && right {
        dx = 0.0;
    }
    if dx != 0.0 && dy != 0.0 {
        let diagonal = std::f64::consts::FRAC_1_SQRT_2;
        dx *= diagonal;
        dy *= diagonal;
    }
    (dx, dy)
}

fn joystick_origin(
    mode: &JoystickMode,
    pos: &crate::profile::RelPos,
    region: Option<&Region>,
    radius: f64,
    dir_x: f64,
    dir_y: f64,
) -> (f64, f64) {
    match mode {
        JoystickMode::Fixed => (pos.x, pos.y),
        JoystickMode::Floating => {
            let region = region.expect("floating joystick requires region");
            floating_joystick_origin(region, radius, dir_x, dir_y)
        }
    }
}

fn joystick_target(
    origin_x: f64,
    origin_y: f64,
    offset_x: f64,
    offset_y: f64,
    region: Option<&Region>,
) -> (f64, f64) {
    let mut x = origin_x + offset_x;
    let mut y = origin_y + offset_y;
    if let Some(region) = region {
        x = x.clamp(region.x, region.x + region.w);
        y = y.clamp(region.y, region.y + region.h);
    }
    (x, y)
}

fn floating_joystick_origin(region: &Region, radius: f64, dir_x: f64, dir_y: f64) -> (f64, f64) {
    // Floating sticks and football-style drag zones need a runtime origin:
    // the synthetic finger should start inside the allowed zone, then keep
    // that origin stable until all bound directions are released.
    let center_x = region.x + region.w / 2.0;
    let center_y = region.y + region.h / 2.0;
    let desired_x = center_x - dir_x * radius * 0.5;
    let desired_y = center_y - dir_y * radius * 0.5;

    let margin_x = radius.min(region.w / 2.0);
    let margin_y = radius.min(region.h / 2.0);
    let min_x = region.x + margin_x;
    let max_x = region.x + region.w - margin_x;
    let min_y = region.y + margin_y;
    let max_y = region.y + region.h - margin_y;

    let origin_x = if min_x <= max_x {
        desired_x.clamp(min_x, max_x)
    } else {
        center_x
    };
    let origin_y = if min_y <= max_y {
        desired_y.clamp(min_y, max_y)
    } else {
        center_y
    };

    (origin_x, origin_y)
}

fn lerp(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::*;

    fn screen() -> Option<ScreenOverride> {
        Some(ScreenOverride {
            width: 1920,
            height: 1080,
        })
    }

    fn test_profile() -> Profile {
        Profile {
            name: "Test".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![
                Node::Tap {
                    id: "jump".into(),
                    layer: String::new(),
                    slot: 0,
                    pos: RelPos { x: 0.5, y: 0.5 },
                    key: "Space".into(),
                },
                Node::Joystick {
                    id: "move".into(),
                    layer: String::new(),
                    slot: 1,
                    pos: RelPos { x: 0.2, y: 0.7 },
                    radius: 0.07,
                    mode: JoystickMode::Fixed,
                    region: None,
                    keys: JoystickKeys {
                        up: "W".into(),
                        down: "S".into(),
                        left: "A".into(),
                        right: "D".into(),
                    },
                },
            ],
        }
    }

    #[test]
    fn tap_down_up() {
        let mut engine = KeymapEngine::new(test_profile());
        let cmds = engine.process(&InputEvent::KeyPress(Key::Space));
        assert_eq!(cmds.len(), 1);
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 0, .. }));
        let cmds = engine.process(&InputEvent::KeyRelease(Key::Space));
        assert_eq!(cmds.len(), 1);
        assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 0 }));
    }

    #[test]
    fn joystick_diagonal_is_normalized() {
        let mut engine = KeymapEngine::new(test_profile());
        let _ = engine.process(&InputEvent::KeyPress(Key::W));
        let cmds = engine.process(&InputEvent::KeyPress(Key::D));
        match &cmds[0] {
            TouchCommand::TouchMove { x, y, .. } => {
                assert!(*x > 0.2);
                assert!(*y < 0.7);
            }
            other => panic!("expected move, got {other:?}"),
        }
    }

    #[test]
    fn floating_joystick_starts_inside_zone_and_keeps_origin() {
        let profile = Profile {
            name: "Float".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![Node::Joystick {
                id: "move".into(),
                layer: String::new(),
                slot: 1,
                pos: RelPos { x: 0.2, y: 0.7 },
                radius: 0.08,
                mode: JoystickMode::Floating,
                region: Some(Region {
                    x: 0.0,
                    y: 0.4,
                    w: 0.45,
                    h: 0.45,
                }),
                keys: JoystickKeys {
                    up: "W".into(),
                    down: "S".into(),
                    left: "A".into(),
                    right: "D".into(),
                },
            }],
        };

        let mut engine = KeymapEngine::new(profile);
        let cmds = engine.process(&InputEvent::KeyPress(Key::D));
        let [TouchCommand::TouchDown {
            x: down_x,
            y: down_y,
            ..
        }, TouchCommand::TouchMove {
            x: move_x,
            y: move_y,
            ..
        }] = cmds.as_slice()
        else {
            panic!("expected down+move, got {cmds:?}");
        };
        assert!((*down_x >= 0.0) && (*down_x <= 0.45));
        assert!((*down_y >= 0.4) && (*down_y <= 0.85));
        assert!(*move_x >= *down_x);

        let cmds = engine.process(&InputEvent::KeyPress(Key::W));
        let [TouchCommand::TouchMove {
            x: next_x,
            y: next_y,
            ..
        }] = cmds.as_slice()
        else {
            panic!("expected single move, got {cmds:?}");
        };
        assert!((*next_x >= 0.0) && (*next_x <= 0.45));
        assert!((*next_y >= 0.4) && (*next_y <= 0.85));
        assert!(*next_y <= *move_y);
    }

    #[test]
    fn drag_gesture_runs_to_completion() {
        let profile = Profile {
            name: "Drag".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![Node::Drag {
                id: "lane_left".into(),
                layer: String::new(),
                slot: 2,
                start: RelPos { x: 0.5, y: 0.7 },
                end: RelPos { x: 0.2, y: 0.7 },
                key: "A".into(),
                duration_ms: 1,
            }],
        };

        let mut engine = KeymapEngine::new(profile);
        let cmds = engine.process(&InputEvent::KeyPress(Key::A));
        assert_eq!(cmds.len(), 1);
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 2, .. }));

        std::thread::sleep(Duration::from_millis(2));
        let cmds = engine.tick();
        assert!(matches!(&cmds[0], TouchCommand::TouchMove { slot: 2, .. }));
        assert!(matches!(&cmds[1], TouchCommand::TouchUp { slot: 2 }));
    }

    #[test]
    fn mouse_camera_starts_at_center_and_moves() {
        let profile = Profile {
            name: "Cam".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![Node::MouseCamera {
                id: "look".into(),
                layer: String::new(),
                slot: 1,
                region: Region {
                    x: 0.3,
                    y: 0.0,
                    w: 0.7,
                    h: 1.0,
                },
                sensitivity: 1.0,
                activation_mode: MouseCameraActivationMode::AlwaysOn,
                activation_key: None,
                invert_y: false,
            }],
        };
        let mut engine = KeymapEngine::new(profile);
        let cmds = engine.process(&InputEvent::MouseMove { dx: 10, dy: 5 });
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
        assert!(matches!(&cmds[1], TouchCommand::TouchMove { slot: 1, .. }));
    }

    #[test]
    fn mouse_camera_while_held_requires_activation_key() {
        let profile = Profile {
            name: "Cam Hold".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![Node::MouseCamera {
                id: "look".into(),
                layer: String::new(),
                slot: 1,
                region: Region {
                    x: 0.3,
                    y: 0.0,
                    w: 0.7,
                    h: 1.0,
                },
                sensitivity: 1.0,
                activation_mode: MouseCameraActivationMode::WhileHeld,
                activation_key: Some("MouseRight".into()),
                invert_y: false,
            }],
        };
        let mut engine = KeymapEngine::new(profile);
        assert!(engine
            .process(&InputEvent::MouseMove { dx: 10, dy: 5 })
            .is_empty());
        assert!(engine
            .process(&InputEvent::KeyPress(Key::MouseRight))
            .is_empty());
        let cmds = engine.process(&InputEvent::MouseMove { dx: 10, dy: 5 });
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
        assert!(matches!(&cmds[1], TouchCommand::TouchMove { slot: 1, .. }));
        let cmds = engine.process(&InputEvent::KeyRelease(Key::MouseRight));
        assert!(matches!(
            cmds.as_slice(),
            [TouchCommand::TouchUp { slot: 1 }]
        ));
    }

    #[test]
    fn mouse_camera_toggle_toggles_on_and_off() {
        let profile = Profile {
            name: "Cam Toggle".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![Node::MouseCamera {
                id: "look".into(),
                layer: String::new(),
                slot: 1,
                region: Region {
                    x: 0.3,
                    y: 0.0,
                    w: 0.7,
                    h: 1.0,
                },
                sensitivity: 1.0,
                activation_mode: MouseCameraActivationMode::Toggle,
                activation_key: Some("MouseRight".into()),
                invert_y: false,
            }],
        };
        let mut engine = KeymapEngine::new(profile);
        assert!(engine
            .process(&InputEvent::MouseMove { dx: 10, dy: 5 })
            .is_empty());
        assert!(engine
            .process(&InputEvent::KeyPress(Key::MouseRight))
            .is_empty());
        let cmds = engine.process(&InputEvent::MouseMove { dx: 10, dy: 5 });
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
        assert!(matches!(&cmds[1], TouchCommand::TouchMove { slot: 1, .. }));
        assert!(engine
            .process(&InputEvent::KeyRelease(Key::MouseRight))
            .is_empty());
        let cmds = engine.process(&InputEvent::KeyPress(Key::MouseRight));
        assert!(matches!(
            cmds.as_slice(),
            [TouchCommand::TouchUp { slot: 1 }]
        ));
        assert!(engine
            .process(&InputEvent::MouseMove { dx: 10, dy: 5 })
            .is_empty());
    }

    #[test]
    fn toggle_tap_toggles_on_press_only() {
        let profile = Profile {
            name: "Toggle".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![Node::ToggleTap {
                id: "scope".into(),
                layer: String::new(),
                slot: 0,
                pos: RelPos { x: 0.8, y: 0.4 },
                key: "Q".into(),
            }],
        };
        let mut engine = KeymapEngine::new(profile);
        let cmds = engine.process(&InputEvent::KeyPress(Key::Q));
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 0, .. }));
        assert!(engine.process(&InputEvent::KeyRelease(Key::Q)).is_empty());
        let cmds = engine.process(&InputEvent::KeyPress(Key::Q));
        assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 0 }));
    }

    #[test]
    fn layer_shift_activates_alternate_binding() {
        let profile = Profile {
            name: "Layers".into(),
            version: 1,
            screen: screen(),
            global_sensitivity: 1.0,
            nodes: vec![
                Node::Tap {
                    id: "jump".into(),
                    layer: String::new(),
                    slot: 0,
                    pos: RelPos { x: 0.5, y: 0.5 },
                    key: "Space".into(),
                },
                Node::Tap {
                    id: "alt_jump".into(),
                    layer: "combat".into(),
                    slot: 1,
                    pos: RelPos { x: 0.7, y: 0.7 },
                    key: "E".into(),
                },
                Node::LayerShift {
                    id: "combat_layer".into(),
                    key: "LeftAlt".into(),
                    layer_name: "combat".into(),
                    mode: LayerMode::Hold,
                },
            ],
        };
        let mut engine = KeymapEngine::new(profile);
        assert!(engine.process(&InputEvent::KeyPress(Key::E)).is_empty());
        assert!(engine
            .process(&InputEvent::KeyPress(Key::LeftAlt))
            .is_empty());
        let cmds = engine.process(&InputEvent::KeyPress(Key::E));
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
        let cmds = engine.process(&InputEvent::KeyRelease(Key::LeftAlt));
        assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 1 }));
    }
}

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::error::Result;
use crate::inject::UinputDevice;
use crate::input::{InputEvent, Key};
use crate::profile::{LayerMode, MacroAction, Node, Profile};

#[derive(Debug, Clone)]
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
    },
    MouseCamera {
        finger_active: bool,
        current_x: f64,
        current_y: f64,
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
    states: Vec<NodeState>,
    sensitivity: f64,
    paused: bool,
    active_layers: HashSet<String>,
}

impl KeymapEngine {
    pub fn new(profile: Profile) -> Self {
        let sensitivity = profile.global_sensitivity;
        let states: Vec<NodeState> = profile.nodes.iter().map(Self::init_state).collect();

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
            states,
            paused: false,
            active_layers: HashSet::new(),
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
            },
            Node::MouseCamera { region, .. } => NodeState::MouseCamera {
                finger_active: false,
                current_x: region.x + region.w / 2.0,
                current_y: region.y + region.h / 2.0,
            },
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

    pub fn active_layers(&self) -> impl Iterator<Item = &str> {
        self.active_layers.iter().map(String::as_str)
    }

    pub fn process(&mut self, event: &InputEvent) -> Vec<TouchCommand> {
        if self.paused {
            return vec![];
        }
        match event {
            InputEvent::KeyPress(key) => self.handle_key_press(*key),
            InputEvent::KeyRelease(key) => self.handle_key_release(*key),
            InputEvent::MouseMove { dx, dy } => self.handle_mouse_move(*dx, *dy),
        }
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
                                    Node::RepeatTap { pos, .. } => pos.clone(),
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
                keys,
                ..
            } => {
                if let Some(d) = Self::joystick_direction(key, keys) {
                    if let NodeState::Joystick {
                        up,
                        down,
                        left,
                        right,
                        finger_active,
                    } = &self.states[idx]
                    {
                        let mut u = *up;
                        let mut dn = *down;
                        let mut l = *left;
                        let mut r = *right;
                        let mut fa = *finger_active;
                        match d {
                            Dir::Up => u = true,
                            Dir::Down => dn = true,
                            Dir::Left => l = true,
                            Dir::Right => r = true,
                        }
                        let (ox, oy) = joystick_offset(u, dn, l, r, *radius);
                        if !fa {
                            cmds.push(TouchCommand::TouchDown {
                                slot: *slot,
                                x: pos.x,
                                y: pos.y,
                            });
                            fa = true;
                        }
                        cmds.push(TouchCommand::TouchMove {
                            slot: *slot,
                            x: pos.x + ox,
                            y: pos.y + oy,
                        });
                        self.states[idx] = NodeState::Joystick {
                            up: u,
                            down: dn,
                            left: l,
                            right: r,
                            finger_active: fa,
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
            Node::MouseCamera { .. } | Node::LayerShift { .. } => {}
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
                pos,
                radius,
                keys,
                ..
            } => {
                if let Some(d) = Self::joystick_direction(key, keys) {
                    if let NodeState::Joystick {
                        up,
                        down,
                        left,
                        right,
                        finger_active,
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
                            };
                        } else if fa {
                            let (ox, oy) = joystick_offset(u, dn, l, r, *radius);
                            cmds.push(TouchCommand::TouchMove {
                                slot: *slot,
                                x: pos.x + ox,
                                y: pos.y + oy,
                            });
                            self.states[idx] = NodeState::Joystick {
                                up: u,
                                down: dn,
                                left: l,
                                right: r,
                                finger_active: fa,
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
            Node::MouseCamera { .. } | Node::LayerShift { .. } => {}
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
                    finger_active,
                    current_x,
                    current_y,
                } = &self.states[idx]
                {
                    let delta_x = dx as f64 * sensitivity * self.sensitivity;
                    let delta_y = if *invert_y { -(dy as f64) } else { dy as f64 }
                        * sensitivity
                        * self.sensitivity;

                    let scale = 1.0 / 500.0;
                    let fa = *finger_active;
                    let mut cx = if fa {
                        *current_x
                    } else {
                        region.x + region.w / 2.0
                    };
                    let mut cy = if fa {
                        *current_y
                    } else {
                        region.y + region.h / 2.0
                    };

                    cx = (cx + delta_x * scale).clamp(region.x, region.x + region.w);
                    cy = (cy + delta_y * scale).clamp(region.y, region.y + region.h);

                    if !fa {
                        cmds.push(TouchCommand::TouchDown {
                            slot: *slot,
                            x: cx,
                            y: cy,
                        });
                    }

                    cmds.push(TouchCommand::TouchMove {
                        slot: *slot,
                        x: cx,
                        y: cy,
                    });
                    self.states[idx] = NodeState::MouseCamera {
                        finger_active: true,
                        current_x: cx,
                        current_y: cy,
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
                };
            }
            NodeState::MouseCamera {
                finger_active: true,
                ..
            } => {
                if let Some(s) = slot {
                    cmds.push(TouchCommand::TouchUp { slot: s });
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

    fn joystick_direction(key: Key, keys: &crate::profile::JoystickKeys) -> Option<Dir> {
        if keys.up.parse::<Key>().ok() == Some(key) {
            Some(Dir::Up)
        } else if keys.down.parse::<Key>().ok() == Some(key) {
            Some(Dir::Down)
        } else if keys.left.parse::<Key>().ok() == Some(key) {
            Some(Dir::Left)
        } else if keys.right.parse::<Key>().ok() == Some(key) {
            Some(Dir::Right)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

fn joystick_offset(up: bool, down: bool, left: bool, right: bool, radius: f64) -> (f64, f64) {
    let mut dx = 0.0;
    let mut dy = 0.0;
    if up {
        dy -= radius;
    }
    if down {
        dy += radius;
    }
    if left {
        dx -= radius;
    }
    if right {
        dx += radius;
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

pub fn execute_commands(device: &mut UinputDevice, cmds: &[TouchCommand]) -> Result<()> {
    for cmd in cmds {
        match cmd {
            TouchCommand::TouchDown { slot, x, y } => device.touch_down(*slot, *x, *y)?,
            TouchCommand::TouchMove { slot, x, y } => device.touch_move(*slot, *x, *y)?,
            TouchCommand::TouchUp { slot } => device.touch_up(*slot)?,
        }
    }
    Ok(())
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
                invert_y: false,
            }],
        };
        let mut engine = KeymapEngine::new(profile);
        let cmds = engine.process(&InputEvent::MouseMove { dx: 10, dy: 5 });
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
        assert!(matches!(&cmds[1], TouchCommand::TouchMove { slot: 1, .. }));
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

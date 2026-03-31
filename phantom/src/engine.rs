use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::Result;
use crate::inject::UinputDevice;
use crate::input::{InputEvent, Key};
use crate::profile::{MacroAction, Node, Profile};

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
}

pub struct KeymapEngine {
    profile: Profile,
    key_bindings: HashMap<Key, Vec<usize>>,
    states: Vec<NodeState>,
    sensitivity: f64,
    paused: bool,
}

impl KeymapEngine {
    pub fn new(profile: Profile) -> Self {
        let sensitivity = profile.global_sensitivity;
        let states: Vec<NodeState> = profile.nodes.iter().map(Self::init_state).collect();

        let mut key_bindings: HashMap<Key, Vec<usize>> = HashMap::new();
        for (idx, node) in profile.nodes.iter().enumerate() {
            for key_str in node.bound_keys() {
                if let Some(key) = Key::from_str(key_str) {
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
        }
    }

    fn init_state(node: &Node) -> NodeState {
        match node {
            Node::Tap { .. } => NodeState::Tap { active: false },
            Node::HoldTap { .. } => NodeState::HoldTap { held: false },
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

            // RepeatTap ticking
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
                            let slot = node.slot().unwrap();
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

            // Macro ticking
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
                                // Macro done — collect slots to release
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
            let node = &self.profile.nodes[idx];
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
                Node::Joystick {
                    slot,
                    pos,
                    radius,
                    keys,
                    ..
                } => {
                    let dir = Self::joystick_direction(key, keys);
                    if let Some(d) = dir {
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
                Node::MouseCamera { .. } => {}
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
            let node = &self.profile.nodes[idx];
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
                Node::Joystick {
                    slot,
                    pos,
                    radius,
                    keys,
                    ..
                } => {
                    let dir = Self::joystick_direction(key, keys);
                    if let Some(d) = dir {
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
                            // Collect slots and release
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
                Node::MouseCamera { .. } => {}
            }
        }
        cmds
    }

    fn handle_mouse_move(&mut self, dx: i32, dy: i32) -> Vec<TouchCommand> {
        let mut cmds = Vec::new();

        for idx in 0..self.profile.nodes.len() {
            if let Node::MouseCamera {
                slot,
                region,
                sensitivity,
                invert_y,
                ..
            } = &self.profile.nodes[idx]
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
                    let mut cx = *current_x;
                    let mut cy = *current_y;

                    let new_x = (cx + delta_x * scale).clamp(region.x, region.x + region.w);
                    let new_y = (cy + delta_y * scale).clamp(region.y, region.y + region.h);

                    cx = new_x;
                    cy = new_y;

                    let fa = *finger_active;
                    if !fa {
                        cx = region.x + region.w / 2.0;
                        cy = region.y + region.h / 2.0;
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
                _ => {}
            }
        }
        cmds
    }

    fn joystick_direction(key: Key, keys: &crate::profile::JoystickKeys) -> Option<Dir> {
        if Key::from_str(&keys.up) == Some(key) {
            Some(Dir::Up)
        } else if Key::from_str(&keys.down) == Some(key) {
            Some(Dir::Down)
        } else if Key::from_str(&keys.left) == Some(key) {
            Some(Dir::Left)
        } else if Key::from_str(&keys.right) == Some(key) {
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

    fn test_profile() -> Profile {
        Profile {
            name: "Test".into(),
            version: 1,
            screen: None,
            global_sensitivity: 1.0,
            nodes: vec![
                Node::Tap {
                    id: "jump".into(),
                    slot: 0,
                    pos: RelPos { x: 0.5, y: 0.5 },
                    key: "Space".into(),
                },
                Node::Joystick {
                    id: "move".into(),
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
    fn tap_ignores_repeat_press() {
        let mut engine = KeymapEngine::new(test_profile());
        engine.process(&InputEvent::KeyPress(Key::Space));
        let cmds = engine.process(&InputEvent::KeyPress(Key::Space));
        assert!(cmds.is_empty());
    }

    #[test]
    fn joystick_wasd() {
        let mut engine = KeymapEngine::new(test_profile());
        let cmds = engine.process(&InputEvent::KeyPress(Key::W));
        assert_eq!(cmds.len(), 2);
        assert!(matches!(&cmds[0], TouchCommand::TouchDown { slot: 1, .. }));
        let cmds = engine.process(&InputEvent::KeyPress(Key::D));
        assert_eq!(cmds.len(), 1);
        let cmds = engine.process(&InputEvent::KeyRelease(Key::W));
        assert_eq!(cmds.len(), 1);
        let cmds = engine.process(&InputEvent::KeyRelease(Key::D));
        assert_eq!(cmds.len(), 1);
        assert!(matches!(&cmds[0], TouchCommand::TouchUp { slot: 1 }));
    }

    #[test]
    fn release_all_active() {
        let mut engine = KeymapEngine::new(test_profile());
        engine.process(&InputEvent::KeyPress(Key::Space));
        engine.process(&InputEvent::KeyPress(Key::W));
        let cmds = engine.release_all();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn pause_blocks_input() {
        let mut engine = KeymapEngine::new(test_profile());
        engine.pause();
        let cmds = engine.process(&InputEvent::KeyPress(Key::Space));
        assert!(cmds.is_empty());
        engine.resume();
        let cmds = engine.process(&InputEvent::KeyPress(Key::Space));
        assert!(!cmds.is_empty());
    }

    #[test]
    fn unknown_key_ignored() {
        let mut engine = KeymapEngine::new(test_profile());
        let cmds = engine.process(&InputEvent::KeyPress(Key::F1));
        assert!(cmds.is_empty());
    }
}

use std::collections::HashSet;

use crate::engine::TouchCommand;
use crate::input::{InputEvent, Key};

pub const RUNTIME_MOUSE_TOUCH_SLOT: u8 = u8::MAX;

#[derive(Debug, Clone)]
pub struct MouseTouchEmulator {
    cursor_x: f64,
    cursor_y: f64,
    finger_down: bool,
    screen_width: u32,
    screen_height: u32,
}

impl MouseTouchEmulator {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            cursor_x: 0.5,
            cursor_y: 0.5,
            finger_down: false,
            screen_width,
            screen_height,
        }
    }

    pub fn is_active(&self) -> bool {
        self.finger_down
    }

    pub fn suspend(&mut self) -> Vec<TouchCommand> {
        if self.finger_down {
            self.finger_down = false;
            vec![TouchCommand::TouchUp {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
            }]
        } else {
            Vec::new()
        }
    }

    pub fn resync_buttons(&mut self, pressed: &HashSet<Key>) -> Vec<TouchCommand> {
        let should_hold = pressed.contains(&Key::MouseLeft);
        match (self.finger_down, should_hold) {
            (false, true) => self.handle_press(),
            (true, false) => self.handle_release(),
            _ => Vec::new(),
        }
    }

    pub fn process(&mut self, event: &InputEvent) -> Vec<TouchCommand> {
        match event {
            InputEvent::MouseMove { dx, dy } => self.handle_move(*dx, *dy),
            InputEvent::KeyPress(Key::MouseLeft) => self.handle_press(),
            InputEvent::KeyRelease(Key::MouseLeft) => self.handle_release(),
            _ => Vec::new(),
        }
    }

    fn handle_move(&mut self, dx: i32, dy: i32) -> Vec<TouchCommand> {
        let width = self.screen_width.max(1) as f64;
        let height = self.screen_height.max(1) as f64;
        self.cursor_x = (self.cursor_x + dx as f64 / width).clamp(0.0, 1.0);
        self.cursor_y = (self.cursor_y + dy as f64 / height).clamp(0.0, 1.0);

        if self.finger_down {
            vec![TouchCommand::TouchMove {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                x: self.cursor_x,
                y: self.cursor_y,
            }]
        } else {
            Vec::new()
        }
    }

    fn handle_press(&mut self) -> Vec<TouchCommand> {
        if self.finger_down {
            return Vec::new();
        }
        self.finger_down = true;
        vec![TouchCommand::TouchDown {
            slot: RUNTIME_MOUSE_TOUCH_SLOT,
            x: self.cursor_x,
            y: self.cursor_y,
        }]
    }

    fn handle_release(&mut self) -> Vec<TouchCommand> {
        if !self.finger_down {
            return Vec::new();
        }
        self.finger_down = false;
        vec![TouchCommand::TouchUp {
            slot: RUNTIME_MOUSE_TOUCH_SLOT,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_click_becomes_touch_down_and_up() {
        let mut emulator = MouseTouchEmulator::new(1920, 1080);

        let down = emulator.process(&InputEvent::KeyPress(Key::MouseLeft));
        assert!(matches!(
            down.as_slice(),
            [TouchCommand::TouchDown {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                ..
            }]
        ));

        let up = emulator.process(&InputEvent::KeyRelease(Key::MouseLeft));
        assert!(matches!(
            up.as_slice(),
            [TouchCommand::TouchUp {
                slot: RUNTIME_MOUSE_TOUCH_SLOT
            }]
        ));
    }

    #[test]
    fn drag_moves_active_touch() {
        let mut emulator = MouseTouchEmulator::new(100, 100);
        let _ = emulator.process(&InputEvent::KeyPress(Key::MouseLeft));
        let cmds = emulator.process(&InputEvent::MouseMove { dx: 10, dy: 5 });
        assert!(matches!(
            cmds.as_slice(),
            [TouchCommand::TouchMove {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                x,
                y
            }] if *x > 0.5 && *y > 0.5
        ));
    }

    #[test]
    fn suspend_releases_active_touch() {
        let mut emulator = MouseTouchEmulator::new(1920, 1080);
        let _ = emulator.process(&InputEvent::KeyPress(Key::MouseLeft));
        assert!(emulator.is_active());
        let cmds = emulator.suspend();
        assert!(!emulator.is_active());
        assert!(matches!(
            cmds.as_slice(),
            [TouchCommand::TouchUp {
                slot: RUNTIME_MOUSE_TOUCH_SLOT
            }]
        ));
    }

    #[test]
    fn resync_buttons_matches_real_left_button_state() {
        let mut emulator = MouseTouchEmulator::new(1920, 1080);
        let mut pressed = HashSet::new();
        pressed.insert(Key::MouseLeft);

        let down = emulator.resync_buttons(&pressed);
        assert!(matches!(
            down.as_slice(),
            [TouchCommand::TouchDown {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                ..
            }]
        ));

        pressed.clear();
        let up = emulator.resync_buttons(&pressed);
        assert!(matches!(
            up.as_slice(),
            [TouchCommand::TouchUp {
                slot: RUNTIME_MOUSE_TOUCH_SLOT
            }]
        ));
    }
}

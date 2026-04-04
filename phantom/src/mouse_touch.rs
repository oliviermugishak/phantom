use std::collections::HashSet;

use crate::engine::TouchCommand;
use crate::hyprland_cursor::HyprlandCursorClient;
use crate::input::{InputEvent, Key};
use crate::logging::trace_detail_enabled;
use crate::x11_cursor::X11CursorClient;

pub const RUNTIME_MOUSE_TOUCH_SLOT: u8 = u8::MAX;

#[derive(Debug)]
pub struct MouseTouchEmulator {
    cursor_x: f64,
    cursor_y: f64,
    finger_down: bool,
    screen_width: u32,
    screen_height: u32,
    hyprland: Option<HyprlandCursorClient>,
    exact_x11: Option<X11CursorClient>,
}

impl MouseTouchEmulator {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let hyprland = match HyprlandCursorClient::spawn() {
            Ok(client) => Some(client),
            Err(e) => {
                tracing::info!("hyprland cursor helper unavailable: {}", e);
                None
            }
        };
        let exact_x11 = match X11CursorClient::spawn() {
            Ok(client) => Some(client),
            Err(e) => {
                tracing::info!(
                    "x11 cursor helper unavailable, using virtual cursor fallback: {}",
                    e
                );
                None
            }
        };
        tracing::info!(
            backend = if hyprland.is_some() {
                "hyprland-client-absolute+x11-helper+virtual-fallback"
            } else if exact_x11.is_some() {
                "x11-helper+virtual-fallback"
            } else {
                "virtual-cursor"
            },
            "mouse-touch backend ready"
        );

        Self {
            cursor_x: 0.5,
            cursor_y: 0.5,
            finger_down: false,
            screen_width,
            screen_height,
            hyprland,
            exact_x11,
        }
    }

    pub fn is_active(&self) -> bool {
        self.finger_down
    }

    pub fn backend_name(&self) -> &'static str {
        if self.hyprland.is_some() {
            "hyprland-client-absolute+x11-helper+virtual-fallback"
        } else if self.exact_x11.is_some() {
            "x11-helper+virtual-fallback"
        } else {
            "virtual-cursor"
        }
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
            InputEvent::MouseMove { dx, dy, .. } => self.handle_move(*dx, *dy),
            InputEvent::KeyPress(Key::MouseLeft) => self.handle_press(),
            InputEvent::KeyRelease(Key::MouseLeft) => self.handle_release(),
            _ => Vec::new(),
        }
    }

    fn handle_move(&mut self, dx: i32, dy: i32) -> Vec<TouchCommand> {
        self.update_virtual_cursor(dx, dy);
        let Some((x, y)) = self.current_position() else {
            return Vec::new();
        };
        self.cursor_x = x;
        self.cursor_y = y;

        if self.finger_down {
            vec![TouchCommand::TouchMove {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                x,
                y,
            }]
        } else {
            Vec::new()
        }
    }

    fn handle_press(&mut self) -> Vec<TouchCommand> {
        if self.finger_down {
            return Vec::new();
        }
        let Some((x, y)) = self.current_position() else {
            return Vec::new();
        };
        self.finger_down = true;
        vec![TouchCommand::TouchDown {
            slot: RUNTIME_MOUSE_TOUCH_SLOT,
            x,
            y,
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

    fn update_virtual_cursor(&mut self, dx: i32, dy: i32) {
        let width = self.screen_width.max(1) as f64;
        let height = self.screen_height.max(1) as f64;
        self.cursor_x = (self.cursor_x + dx as f64 / width).clamp(0.0, 1.0);
        self.cursor_y = (self.cursor_y + dy as f64 / height).clamp(0.0, 1.0);
    }

    fn current_position(&mut self) -> Option<(f64, f64)> {
        if let Some(client) = self.hyprland.as_mut() {
            if let Some(position) = client.query_position() {
                return Some(position);
            }
            tracing::warn!(
                "hyprland cursor helper stopped responding, falling back to x11/virtual cursor"
            );
            self.hyprland = None;
            if trace_detail_enabled() {
                tracing::trace!(
                    "mouse-touch hyprland cursor helper unavailable, falling back to x11/virtual cursor"
                );
            }
        }
        if let Some(client) = self.exact_x11.as_mut() {
            if let Some(position) = client.query_position() {
                return Some(position);
            }
            tracing::warn!("x11 cursor helper stopped responding, falling back to virtual cursor");
            self.exact_x11 = None;
            if trace_detail_enabled() {
                tracing::trace!(
                    "mouse-touch exact cursor helper unavailable, falling back to virtual cursor"
                );
            }
        }
        Some((self.cursor_x, self.cursor_y))
    }

    #[cfg(test)]
    fn new_virtual(screen_width: u32, screen_height: u32) -> Self {
        Self {
            cursor_x: 0.5,
            cursor_y: 0.5,
            finger_down: false,
            screen_width,
            screen_height,
            hyprland: None,
            exact_x11: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MouseMotionSource;

    #[test]
    fn left_click_becomes_touch_down_and_up() {
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);

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
        let mut emulator = MouseTouchEmulator::new_virtual(100, 100);
        let _ = emulator.process(&InputEvent::KeyPress(Key::MouseLeft));
        let cmds = emulator.process(&InputEvent::MouseMove {
            dx: 10,
            dy: 5,
            source: MouseMotionSource::Relative,
        });
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
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);
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
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);
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

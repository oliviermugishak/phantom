use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::config::MouseTouchConfig;
use crate::engine::TouchCommand;
use crate::hyprland_cursor::HyprlandCursorClient;
use crate::input::{InputEvent, Key, MouseMotionSource};
use crate::logging::trace_detail_enabled;
use crate::overlay::CursorOverlayState;
use crate::x11_cursor::X11CursorClient;

pub const RUNTIME_MOUSE_TOUCH_SLOT: u8 = u8::MAX;
const ACCEL_MAX_DT: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy)]
pub struct MouseTouchAccelConfig {
    pub min_gain: f64,
    pub max_gain: f64,
    pub low_speed: f64,
    pub high_speed: f64,
    pub speed_smoothing: f64,
}

impl Default for MouseTouchAccelConfig {
    fn default() -> Self {
        Self::from_config(&MouseTouchConfig::default())
    }
}

impl MouseTouchAccelConfig {
    pub fn from_config(config: &MouseTouchConfig) -> Self {
        Self {
            min_gain: config.accel_min_gain,
            max_gain: config.accel_max_gain,
            low_speed: config.accel_low_speed.max(0.001),
            high_speed: config.accel_high_speed.max(config.accel_low_speed + 0.001),
            speed_smoothing: config.accel_speed_smoothing.clamp(0.0, 1.0),
        }
    }

    fn gain(&self, speed_px_per_ms: f64) -> f64 {
        let span = (self.high_speed - self.low_speed).max(0.001);
        let t = ((speed_px_per_ms - self.low_speed) / span).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        self.min_gain + (self.max_gain - self.min_gain) * smooth
    }
}

#[derive(Debug, Clone, Copy)]
enum ScrollDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HostFrame {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CursorSeed {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) frame: HostFrame,
}

#[derive(Debug)]
pub struct MouseTouchEmulator {
    cursor_x: f64,
    cursor_y: f64,
    finger_down: bool,
    host_frame: Option<HostFrame>,
    screen_width: u32,
    screen_height: u32,
    hyprland: Option<HyprlandCursorClient>,
    exact_x11: Option<X11CursorClient>,
    last_seed_source: Option<&'static str>,
    last_relative_move_at: Option<Instant>,
    smoothed_speed_px_per_ms: f64,
    accel: MouseTouchAccelConfig,
    scroll_step: f64,
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
                "owned-hyprland-ready+virtual"
            } else if exact_x11.is_some() {
                "owned-x11-ready+virtual"
            } else {
                "owned-virtual"
            },
            "mouse-touch backend ready"
        );

        Self {
            cursor_x: 0.5,
            cursor_y: 0.5,
            finger_down: false,
            host_frame: None,
            screen_width,
            screen_height,
            hyprland,
            exact_x11,
            last_seed_source: None,
            last_relative_move_at: None,
            smoothed_speed_px_per_ms: 0.0,
            accel: MouseTouchAccelConfig::default(),
            scroll_step: 0.06,
        }
    }

    pub fn apply_feel(&mut self, accel: MouseTouchAccelConfig, scroll_step: f64) {
        self.accel = accel;
        self.scroll_step = scroll_step.clamp(0.01, 0.4);
    }

    pub fn is_active(&self) -> bool {
        self.finger_down
    }

    pub fn backend_name(&self) -> &'static str {
        match self.last_seed_source {
            Some("hyprland") => "owned-hyprland-seeded+virtual",
            Some("x11") => "owned-x11-seeded+virtual",
            _ => "owned-virtual",
        }
    }

    pub fn seed_from_host_cursor(&mut self) {
        if let Some((seed, source)) = self.seed_position() {
            self.cursor_x = seed.x;
            self.cursor_y = seed.y;
            self.host_frame = Some(seed.frame);
            self.last_seed_source = Some(source);
            tracing::info!(
                backend = source,
                x = seed.x,
                y = seed.y,
                "seeded owned menu-touch cursor from host position"
            );
        } else {
            tracing::info!(
                x = self.cursor_x,
                y = self.cursor_y,
                "host cursor seed unavailable, reusing existing owned menu-touch cursor position"
            );
        }
    }

    pub fn cursor_overlay_state(&self, visible: bool) -> CursorOverlayState {
        let Some(frame) = self.host_frame else {
            return CursorOverlayState {
                visible: false,
                pressed: self.finger_down,
                screen_x: 0.0,
                screen_y: 0.0,
            };
        };

        CursorOverlayState {
            visible,
            pressed: self.finger_down,
            screen_x: (frame.left + self.cursor_x * frame.width) as f32,
            screen_y: (frame.top + self.cursor_y * frame.height) as f32,
        }
    }

    pub fn overlay_frame(&self) -> Option<crate::overlay::OverlayFrame> {
        self.host_frame.map(Into::into)
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
            InputEvent::MouseMove { dx, dy, source } => self.handle_move(*dx, *dy, *source),
            InputEvent::KeyPress(Key::MouseLeft) => self.handle_press(),
            InputEvent::KeyRelease(Key::MouseLeft) => self.handle_release(),
            InputEvent::KeyPress(Key::WheelUp) => self.handle_scroll(ScrollDirection::Up),
            InputEvent::KeyPress(Key::WheelDown) => self.handle_scroll(ScrollDirection::Down),
            _ => Vec::new(),
        }
    }

    fn handle_move(&mut self, dx: i32, dy: i32, source: MouseMotionSource) -> Vec<TouchCommand> {
        self.update_virtual_cursor(dx, dy, source);

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

    fn update_virtual_cursor(&mut self, dx: i32, dy: i32, source: MouseMotionSource) {
        let (width, height) = self
            .host_frame
            .map(|frame| (frame.width.max(1.0), frame.height.max(1.0)))
            .unwrap_or_else(|| {
                (
                    self.screen_width.max(1) as f64,
                    self.screen_height.max(1) as f64,
                )
            });
        let gain = match source {
            MouseMotionSource::Relative => self.relative_adaptive_gain(dx, dy),
            MouseMotionSource::Absolute => 1.35,
        };
        self.cursor_x = (self.cursor_x + (dx as f64 * gain) / width).clamp(0.0, 1.0);
        self.cursor_y = (self.cursor_y + (dy as f64 * gain) / height).clamp(0.0, 1.0);
    }

    fn relative_adaptive_gain(&mut self, dx: i32, dy: i32) -> f64 {
        let now = Instant::now();
        let dt = self
            .last_relative_move_at
            .map(|previous| now.saturating_duration_since(previous).min(ACCEL_MAX_DT))
            .unwrap_or(ACCEL_MAX_DT);
        self.last_relative_move_at = Some(now);
        let dt_ms = dt.as_secs_f64() * 1000.0;
        let instant_speed = ((dx as f64).hypot(dy as f64) / dt_ms.max(0.001)).max(0.0);
        self.smoothed_speed_px_per_ms +=
            self.accel.speed_smoothing * (instant_speed - self.smoothed_speed_px_per_ms);
        self.accel.gain(self.smoothed_speed_px_per_ms)
    }

    fn handle_scroll(&mut self, direction: ScrollDirection) -> Vec<TouchCommand> {
        if self.finger_down {
            return Vec::new();
        }
        let offset = match direction {
            ScrollDirection::Up => -self.scroll_step,
            ScrollDirection::Down => self.scroll_step,
        };
        let start_y = self.cursor_y;
        let end_y = (self.cursor_y + offset).clamp(0.0, 1.0);
        vec![
            TouchCommand::TouchDown {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                x: self.cursor_x,
                y: start_y,
            },
            TouchCommand::TouchMove {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
                x: self.cursor_x,
                y: end_y,
            },
            TouchCommand::TouchUp {
                slot: RUNTIME_MOUSE_TOUCH_SLOT,
            },
        ]
    }

    pub fn refresh_host_frame(&mut self) -> bool {
        let Some(frame) = self.query_host_frame() else {
            return false;
        };
        if self.host_frame == Some(frame) {
            return false;
        }
        self.host_frame = Some(frame);
        true
    }

    fn query_host_frame(&mut self) -> Option<HostFrame> {
        if let Some(client) = self.hyprland.as_mut() {
            match client.query_frame() {
                Ok(Some(frame)) => return Some(frame),
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!("hyprland frame helper stopped responding: {}", err);
                    self.hyprland = None;
                }
            }
        }
        None
    }

    fn seed_position(&mut self) -> Option<(CursorSeed, &'static str)> {
        if let Some(client) = self.hyprland.as_mut() {
            match client.query_seed() {
                Ok(Some(position)) => return Some((position, "hyprland")),
                Ok(None) => {
                    if trace_detail_enabled() {
                        tracing::trace!(
                            "hyprland cursor helper returned no window under the pointer"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "hyprland cursor helper stopped responding: {}; falling back to x11/virtual cursor",
                        err
                    );
                    self.hyprland = None;
                }
            }
        }
        if let Some(client) = self.exact_x11.as_mut() {
            match client.query_seed() {
                Ok(Some(position)) => return Some((position, "x11")),
                Ok(None) => {
                    if trace_detail_enabled() {
                        tracing::trace!("x11 cursor helper returned no window under the pointer");
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "x11 cursor helper stopped responding: {}; falling back to virtual cursor",
                        err
                    );
                    self.exact_x11 = None;
                }
            }
        }
        None
    }

    #[cfg(test)]
    fn new_virtual(screen_width: u32, screen_height: u32) -> Self {
        Self {
            cursor_x: 0.5,
            cursor_y: 0.5,
            finger_down: false,
            host_frame: None,
            screen_width,
            screen_height,
            hyprland: None,
            exact_x11: None,
            last_seed_source: None,
            last_relative_move_at: None,
            smoothed_speed_px_per_ms: 0.0,
            accel: MouseTouchAccelConfig::default(),
            scroll_step: 0.06,
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

        let replay = emulator.resync_buttons(&pressed);
        assert!(matches!(
            replay.as_slice(),
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

    #[test]
    fn cursor_overlay_state_uses_seeded_host_frame() {
        let mut emulator = MouseTouchEmulator::new_virtual(100, 100);
        emulator.host_frame = Some(HostFrame {
            left: 100.0,
            top: 200.0,
            width: 300.0,
            height: 400.0,
        });
        emulator.cursor_x = 0.25;
        emulator.cursor_y = 0.5;

        let state = emulator.cursor_overlay_state(true);
        assert!(state.visible);
        assert_eq!(state.screen_x, 175.0);
        assert_eq!(state.screen_y, 400.0);
    }

    #[test]
    fn wheel_up_emits_swipe_without_holding_slot() {
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);
        emulator.cursor_x = 0.4;
        emulator.cursor_y = 0.5;
        let cmds = emulator.process(&InputEvent::KeyPress(Key::WheelUp));
        assert!(matches!(
            cmds.as_slice(),
            [
                TouchCommand::TouchDown {
                    slot: RUNTIME_MOUSE_TOUCH_SLOT,
                    x,
                    y
                },
                TouchCommand::TouchMove {
                    slot: RUNTIME_MOUSE_TOUCH_SLOT,
                    ..
                },
                TouchCommand::TouchUp {
                    slot: RUNTIME_MOUSE_TOUCH_SLOT
                }
            ] if (*x - 0.4).abs() < f64::EPSILON && *y > 0.43
        ));
        assert!(!emulator.is_active());
        assert_eq!(emulator.cursor_y, 0.5);
    }

    #[test]
    fn wheel_down_swipes_opposite_direction() {
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);
        emulator.cursor_y = 0.5;
        let cmds = emulator.process(&InputEvent::KeyPress(Key::WheelDown));
        match cmds.as_slice() {
            [TouchCommand::TouchDown { y: start, .. }, TouchCommand::TouchMove { y: end, .. }, TouchCommand::TouchUp { .. }] =>
            {
                assert!(*end > *start);
            }
            other => panic!("unexpected scroll commands: {other:?}"),
        }
    }

    #[test]
    fn wheel_ignored_while_left_button_held() {
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);
        let _ = emulator.process(&InputEvent::KeyPress(Key::MouseLeft));
        assert!(emulator
            .process(&InputEvent::KeyPress(Key::WheelUp))
            .is_empty());
        assert!(emulator.is_active());
    }

    #[test]
    fn refresh_host_frame_does_not_move_cursor_when_unchanged() {
        let mut emulator = MouseTouchEmulator::new_virtual(1920, 1080);
        emulator.cursor_x = 0.25;
        emulator.cursor_y = 0.5;
        emulator.host_frame = Some(HostFrame {
            left: 10.0,
            top: 20.0,
            width: 100.0,
            height: 200.0,
        });
        assert!(!emulator.refresh_host_frame());
        assert_eq!(emulator.cursor_x, 0.25);
        assert_eq!(emulator.cursor_y, 0.5);
    }

    #[test]
    fn absolute_gain_stays_constant() {
        let mut emulator = MouseTouchEmulator::new_virtual(100, 100);
        emulator.process(&InputEvent::MouseMove {
            dx: 10,
            dy: 0,
            source: MouseMotionSource::Absolute,
        });
        assert!((emulator.cursor_x - 0.635).abs() < 0.0001);
    }
}

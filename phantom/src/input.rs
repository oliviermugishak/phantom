use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str::FromStr;

use crate::error::{PhantomError, Result};

nix::ioctl_write_int!(eviocgrab, b'E', 0x90);

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;

const SYN_REPORT: u16 = 0x00;
const SYN_DROPPED: u16 = 0x03;

const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_WHEEL: u16 = 0x08;

const EVDEV_CAP_MAX: usize = 0x20;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawInputEvent {
    pub tv_sec: i64,
    pub tv_usec: i64,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

const _: [(); 24] = [(); std::mem::size_of::<RawInputEvent>()];

#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyPress(Key),
    KeyRelease(Key),
    MouseMove { dx: i32, dy: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    LeftCtrl,
    RightCtrl,
    LeftShift,
    RightShift,
    LeftAlt,
    RightAlt,
    LeftMeta,
    RightMeta,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Tab,
    Esc,
    Minus,
    Equal,
    LeftBrace,
    RightBrace,
    Semicolon,
    Apostrophe,
    Grave,
    Backslash,
    Comma,
    Dot,
    Slash,
    CapsLock,
    NumLock,
    ScrollLock,
    SysRq,
    Pause,
    KP0,
    KP1,
    KP2,
    KP3,
    KP4,
    KP5,
    KP6,
    KP7,
    KP8,
    KP9,
    MouseLeft,
    MouseRight,
    MouseMiddle,
    MouseBack,
    MouseForward,
    WheelUp,
    WheelDown,
    Unknown(u16),
}

impl Key {
    pub fn parse_name(s: &str) -> Option<Key> {
        let upper = s.to_uppercase();
        match upper.as_str() {
            // Aliases
            "CTRL" => Some(Key::LeftCtrl),
            "SHIFT" => Some(Key::LeftShift),
            "ALT" => Some(Key::LeftAlt),
            "WIN" | "SUPER" => Some(Key::LeftMeta),
            "ENTER" => Some(Key::Enter),
            "ESC" => Some(Key::Esc),
            "SPACE" => Some(Key::Space),
            "TAB" => Some(Key::Tab),
            "BACKSPACE" => Some(Key::Backspace),
            "DELETE" => Some(Key::Delete),
            "INSERT" => Some(Key::Insert),
            "UP" => Some(Key::Up),
            "DOWN" => Some(Key::Down),
            "LEFT" => Some(Key::Left),
            "RIGHT" => Some(Key::Right),
            "HOME" => Some(Key::Home),
            "END" => Some(Key::End),
            "PAGEUP" => Some(Key::PageUp),
            "PAGEDOWN" => Some(Key::PageDown),
            "CAPSLOCK" => Some(Key::CapsLock),
            "MOUSELEFT" => Some(Key::MouseLeft),
            "MOUSERIGHT" => Some(Key::MouseRight),
            "MOUSEMIDDLE" => Some(Key::MouseMiddle),
            "MOUSEBACK" => Some(Key::MouseBack),
            "MOUSEFORWARD" => Some(Key::MouseForward),
            "WHEELUP" => Some(Key::WheelUp),
            "WHEELDOWN" => Some(Key::WheelDown),
            "LEFTCTRL" => Some(Key::LeftCtrl),
            "RIGHTCTRL" => Some(Key::RightCtrl),
            "LEFTSHIFT" => Some(Key::LeftShift),
            "RIGHTSHIFT" => Some(Key::RightShift),
            "LEFTALT" => Some(Key::LeftAlt),
            "RIGHTALT" => Some(Key::RightAlt),
            "LEFTMETA" => Some(Key::LeftMeta),
            "RIGHTMETA" => Some(Key::RightMeta),
            // Punctuation
            "MINUS" | "-" => Some(Key::Minus),
            "EQUAL" | "=" => Some(Key::Equal),
            "LEFTBRACE" | "[" => Some(Key::LeftBrace),
            "RIGHTBRACE" | "]" => Some(Key::RightBrace),
            "SEMICOLON" | ";" => Some(Key::Semicolon),
            "APOSTROPHE" | "'" => Some(Key::Apostrophe),
            "GRAVE" | "`" => Some(Key::Grave),
            "BACKSLASH" | "\\" => Some(Key::Backslash),
            "COMMA" | "," => Some(Key::Comma),
            "DOT" | "." => Some(Key::Dot),
            "SLASH" | "/" => Some(Key::Slash),
            // Locks and system
            "NUMLOCK" => Some(Key::NumLock),
            "SCROLLLOCK" => Some(Key::ScrollLock),
            "SYSRQ" => Some(Key::SysRq),
            "PAUSE" => Some(Key::Pause),
            // Letters
            "A" => Some(Key::A),
            "B" => Some(Key::B),
            "C" => Some(Key::C),
            "D" => Some(Key::D),
            "E" => Some(Key::E),
            "F" => Some(Key::F),
            "G" => Some(Key::G),
            "H" => Some(Key::H),
            "I" => Some(Key::I),
            "J" => Some(Key::J),
            "K" => Some(Key::K),
            "L" => Some(Key::L),
            "M" => Some(Key::M),
            "N" => Some(Key::N),
            "O" => Some(Key::O),
            "P" => Some(Key::P),
            "Q" => Some(Key::Q),
            "R" => Some(Key::R),
            "S" => Some(Key::S),
            "T" => Some(Key::T),
            "U" => Some(Key::U),
            "V" => Some(Key::V),
            "W" => Some(Key::W),
            "X" => Some(Key::X),
            "Y" => Some(Key::Y),
            "Z" => Some(Key::Z),
            // Numbers
            "0" => Some(Key::Key0),
            "1" => Some(Key::Key1),
            "2" => Some(Key::Key2),
            "3" => Some(Key::Key3),
            "4" => Some(Key::Key4),
            "5" => Some(Key::Key5),
            "6" => Some(Key::Key6),
            "7" => Some(Key::Key7),
            "8" => Some(Key::Key8),
            "9" => Some(Key::Key9),
            // Function keys
            "F1" => Some(Key::F1),
            "F2" => Some(Key::F2),
            "F3" => Some(Key::F3),
            "F4" => Some(Key::F4),
            "F5" => Some(Key::F5),
            "F6" => Some(Key::F6),
            "F7" => Some(Key::F7),
            "F8" => Some(Key::F8),
            "F9" => Some(Key::F9),
            "F10" => Some(Key::F10),
            "F11" => Some(Key::F11),
            "F12" => Some(Key::F12),
            // Numpad
            "KP0" => Some(Key::KP0),
            "KP1" => Some(Key::KP1),
            "KP2" => Some(Key::KP2),
            "KP3" => Some(Key::KP3),
            "KP4" => Some(Key::KP4),
            "KP5" => Some(Key::KP5),
            "KP6" => Some(Key::KP6),
            "KP7" => Some(Key::KP7),
            "KP8" => Some(Key::KP8),
            "KP9" => Some(Key::KP9),
            _ => None,
        }
    }
}

impl FromStr for Key {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse_name(s).ok_or("unknown key")
    }
}

/// Map linux evdev key codes to our Key enum.
fn evdev_code_to_key(code: u16) -> Option<Key> {
    match code {
        1 => Some(Key::Esc),
        2..=11 => Some(
            [
                Key::Key1,
                Key::Key2,
                Key::Key3,
                Key::Key4,
                Key::Key5,
                Key::Key6,
                Key::Key7,
                Key::Key8,
                Key::Key9,
                Key::Key0,
            ][(code - 2) as usize],
        ),
        12 => Some(Key::Minus),
        13 => Some(Key::Equal),
        14 => Some(Key::Backspace),
        15 => Some(Key::Tab),
        16 => Some(Key::Q),
        17 => Some(Key::W),
        18 => Some(Key::E),
        19 => Some(Key::R),
        20 => Some(Key::T),
        21 => Some(Key::Y),
        22 => Some(Key::U),
        23 => Some(Key::I),
        24 => Some(Key::O),
        25 => Some(Key::P),
        26 => Some(Key::LeftBrace),
        27 => Some(Key::RightBrace),
        28 => Some(Key::Enter),
        29 => Some(Key::LeftCtrl),
        30 => Some(Key::A),
        31 => Some(Key::S),
        32 => Some(Key::D),
        33 => Some(Key::F),
        34 => Some(Key::G),
        35 => Some(Key::H),
        36 => Some(Key::J),
        37 => Some(Key::K),
        38 => Some(Key::L),
        39 => Some(Key::Semicolon),
        40 => Some(Key::Apostrophe),
        41 => Some(Key::Grave),
        42 => Some(Key::LeftShift),
        43 => Some(Key::Backslash),
        44 => Some(Key::Z),
        45 => Some(Key::X),
        46 => Some(Key::C),
        47 => Some(Key::V),
        48 => Some(Key::B),
        49 => Some(Key::N),
        50 => Some(Key::M),
        51 => Some(Key::Comma),
        52 => Some(Key::Dot),
        53 => Some(Key::Slash),
        54 => Some(Key::RightShift),
        55 => Some(Key::Unknown(55)), // KP*
        56 => Some(Key::LeftAlt),
        57 => Some(Key::Space),
        58 => Some(Key::CapsLock),
        59..=68 => Some(
            [
                Key::F1,
                Key::F2,
                Key::F3,
                Key::F4,
                Key::F5,
                Key::F6,
                Key::F7,
                Key::F8,
                Key::F9,
                Key::F10,
            ][(code - 59) as usize],
        ),
        69 => Some(Key::NumLock),
        70 => Some(Key::ScrollLock),
        // Numpad 7-9, 4-6, 1-3, 0
        71 => Some(Key::KP7),
        72 => Some(Key::KP8),
        73 => Some(Key::KP9),
        74 => Some(Key::KP4),
        75 => Some(Key::KP5),
        76 => Some(Key::KP6),
        77 => Some(Key::KP1),
        78 => Some(Key::KP2),
        79 => Some(Key::KP3),
        80 => Some(Key::KP0),
        87 => Some(Key::F11),
        88 => Some(Key::F12),
        97 => Some(Key::RightCtrl),
        99 => Some(Key::SysRq),
        100 => Some(Key::RightAlt),
        102 => Some(Key::Home),
        103 => Some(Key::Up),
        104 => Some(Key::PageUp),
        105 => Some(Key::Left),
        106 => Some(Key::Right),
        107 => Some(Key::End),
        108 => Some(Key::Down),
        109 => Some(Key::PageDown),
        110 => Some(Key::Insert),
        111 => Some(Key::Delete),
        119 => Some(Key::Pause),
        125 => Some(Key::LeftMeta),
        0x110 => Some(Key::MouseLeft),
        0x111 => Some(Key::MouseRight),
        0x112 => Some(Key::MouseMiddle),
        0x113 => Some(Key::MouseBack),
        0x114 => Some(Key::MouseForward),
        _ => Some(Key::Unknown(code)),
    }
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub path: String,
    pub name: String,
    pub file: File,
    pub is_keyboard: bool,
    pub is_mouse: bool,
}

impl DeviceInfo {
    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

pub struct InputCapture {
    devices: Vec<DeviceInfo>,
    epoll_fd: RawFd,
    grabbed: bool,
}

impl InputCapture {
    pub fn discover_and_grab() -> Result<Self> {
        let mut devices = Vec::new();
        let entries = fs::read_dir("/dev/input").map_err(|e| PhantomError::DeviceNotFound {
            path: format!("/dev/input: {}", e),
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if !path_str.contains("event") {
                continue;
            }

            match Self::probe_device(&path_str) {
                Ok(Some(info)) => {
                    tracing::info!("grabbed: {} ({})", path_str, info.name);
                    devices.push(info);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("skipped {}: {}", path_str, e);
                }
            }
        }

        if devices.is_empty() {
            return Err(PhantomError::NoInputDevices);
        }

        let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epoll_fd < 0 {
            return Err(PhantomError::Io(std::io::Error::last_os_error()));
        }

        for dev in &devices {
            let mut event = libc::epoll_event {
                events: (libc::EPOLLIN | libc::EPOLLET) as u32,
                u64: dev.fd() as u64,
            };
            let ret =
                unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, dev.fd(), &mut event) };
            if ret < 0 {
                return Err(PhantomError::Io(std::io::Error::last_os_error()));
            }
        }

        tracing::info!("captured {} input devices", devices.len());
        Ok(Self {
            devices,
            epoll_fd,
            grabbed: true,
        })
    }

    fn probe_device(path: &str) -> Result<Option<DeviceInfo>> {
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(PhantomError::Io)?;

        let fd = file.as_raw_fd();

        // Read device name
        let mut name_buf = [0u8; 256];
        let name_len = unsafe {
            libc::ioctl(
                fd,
                eviocgname_request(name_buf.len()),
                name_buf.as_mut_ptr(),
            )
        };
        let device_name = if name_len > 0 {
            String::from_utf8_lossy(&name_buf[..name_len as usize])
                .trim_end_matches('\0')
                .to_string()
        } else {
            "unknown".to_string()
        };

        // Skip our own device
        if device_name.contains("Phantom") {
            return Ok(None);
        }

        // Check event type capabilities
        let mut ev_bits = [0u8; EVDEV_CAP_MAX.div_ceil(8)];
        let ret = unsafe {
            libc::ioctl(
                fd,
                eviocgbit_request(0, ev_bits.len()),
                ev_bits.as_mut_ptr(),
            )
        };
        if ret < 0 {
            return Ok(None);
        }

        let has_key = test_bit(EV_KEY, &ev_bits);
        let has_rel = test_bit(EV_REL, &ev_bits);

        if !has_key && !has_rel {
            return Ok(None);
        }

        // Check specific key capabilities for keyboard detection
        let key_buf_size = (0x114usize + 1).div_ceil(8); // enough for all keys including mouse
        let mut key_bits = vec![0u8; key_buf_size];
        let ret = unsafe {
            libc::ioctl(
                fd,
                eviocgbit_request(EV_KEY, key_bits.len()),
                key_bits.as_mut_ptr(),
            )
        };
        let is_keyboard = ret >= 0 && test_bit(30 /* KEY_A */, &key_bits);

        // Check relative axis capabilities for mouse detection
        let mut rel_bits = [0u8; (REL_Y as usize + 1).div_ceil(8)];
        let ret = unsafe {
            libc::ioctl(
                fd,
                eviocgbit_request(EV_REL, rel_bits.len()),
                rel_bits.as_mut_ptr(),
            )
        };
        let is_mouse =
            has_rel && ret >= 0 && test_bit(REL_X, &rel_bits) && test_bit(REL_Y, &rel_bits);

        if !is_keyboard && !is_mouse {
            return Ok(None);
        }

        // Re-open non-blocking for epoll
        drop(file);
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(PhantomError::Io)?;
        let fd = file.as_raw_fd();

        // Grab exclusive
        if let Err(err) = unsafe { eviocgrab(fd, 1) } {
            return Err(PhantomError::IoctlFailed {
                operation: "EVIOCGRAB".into(),
                path: path.into(),
                reason: std::io::Error::from_raw_os_error(err as i32).to_string(),
            });
        }

        Ok(Some(DeviceInfo {
            path: path.to_string(),
            name: device_name,
            file,
            is_keyboard,
            is_mouse,
        }))
    }

    pub fn poll_events(&self, timeout_ms: i32) -> Result<Vec<(RawFd, RawInputEvent)>> {
        let mut epoll_events = [libc::epoll_event { events: 0, u64: 0 }; 16];
        let nfds = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                epoll_events.as_mut_ptr(),
                epoll_events.len() as i32,
                timeout_ms,
            )
        };
        if nfds < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                return Ok(vec![]);
            }
            return Err(PhantomError::Io(err));
        }

        let mut events = Vec::new();
        for epoll_event in epoll_events.iter().take(nfds as usize) {
            let fd = epoll_event.u64 as RawFd;
            if epoll_event.events & (libc::EPOLLERR as u32 | libc::EPOLLHUP as u32) != 0 {
                tracing::warn!("input fd {} reported EPOLLERR/EPOLLHUP", fd);
            }
            events.extend(Self::read_events(fd)?);
        }
        Ok(events)
    }

    fn read_events(fd: RawFd) -> Result<Vec<(RawFd, RawInputEvent)>> {
        let mut events = Vec::new();
        let event_size = std::mem::size_of::<RawInputEvent>();
        let mut buf = [RawInputEvent {
            tv_sec: 0,
            tv_usec: 0,
            type_: 0,
            code: 0,
            value: 0,
        }; 256];

        loop {
            let n = unsafe {
                libc::read(
                    fd,
                    buf.as_mut_ptr().cast::<libc::c_void>(),
                    std::mem::size_of_val(&buf),
                )
            };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                if matches!(err.raw_os_error(), Some(libc::EAGAIN)) {
                    break;
                }
                if matches!(err.raw_os_error(), Some(libc::ENODEV)) {
                    tracing::warn!("input device fd {} disappeared", fd);
                    break;
                }
                return Err(PhantomError::Io(err));
            }
            if n == 0 {
                break;
            }
            if !(n as usize).is_multiple_of(event_size) {
                tracing::warn!("discarding short read from fd {}: {} bytes", fd, n);
                break;
            }
            let count = n as usize / event_size;
            for event in buf.iter().take(count) {
                events.push((fd, *event));
            }
        }
        Ok(events)
    }

    pub fn process_events(&self, raw: &[(RawFd, RawInputEvent)]) -> Vec<InputEvent> {
        let mut result = Vec::new();
        let mut dropped_fds = HashSet::new();

        for (fd, event) in raw {
            if event.type_ == EV_SYN {
                if event.code == SYN_DROPPED {
                    tracing::warn!(
                        "SYN_DROPPED on fd {}, dropping buffered events until next SYN_REPORT",
                        fd
                    );
                    dropped_fds.insert(*fd);
                    continue;
                }
                if event.code == SYN_REPORT {
                    dropped_fds.remove(fd);
                }
                continue;
            }

            if dropped_fds.contains(fd) {
                continue;
            }

            if event.type_ == EV_KEY {
                // Filter repeat events (value == 2)
                if event.value == 2 {
                    continue;
                }
                if let Some(key) = evdev_code_to_key(event.code) {
                    if event.value == 1 {
                        result.push(InputEvent::KeyPress(key));
                    } else if event.value == 0 {
                        result.push(InputEvent::KeyRelease(key));
                    }
                }
            } else if event.type_ == EV_REL {
                if event.code == REL_X {
                    result.push(InputEvent::MouseMove {
                        dx: event.value,
                        dy: 0,
                    });
                } else if event.code == REL_Y {
                    result.push(InputEvent::MouseMove {
                        dx: 0,
                        dy: event.value,
                    });
                } else if event.code == REL_WHEEL {
                    // Map scroll wheel to key press+release
                    if event.value != 0 {
                        let key = if event.value > 0 {
                            Key::WheelUp
                        } else {
                            Key::WheelDown
                        };
                        result.push(InputEvent::KeyPress(key));
                        result.push(InputEvent::KeyRelease(key));
                    }
                }
            }
        }

        Self::merge_mouse_moves(&mut result);
        result
    }

    fn merge_mouse_moves(events: &mut Vec<InputEvent>) {
        let mut i = 0;
        while i + 1 < events.len() {
            if let (
                InputEvent::MouseMove { dx: dx1, dy: dy1 },
                InputEvent::MouseMove { dx: dx2, dy: dy2 },
            ) = (&events[i], &events[i + 1])
            {
                events[i] = InputEvent::MouseMove {
                    dx: dx1 + dx2,
                    dy: dy1 + dy2,
                };
                events.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn has_mouse(&self) -> bool {
        self.devices.iter().any(|d| d.is_mouse)
    }

    pub fn has_keyboard(&self) -> bool {
        self.devices.iter().any(|d| d.is_keyboard)
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    pub fn set_grabbed(&mut self, grabbed: bool) -> Result<()> {
        if self.grabbed == grabbed {
            return Ok(());
        }

        for dev in &self.devices {
            let value = if grabbed { 1 } else { 0 };
            if let Err(err) = unsafe { eviocgrab(dev.fd(), value) } {
                return Err(PhantomError::IoctlFailed {
                    operation: "EVIOCGRAB".into(),
                    path: dev.path.clone(),
                    reason: std::io::Error::from_raw_os_error(err as i32).to_string(),
                });
            }
        }

        self.grabbed = grabbed;
        tracing::info!(
            "{} exclusive capture for {} devices",
            if grabbed { "enabled" } else { "disabled" },
            self.devices.len()
        );
        Ok(())
    }
}

impl Drop for InputCapture {
    fn drop(&mut self) {
        if self.grabbed {
            for dev in &self.devices {
                unsafe {
                    let _ = eviocgrab(dev.fd(), 0);
                }
                tracing::info!("released: {}", dev.path);
            }
        }
        unsafe {
            libc::close(self.epoll_fd);
        }
    }
}

fn test_bit(bit: u16, bits: &[u8]) -> bool {
    let idx = (bit as usize) / 8;
    let off = (bit as usize) % 8;
    if idx >= bits.len() {
        return false;
    }
    (bits[idx] >> off) & 1 != 0
}

fn eviocgname_request(len: usize) -> libc::c_ulong {
    nix::request_code_read!(b'E', 0x06, len) as libc::c_ulong
}

fn eviocgbit_request(ev: u16, len: usize) -> libc::c_ulong {
    nix::request_code_read!(b'E', 0x20 + ev as u8, len) as libc::c_ulong
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_requests_match_kernel_headers_on_x86_64() {
        assert_eq!(eviocgname_request(256), 0x8100_4506);
        assert_eq!(eviocgbit_request(0, 4), 0x8004_4520);
        assert_eq!(
            nix::request_code_write!(b'E', 0x90, std::mem::size_of::<libc::c_int>()),
            0x4004_4590
        );
    }

    #[test]
    fn merge_mouse_moves_only_merges_adjacent_moves() {
        let mut events = vec![
            InputEvent::MouseMove { dx: 1, dy: 0 },
            InputEvent::MouseMove { dx: 0, dy: 2 },
            InputEvent::KeyPress(Key::A),
            InputEvent::MouseMove { dx: 3, dy: 4 },
        ];
        InputCapture::merge_mouse_moves(&mut events);

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], InputEvent::MouseMove { dx: 1, dy: 2 }));
        assert!(matches!(events[1], InputEvent::KeyPress(Key::A)));
        assert!(matches!(events[2], InputEvent::MouseMove { dx: 3, dy: 4 }));
    }
}

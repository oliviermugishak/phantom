use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str::FromStr;

use crate::error::{PhantomError, Result};
use crate::logging::trace_detail_enabled;

nix::ioctl_write_int!(eviocgrab, b'E', 0x90);

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;

const SYN_REPORT: u16 = 0x00;
const SYN_DROPPED: u16 = 0x03;

const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_WHEEL: u16 = 0x08;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;

const EVDEV_CAP_MAX: usize = 0x20;
const EVDEV_KEY_MAX: usize = 0x2ff;
const EVDEV_KEY_BUF_SIZE: usize = (EVDEV_KEY_MAX + 1).div_ceil(8);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawInputEvent {
    pub tv_sec: i64,
    pub tv_usec: i64,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InputAbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

const _: [(); 24] = [(); std::mem::size_of::<RawInputEvent>()];
const _: [(); 24] = [(); std::mem::size_of::<InputAbsInfo>()];

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
    pub fn is_mouse(self) -> bool {
        matches!(
            self,
            Key::MouseLeft
                | Key::MouseRight
                | Key::MouseMiddle
                | Key::MouseBack
                | Key::MouseForward
                | Key::WheelUp
                | Key::WheelDown
        )
    }

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

    pub fn evdev_code(self) -> Option<u16> {
        match self {
            Key::Esc => Some(1),
            Key::Key1 => Some(2),
            Key::Key2 => Some(3),
            Key::Key3 => Some(4),
            Key::Key4 => Some(5),
            Key::Key5 => Some(6),
            Key::Key6 => Some(7),
            Key::Key7 => Some(8),
            Key::Key8 => Some(9),
            Key::Key9 => Some(10),
            Key::Key0 => Some(11),
            Key::Minus => Some(12),
            Key::Equal => Some(13),
            Key::Backspace => Some(14),
            Key::Tab => Some(15),
            Key::Q => Some(16),
            Key::W => Some(17),
            Key::E => Some(18),
            Key::R => Some(19),
            Key::T => Some(20),
            Key::Y => Some(21),
            Key::U => Some(22),
            Key::I => Some(23),
            Key::O => Some(24),
            Key::P => Some(25),
            Key::LeftBrace => Some(26),
            Key::RightBrace => Some(27),
            Key::Enter => Some(28),
            Key::LeftCtrl => Some(29),
            Key::A => Some(30),
            Key::S => Some(31),
            Key::D => Some(32),
            Key::F => Some(33),
            Key::G => Some(34),
            Key::H => Some(35),
            Key::J => Some(36),
            Key::K => Some(37),
            Key::L => Some(38),
            Key::Semicolon => Some(39),
            Key::Apostrophe => Some(40),
            Key::Grave => Some(41),
            Key::LeftShift => Some(42),
            Key::Backslash => Some(43),
            Key::Z => Some(44),
            Key::X => Some(45),
            Key::C => Some(46),
            Key::V => Some(47),
            Key::B => Some(48),
            Key::N => Some(49),
            Key::M => Some(50),
            Key::Comma => Some(51),
            Key::Dot => Some(52),
            Key::Slash => Some(53),
            Key::RightShift => Some(54),
            Key::LeftAlt => Some(56),
            Key::Space => Some(57),
            Key::CapsLock => Some(58),
            Key::F1 => Some(59),
            Key::F2 => Some(60),
            Key::F3 => Some(61),
            Key::F4 => Some(62),
            Key::F5 => Some(63),
            Key::F6 => Some(64),
            Key::F7 => Some(65),
            Key::F8 => Some(66),
            Key::F9 => Some(67),
            Key::F10 => Some(68),
            Key::NumLock => Some(69),
            Key::ScrollLock => Some(70),
            Key::KP7 => Some(71),
            Key::KP8 => Some(72),
            Key::KP9 => Some(73),
            Key::KP4 => Some(74),
            Key::KP5 => Some(75),
            Key::KP6 => Some(76),
            Key::KP1 => Some(77),
            Key::KP2 => Some(78),
            Key::KP3 => Some(79),
            Key::KP0 => Some(80),
            Key::F11 => Some(87),
            Key::F12 => Some(88),
            Key::RightCtrl => Some(97),
            Key::SysRq => Some(99),
            Key::RightAlt => Some(100),
            Key::Home => Some(102),
            Key::Up => Some(103),
            Key::PageUp => Some(104),
            Key::Left => Some(105),
            Key::Right => Some(106),
            Key::End => Some(107),
            Key::Down => Some(108),
            Key::PageDown => Some(109),
            Key::Insert => Some(110),
            Key::Delete => Some(111),
            Key::Pause => Some(119),
            Key::LeftMeta => Some(125),
            Key::RightMeta => Some(126),
            Key::MouseLeft => Some(0x110),
            Key::MouseRight => Some(0x111),
            Key::MouseMiddle => Some(0x112),
            Key::MouseBack => Some(0x113),
            Key::MouseForward => Some(0x114),
            Key::Unknown(code) => Some(code),
            Key::WheelUp | Key::WheelDown => None,
        }
    }
}

impl InputEvent {
    pub fn is_mouse_input(&self) -> bool {
        match self {
            InputEvent::MouseMove { .. } => true,
            InputEvent::KeyPress(key) | InputEvent::KeyRelease(key) => key.is_mouse(),
        }
    }

    pub fn is_keyboard_input(&self) -> bool {
        match self {
            InputEvent::MouseMove { .. } => false,
            InputEvent::KeyPress(key) | InputEvent::KeyRelease(key) => !key.is_mouse(),
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
    pointer_kind: PointerKind,
    abs_x: Option<i32>,
    abs_y: Option<i32>,
    abs_range_x: Option<(i32, i32)>,
    abs_range_y: Option<(i32, i32)>,
    last_abs_position: Option<(i32, i32)>,
    abs_dirty: bool,
    pressed_keys: HashSet<Key>,
    desynced: bool,
}

impl DeviceInfo {
    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

pub struct InputCapture {
    devices: Vec<DeviceInfo>,
    fd_to_index: HashMap<RawFd, usize>,
    epoll_fd: RawFd,
    mouse_grabbed: bool,
    keyboard_grabbed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerKind {
    None,
    Relative,
    Absolute,
}

impl InputCapture {
    /// Discover input devices and set up epoll for reading.
    /// Device discovery itself does not decide the grab policy.
    /// The daemon applies the runtime grab policy after startup.
    pub fn discover() -> Result<Self> {
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

            match Self::probe_device_open(&path_str) {
                Ok(Some(info)) => {
                    tracing::info!("watching: {} ({})", path_str, info.name);
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

        let mut fd_to_index = HashMap::with_capacity(devices.len());
        for (idx, dev) in devices.iter().enumerate() {
            fd_to_index.insert(dev.fd(), idx);
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

        tracing::info!("watching {} input devices", devices.len());
        Ok(Self {
            devices,
            fd_to_index,
            epoll_fd,
            mouse_grabbed: false,
            keyboard_grabbed: false,
        })
    }

    fn probe_device_open(path: &str) -> Result<Option<DeviceInfo>> {
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
        let has_abs = test_bit(EV_ABS, &ev_bits);

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
        let has_pointer_buttons =
            ret >= 0 && (test_bit(0x110 /* BTN_LEFT */, &key_bits) || test_bit(0x111, &key_bits));

        // Check relative axis capabilities for mouse detection
        let mut rel_bits = [0u8; (REL_Y as usize + 1).div_ceil(8)];
        let ret = unsafe {
            libc::ioctl(
                fd,
                eviocgbit_request(EV_REL, rel_bits.len()),
                rel_bits.as_mut_ptr(),
            )
        };
        let has_rel_pointer =
            has_rel && ret >= 0 && test_bit(REL_X, &rel_bits) && test_bit(REL_Y, &rel_bits);

        let mut abs_bits = [0u8; (ABS_Y as usize + 1).div_ceil(8)];
        let abs_ret = unsafe {
            libc::ioctl(
                fd,
                eviocgbit_request(EV_ABS, abs_bits.len()),
                abs_bits.as_mut_ptr(),
            )
        };
        let name_lower = device_name.to_lowercase();
        let has_abs_pointer =
            has_abs && abs_ret >= 0 && test_bit(ABS_X, &abs_bits) && test_bit(ABS_Y, &abs_bits);
        let is_touchpad_like = name_lower.contains("touchpad")
            || name_lower.contains("trackpad")
            || has_pointer_buttons;

        let pointer_kind = if has_rel_pointer {
            PointerKind::Relative
        } else if has_abs_pointer && is_touchpad_like {
            PointerKind::Absolute
        } else {
            PointerKind::None
        };
        let abs_range_x = if matches!(pointer_kind, PointerKind::Absolute) {
            query_abs_range(fd, ABS_X)
        } else {
            None
        };
        let abs_range_y = if matches!(pointer_kind, PointerKind::Absolute) {
            query_abs_range(fd, ABS_Y)
        } else {
            None
        };

        let is_mouse = !matches!(pointer_kind, PointerKind::None);

        if !is_keyboard && !is_mouse {
            return Ok(None);
        }

        // Re-open non-blocking for epoll (NO GRAB)
        drop(file);
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(PhantomError::Io)?;

        Ok(Some(DeviceInfo {
            path: path.to_string(),
            name: device_name,
            file,
            is_keyboard,
            is_mouse,
            pointer_kind,
            abs_x: None,
            abs_y: None,
            abs_range_x,
            abs_range_y,
            last_abs_position: None,
            abs_dirty: false,
            pressed_keys: HashSet::new(),
            desynced: false,
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

    pub fn process_events(&mut self, raw: &[(RawFd, RawInputEvent)]) -> Vec<InputEvent> {
        let mut result = Vec::new();

        for (fd, event) in raw {
            if trace_detail_enabled() {
                tracing::trace!(
                    fd = *fd,
                    type_ = event.type_,
                    code = event.code,
                    value = event.value,
                    "raw evdev event"
                );
            }
            let Some(device) = self.device_for_fd_mut(*fd) else {
                tracing::warn!("received input event for unknown fd {}", fd);
                continue;
            };

            if event.type_ == EV_SYN {
                if event.code == SYN_DROPPED {
                    tracing::warn!(
                        "SYN_DROPPED on fd {}, dropping buffered events until next SYN_REPORT and resyncing key state",
                        fd
                    );
                    device.desynced = true;
                    continue;
                }
                if event.code == SYN_REPORT && device.desynced {
                    device.desynced = false;
                    match Self::resync_key_state(device) {
                        Ok(events) => result.extend(events),
                        Err(e) => {
                            tracing::warn!("failed to resync key state for {}: {}", device.path, e)
                        }
                    }
                }
                if event.code == SYN_REPORT {
                    if let Some(mouse_move) = Self::flush_absolute_pointer_motion(device) {
                        result.push(mouse_move);
                    }
                }
                continue;
            }

            if device.desynced {
                continue;
            }

            if event.type_ == EV_KEY {
                // Filter repeat events (value == 2)
                if event.value == 2 {
                    continue;
                }
                if let Some(key) = evdev_code_to_key(event.code) {
                    if event.value == 1 {
                        if device.pressed_keys.insert(key) {
                            if tracing::enabled!(tracing::Level::TRACE) {
                                tracing::trace!(
                                    device = %device.path,
                                    key = ?key,
                                    pressed = %format_pressed_keys(&device.pressed_keys),
                                    "translated key press"
                                );
                            }
                            result.push(InputEvent::KeyPress(key));
                        }
                    } else if event.value == 0 {
                        device.pressed_keys.remove(&key);
                        if tracing::enabled!(tracing::Level::TRACE) {
                            tracing::trace!(
                                device = %device.path,
                                key = ?key,
                                pressed = %format_pressed_keys(&device.pressed_keys),
                                "translated key release"
                            );
                        }
                        result.push(InputEvent::KeyRelease(key));
                    }
                }
            } else if event.type_ == EV_REL {
                if event.code == REL_X && matches!(device.pointer_kind, PointerKind::Relative) {
                    if trace_detail_enabled() {
                        tracing::trace!(
                            device = %device.path,
                            dx = event.value,
                            dy = 0,
                            "translated mouse move"
                        );
                    }
                    result.push(InputEvent::MouseMove {
                        dx: event.value,
                        dy: 0,
                    });
                } else if event.code == REL_Y
                    && matches!(device.pointer_kind, PointerKind::Relative)
                {
                    if trace_detail_enabled() {
                        tracing::trace!(
                            device = %device.path,
                            dx = 0,
                            dy = event.value,
                            "translated mouse move"
                        );
                    }
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
            } else if event.type_ == EV_ABS && matches!(device.pointer_kind, PointerKind::Absolute)
            {
                if event.code == ABS_X {
                    device.abs_x = Some(event.value);
                    device.abs_dirty = true;
                } else if event.code == ABS_Y {
                    device.abs_y = Some(event.value);
                    device.abs_dirty = true;
                }
            }
        }

        Self::merge_mouse_moves(&mut result);
        result
    }

    fn device_for_fd_mut(&mut self, fd: RawFd) -> Option<&mut DeviceInfo> {
        let idx = *self.fd_to_index.get(&fd)?;
        self.devices.get_mut(idx)
    }

    fn resync_key_state(device: &mut DeviceInfo) -> Result<Vec<InputEvent>> {
        let mut key_bits = vec![0u8; EVDEV_KEY_BUF_SIZE];
        let ret = unsafe {
            libc::ioctl(
                device.fd(),
                eviocgkey_request(key_bits.len()),
                key_bits.as_mut_ptr(),
            )
        };
        if ret < 0 {
            return Err(PhantomError::Io(std::io::Error::last_os_error()));
        }

        let mut pressed_now = HashSet::new();
        for code in 0..=EVDEV_KEY_MAX as u16 {
            if !test_bit(code, &key_bits) {
                continue;
            }
            if let Some(key) = evdev_code_to_key(code) {
                pressed_now.insert(key);
            }
        }

        let mut events = Vec::new();
        for key in pressed_now.difference(&device.pressed_keys) {
            events.push(InputEvent::KeyPress(*key));
        }
        for key in device.pressed_keys.difference(&pressed_now) {
            events.push(InputEvent::KeyRelease(*key));
        }
        device.pressed_keys = pressed_now;
        Ok(events)
    }

    fn merge_mouse_moves(events: &mut Vec<InputEvent>) {
        let original = std::mem::take(events);
        let mut merged = Vec::with_capacity(original.len());
        for event in original {
            match (merged.last_mut(), event) {
                (
                    Some(InputEvent::MouseMove { dx, dy }),
                    InputEvent::MouseMove {
                        dx: next_dx,
                        dy: next_dy,
                    },
                ) => {
                    *dx += next_dx;
                    *dy += next_dy;
                }
                (_, event) => merged.push(event),
            }
        }
        *events = merged;
    }

    fn flush_absolute_pointer_motion(device: &mut DeviceInfo) -> Option<InputEvent> {
        if !matches!(device.pointer_kind, PointerKind::Absolute) || !device.abs_dirty {
            return None;
        }

        device.abs_dirty = false;
        let (next_x, next_y) = match (device.abs_x, device.abs_y) {
            (Some(x), Some(y)) => (x, y),
            _ => return None,
        };

        let (prev_x, prev_y) = device.last_abs_position.replace((next_x, next_y))?;

        let dx = next_x - prev_x;
        let dy = next_y - prev_y;
        if absolute_reanchor_jump(dx, device.abs_range_x)
            || absolute_reanchor_jump(dy, device.abs_range_y)
        {
            if trace_detail_enabled() {
                tracing::trace!(
                    device = %device.path,
                    dx = dx,
                    dy = dy,
                    "ignoring touchpad re-anchor jump"
                );
            }
            return None;
        }
        if dx == 0 && dy == 0 {
            return None;
        }
        if dx.abs() <= 1 && dy.abs() <= 1 {
            return None;
        }

        if trace_detail_enabled() {
            tracing::trace!(device = %device.path, dx = dx, dy = dy, "translated touchpad move");
        }
        Some(InputEvent::MouseMove { dx, dy })
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

    pub fn mouse_grabbed(&self) -> bool {
        self.mouse_grabbed
    }

    pub fn keyboard_grabbed(&self) -> bool {
        self.keyboard_grabbed
    }

    pub fn current_pressed_mouse_keys(&self) -> HashSet<Key> {
        let mut pressed = HashSet::new();
        for device in &self.devices {
            if !device.is_mouse {
                continue;
            }
            for key in &device.pressed_keys {
                if key.is_mouse() {
                    pressed.insert(*key);
                }
            }
        }
        pressed
    }

    pub fn set_grabbed_all(&mut self, grabbed: bool) -> Result<()> {
        let previous_keyboard = self.keyboard_grabbed;
        let previous_mouse = self.mouse_grabbed;

        // Keyboard and mouse grabs are tracked independently so the daemon can
        // stay in capture while temporarily returning only the mouse to the
        // desktop. Apply them in a defined order and restore on failure.
        let result = if grabbed {
            self.set_grabbed_keyboard_only(true)
                .and_then(|_| self.set_grabbed_mouse_only(true))
        } else {
            self.set_grabbed_mouse_only(false)
                .and_then(|_| self.set_grabbed_keyboard_only(false))
        };

        if let Err(err) = result {
            if let Err(restore_err) = self.restore_grab_state(previous_keyboard, previous_mouse) {
                tracing::warn!(
                    "failed to restore grab state after error: original={}, restore={}",
                    err,
                    restore_err
                );
            }
            return Err(err);
        }
        Ok(())
    }

    /// Grab or release mouse devices. Keyboard state is unchanged.
    pub fn set_grabbed_mouse_only(&mut self, grabbed: bool) -> Result<()> {
        if self.mouse_grabbed == grabbed {
            return Ok(());
        }

        for dev in &self.devices {
            if !dev.is_mouse {
                continue;
            }
            let value = if grabbed { 1 } else { 0 };
            if let Err(err) = unsafe { eviocgrab(dev.fd(), value) } {
                return Err(PhantomError::IoctlFailed {
                    operation: "EVIOCGRAB".into(),
                    path: dev.path.clone(),
                    reason: std::io::Error::from_raw_os_error(err as i32).to_string(),
                });
            }
        }

        self.mouse_grabbed = grabbed;
        tracing::info!(
            "mouse grab {}",
            if grabbed { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Grab or release keyboard devices. Mouse state is unchanged.
    pub fn set_grabbed_keyboard_only(&mut self, grabbed: bool) -> Result<()> {
        if self.keyboard_grabbed == grabbed {
            return Ok(());
        }

        for dev in &self.devices {
            if !dev.is_keyboard {
                continue;
            }
            let value = if grabbed { 1 } else { 0 };
            if let Err(err) = unsafe { eviocgrab(dev.fd(), value) } {
                return Err(PhantomError::IoctlFailed {
                    operation: "EVIOCGRAB".into(),
                    path: dev.path.clone(),
                    reason: std::io::Error::from_raw_os_error(err as i32).to_string(),
                });
            }
        }

        self.keyboard_grabbed = grabbed;
        tracing::info!(
            "keyboard grab {}",
            if grabbed { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Release all grabs unconditionally. Used during shutdown.
    pub fn force_release_all(&mut self) {
        for dev in &mut self.devices {
            unsafe {
                let _ = eviocgrab(dev.fd(), 0);
            }
            dev.pressed_keys.clear();
            dev.desynced = false;
        }
        self.mouse_grabbed = false;
        self.keyboard_grabbed = false;
    }

    fn restore_grab_state(&mut self, keyboard: bool, mouse: bool) -> Result<()> {
        if keyboard {
            self.set_grabbed_keyboard_only(true)?;
        } else {
            self.set_grabbed_keyboard_only(false)?;
        }

        if mouse {
            self.set_grabbed_mouse_only(true)?;
        } else {
            self.set_grabbed_mouse_only(false)?;
        }

        Ok(())
    }
}

impl Drop for InputCapture {
    fn drop(&mut self) {
        for dev in &mut self.devices {
            unsafe {
                let _ = eviocgrab(dev.fd(), 0);
            }
            dev.pressed_keys.clear();
            dev.desynced = false;
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

fn format_pressed_keys(keys: &HashSet<Key>) -> String {
    let mut rendered: Vec<String> = keys.iter().map(|key| format!("{:?}", key)).collect();
    rendered.sort();
    rendered.join(",")
}

fn query_abs_range(fd: RawFd, axis: u16) -> Option<(i32, i32)> {
    let mut info = InputAbsInfo {
        value: 0,
        minimum: 0,
        maximum: 0,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    };
    let ret = unsafe { libc::ioctl(fd, eviocgabs_request(axis), &mut info as *mut InputAbsInfo) };
    if ret < 0 {
        return None;
    }
    Some((info.minimum, info.maximum))
}

fn absolute_reanchor_jump(delta: i32, range: Option<(i32, i32)>) -> bool {
    let span = range
        .map(|(min, max)| (max - min).unsigned_abs() as i32)
        .unwrap_or_default();
    let threshold = ((span as f64) * 0.12).round() as i32;
    let threshold = threshold.clamp(64, 1024);
    delta.abs() >= threshold
}

fn eviocgname_request(len: usize) -> libc::c_ulong {
    nix::request_code_read!(b'E', 0x06, len) as libc::c_ulong
}

fn eviocgbit_request(ev: u16, len: usize) -> libc::c_ulong {
    nix::request_code_read!(b'E', 0x20 + ev as u8, len) as libc::c_ulong
}

fn eviocgkey_request(len: usize) -> libc::c_ulong {
    nix::request_code_read!(b'E', 0x18, len) as libc::c_ulong
}

fn eviocgabs_request(axis: u16) -> libc::c_ulong {
    nix::request_code_read!(b'E', 0x40 + axis as u8, std::mem::size_of::<InputAbsInfo>())
        as libc::c_ulong
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;

    #[test]
    fn ioctl_requests_match_kernel_headers_on_x86_64() {
        assert_eq!(eviocgname_request(256), 0x8100_4506);
        assert_eq!(eviocgbit_request(0, 4), 0x8004_4520);
        assert_eq!(eviocgkey_request(EVDEV_KEY_BUF_SIZE), 0x8060_4518);
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

    #[test]
    fn input_origin_helpers_distinguish_mouse_and_keyboard() {
        assert!(Key::MouseLeft.is_mouse());
        assert!(!Key::A.is_mouse());
        assert!(InputEvent::MouseMove { dx: 1, dy: 1 }.is_mouse_input());
        assert!(InputEvent::KeyPress(Key::MouseRight).is_mouse_input());
        assert!(InputEvent::KeyPress(Key::A).is_keyboard_input());
        assert!(!InputEvent::KeyRelease(Key::MouseLeft).is_keyboard_input());
    }

    #[test]
    fn absolute_pointer_devices_emit_mouse_move_on_syn_report() {
        let file = File::open("/dev/null").unwrap();
        let fd = file.as_raw_fd();
        let mut capture = InputCapture {
            devices: vec![DeviceInfo {
                path: "/dev/null".into(),
                name: "Test Touchpad".into(),
                file,
                is_keyboard: false,
                is_mouse: true,
                pointer_kind: PointerKind::Absolute,
                abs_x: None,
                abs_y: None,
                abs_range_x: Some((0, 1000)),
                abs_range_y: Some((0, 1000)),
                last_abs_position: None,
                abs_dirty: false,
                pressed_keys: HashSet::new(),
                desynced: false,
            }],
            fd_to_index: HashMap::from([(fd, 0)]),
            epoll_fd: -1,
            mouse_grabbed: false,
            keyboard_grabbed: false,
        };

        let first = capture.process_events(&[
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_ABS,
                    code: ABS_X,
                    value: 100,
                },
            ),
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_ABS,
                    code: ABS_Y,
                    value: 200,
                },
            ),
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_SYN,
                    code: SYN_REPORT,
                    value: 0,
                },
            ),
        ]);
        assert!(first.is_empty());

        let second = capture.process_events(&[
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_ABS,
                    code: ABS_X,
                    value: 112,
                },
            ),
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_ABS,
                    code: ABS_Y,
                    value: 206,
                },
            ),
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_SYN,
                    code: SYN_REPORT,
                    value: 0,
                },
            ),
        ]);

        assert_eq!(second.len(), 1);
        assert!(matches!(second[0], InputEvent::MouseMove { dx: 12, dy: 6 }));
    }

    #[test]
    fn absolute_pointer_ignores_large_reanchor_jumps() {
        let file = File::open("/dev/null").unwrap();
        let fd = file.as_raw_fd();
        let mut capture = InputCapture {
            devices: vec![DeviceInfo {
                path: "/dev/null".into(),
                name: "Test Touchpad".into(),
                file,
                is_keyboard: false,
                is_mouse: true,
                pointer_kind: PointerKind::Absolute,
                abs_x: Some(100),
                abs_y: Some(100),
                abs_range_x: Some((0, 1000)),
                abs_range_y: Some((0, 1000)),
                last_abs_position: Some((100, 100)),
                abs_dirty: true,
                pressed_keys: HashSet::new(),
                desynced: false,
            }],
            fd_to_index: HashMap::from([(fd, 0)]),
            epoll_fd: -1,
            mouse_grabbed: false,
            keyboard_grabbed: false,
        };

        let events = capture.process_events(&[
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_ABS,
                    code: ABS_X,
                    value: 800,
                },
            ),
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_ABS,
                    code: ABS_Y,
                    value: 850,
                },
            ),
            (
                fd,
                RawInputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: EV_SYN,
                    code: SYN_REPORT,
                    value: 0,
                },
            ),
        ]);

        assert!(events.is_empty());
    }
}

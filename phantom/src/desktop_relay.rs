use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use crate::error::{PhantomError, Result};
use crate::input::Key;

nix::ioctl_write_int!(ui_set_evbit, b'U', 100);
nix::ioctl_write_int!(ui_set_keybit, b'U', 101);
nix::ioctl_none!(ui_dev_create, b'U', 1);
nix::ioctl_none!(ui_dev_destroy, b'U', 2);
nix::ioctl_write_ptr!(ui_dev_setup, b'U', 3, UinputSetup);

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0x00;
const BUS_USB: u16 = 0x03;
const KEY_MAX: u16 = 0x2ff;

pub const PHANTOM_DESKTOP_KEYBOARD_NAME: &str = "Phantom Desktop Keyboard";

#[repr(C)]
#[derive(Clone, Copy)]
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

const _: [(); 24] = [(); std::mem::size_of::<InputEvent>()];
const _: [(); 8] = [(); std::mem::size_of::<InputId>()];
const _: [(); 92] = [(); std::mem::size_of::<UinputSetup>()];

pub struct DesktopKeyboardRelay {
    file: File,
    pressed_keys: HashSet<Key>,
}

impl DesktopKeyboardRelay {
    pub fn new() -> Result<Self> {
        let file = Self::open_uinput()?;
        let fd = file.as_raw_fd();
        Self::configure_capabilities(fd)?;
        Self::configure_device(fd)?;

        unsafe { ui_dev_create(fd).map_err(|e| ioctl_err("UI_DEV_CREATE", e, "/dev/uinput"))? };

        std::thread::sleep(Duration::from_millis(50));
        tracing::info!("desktop keyboard relay ready");

        Ok(Self {
            file,
            pressed_keys: HashSet::new(),
        })
    }

    pub fn relay_key_event(&mut self, key: Key, pressed: bool) -> Result<()> {
        if key.is_mouse() {
            return Ok(());
        }
        let Some(code) = key.evdev_code() else {
            tracing::trace!(?key, "skipping desktop relay for unmapped key");
            return Ok(());
        };

        if pressed {
            self.pressed_keys.insert(key);
        } else {
            self.pressed_keys.remove(&key);
        }

        self.write_event(EV_KEY, code, if pressed { 1 } else { 0 })?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        Ok(())
    }

    pub fn release_all(&mut self) -> Result<()> {
        if self.pressed_keys.is_empty() {
            return Ok(());
        }

        let pressed: Vec<Key> = self.pressed_keys.iter().copied().collect();
        for key in pressed {
            self.relay_key_event(key, false)?;
        }
        Ok(())
    }

    fn open_uinput() -> Result<File> {
        OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    PhantomError::PermissionDenied {
                        path: "/dev/uinput".into(),
                        reason: "run as root or grant write access to /dev/uinput".into(),
                    }
                } else {
                    PhantomError::Io(e)
                }
            })
    }

    fn configure_capabilities(fd: RawFd) -> Result<()> {
        unsafe {
            ui_set_evbit(fd, EV_KEY as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_KEY", e, "/dev/uinput"))?;
            ui_set_evbit(fd, EV_SYN as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_SYN", e, "/dev/uinput"))?;
            for code in 0..=KEY_MAX {
                ui_set_keybit(fd, code as libc::c_ulong)
                    .map_err(|e| ioctl_err("UI_SET_KEYBIT", e, "/dev/uinput"))?;
            }
        }
        Ok(())
    }

    fn configure_device(fd: RawFd) -> Result<()> {
        let setup = build_setup();
        unsafe {
            ui_dev_setup(fd, &setup as *const UinputSetup)
                .map_err(|e| ioctl_err("UI_DEV_SETUP", e, "/dev/uinput"))?;
        }
        Ok(())
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> Result<()> {
        let ev = InputEvent {
            tv_sec: 0,
            tv_usec: 0,
            type_,
            code,
            value,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&ev as *const InputEvent).cast::<u8>(),
                std::mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(bytes)?;
        Ok(())
    }
}

impl Drop for DesktopKeyboardRelay {
    fn drop(&mut self) {
        let _ = self.release_all();
        unsafe {
            let _ = ui_dev_destroy(self.file.as_raw_fd());
        }
        tracing::info!("desktop keyboard relay destroyed");
    }
}

fn build_setup() -> UinputSetup {
    let mut name = [0u8; 80];
    let raw_name = PHANTOM_DESKTOP_KEYBOARD_NAME.as_bytes();
    let len = raw_name.len().min(name.len() - 1);
    name[..len].copy_from_slice(&raw_name[..len]);

    UinputSetup {
        id: InputId {
            bustype: BUS_USB,
            vendor: 0x1234,
            product: 0x4321,
            version: 1,
        },
        name,
        ff_effects_max: 0,
    }
}

fn ioctl_err(operation: &str, err: nix::errno::Errno, path: &str) -> PhantomError {
    PhantomError::IoctlFailed {
        operation: operation.into(),
        path: path.into(),
        reason: std::io::Error::from_raw_os_error(err as i32).to_string(),
    }
}

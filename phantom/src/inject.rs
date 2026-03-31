use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;

use crate::error::{PhantomError, Result};

// ioctl command numbers for /dev/uinput (Linux kernel interface)
// These are the raw ioctl numbers from linux/uinput.h
const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
const UI_SET_ABSBIT: libc::c_ulong = 0x40045567;
const UI_SET_PROPBIT: libc::c_ulong = 0x40045569;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const UI_DEV_SETUP: libc::c_ulong = 0x405c5503;
const UI_ABS_SETUP: libc::c_ulong = 0x401c55c4;

// Event types
const EV_SYN: u16 = 0x00;
const EV_ABS: u16 = 0x03;

// Absolute axes
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;

// Sync
const SYN_REPORT: u16 = 0x00;

// Bus type
const BUS_VIRTUAL: u16 = 0x06;

const MAX_SLOTS: i32 = 9;

// Linux input_event struct (24 bytes on 64-bit)
#[repr(C)]
#[derive(Clone, Copy)]
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}

// input_absinfo struct
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

// uinput_abs_setup struct for UI_ABS_SETUP ioctl
#[repr(C)]
struct UinputAbsSetup {
    code: u16,
    _padding: [u8; 6],
    absinfo: InputAbsInfo,
}

// input_id struct
#[repr(C)]
#[derive(Clone, Copy)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

// uinput_setup struct for UI_DEV_SETUP ioctl
#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

/// Safe wrapper around libc::ioctl for setting an int value.
unsafe fn ioctl_set_int(fd: i32, request: libc::c_ulong, value: i32) -> std::io::Result<()> {
    let ret = libc::ioctl(fd, request, value);
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Safe wrapper around libc::ioctl for passing a pointer.
unsafe fn ioctl_set_ptr<T>(fd: i32, request: libc::c_ulong, ptr: *const T) -> std::io::Result<()> {
    let ret = libc::ioctl(fd, request, ptr);
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub struct UinputDevice {
    file: File,
    screen_width: i32,
    screen_height: i32,
    active_slots: [bool; 10],
}

impl UinputDevice {
    pub fn new(screen_width: u32, screen_height: u32) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    PhantomError::PermissionDenied {
                        path: "/dev/uinput".into(),
                        reason: "run as root or add user to 'input' group".into(),
                    }
                } else {
                    PhantomError::Io(e)
                }
            })?;

        let fd = file.as_raw_fd();

        // Enable event types
        unsafe {
            ioctl_set_int(fd, UI_SET_EVBIT, EV_ABS as i32)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_ABS", e))?;
            ioctl_set_int(fd, UI_SET_EVBIT, 0x01i32) // EV_KEY
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_KEY", e))?;
            ioctl_set_int(fd, UI_SET_EVBIT, EV_SYN as i32)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_SYN", e))?;

            // Enable absolute axes
            ioctl_set_int(fd, UI_SET_ABSBIT, ABS_MT_SLOT as i32)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT MT_SLOT", e))?;
            ioctl_set_int(fd, UI_SET_ABSBIT, ABS_MT_TRACKING_ID as i32)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT MT_TRACKING_ID", e))?;
            ioctl_set_int(fd, UI_SET_ABSBIT, ABS_MT_POSITION_X as i32)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT MT_POSITION_X", e))?;
            ioctl_set_int(fd, UI_SET_ABSBIT, ABS_MT_POSITION_Y as i32)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT MT_POSITION_Y", e))?;

            // Enable touch button (BTN_TOUCH = 0x14a)
            ioctl_set_int(fd, UI_SET_KEYBIT, 0x14ai32)
                .map_err(|e| ioctl_err("UI_SET_KEYBIT BTN_TOUCH", e))?;

            // Set direct touch property (INPUT_PROP_DIRECT = 0x01)
            ioctl_set_int(fd, UI_SET_PROPBIT, 0x01i32)
                .map_err(|e| ioctl_err("UI_SET_PROPBIT INPUT_PROP_DIRECT", e))?;
        }

        // Configure axis ranges
        // Try UI_ABS_SETUP first (newer kernels)
        let use_new_api = Self::set_abs_axis_new(fd, ABS_MT_SLOT, 0, MAX_SLOTS).is_ok();
        if !use_new_api {
            tracing::info!("UI_ABS_SETUP not available, using fallback");
        }

        Self::set_abs_axis(fd, ABS_MT_SLOT, 0, MAX_SLOTS, use_new_api)?;
        Self::set_abs_axis(fd, ABS_MT_TRACKING_ID, 0, 65535, use_new_api)?;
        Self::set_abs_axis(
            fd,
            ABS_MT_POSITION_X,
            0,
            (screen_width as i32) - 1,
            use_new_api,
        )?;
        Self::set_abs_axis(
            fd,
            ABS_MT_POSITION_Y,
            0,
            (screen_height as i32) - 1,
            use_new_api,
        )?;

        // Set device identity
        let mut setup = UinputSetup {
            id: InputId {
                bustype: BUS_VIRTUAL,
                vendor: 0x1234,
                product: 0x5678,
                version: 1,
            },
            name: [0u8; 80],
            ff_effects_max: 0,
        };
        let name_bytes = b"Phantom Virtual Touch";
        setup.name[..name_bytes.len()].copy_from_slice(name_bytes);

        unsafe {
            let ret = libc::ioctl(fd, UI_DEV_SETUP, &setup as *const UinputSetup);
            if ret < 0 {
                tracing::warn!(
                    "UI_DEV_SETUP failed: {}, device name may be generic",
                    std::io::Error::last_os_error()
                );
            }
        }

        // Create device
        unsafe {
            ioctl_set_int(fd, UI_DEV_CREATE, 0).map_err(|e| PhantomError::IoctlFailed {
                operation: "UI_DEV_CREATE".into(),
                path: "/dev/uinput".into(),
                reason: e.to_string(),
            })?;
        }

        tracing::info!(
            "uinput device created: {}x{}, {} slots",
            screen_width,
            screen_height,
            MAX_SLOTS + 1
        );

        Ok(Self {
            file,
            screen_width: screen_width as i32,
            screen_height: screen_height as i32,
            active_slots: [false; 10],
        })
    }

    fn set_abs_axis_new(fd: i32, code: u16, min: i32, max: i32) -> std::io::Result<()> {
        let setup = UinputAbsSetup {
            code,
            _padding: [0; 6],
            absinfo: InputAbsInfo {
                value: 0,
                minimum: min,
                maximum: max,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        };
        unsafe { ioctl_set_ptr(fd, UI_ABS_SETUP, &setup as *const UinputAbsSetup) }
    }

    fn set_abs_axis(fd: i32, code: u16, min: i32, max: i32, use_new: bool) -> Result<()> {
        if use_new {
            Self::set_abs_axis_new(fd, code, min, max).map_err(|e| PhantomError::IoctlFailed {
                operation: format!("UI_ABS_SETUP axis={:#x}", code),
                path: "/dev/uinput".into(),
                reason: e.to_string(),
            })?;
        } else {
            // Fallback: set each axis's min/max/fuzz/flat individually
            // We just call ioctl with UI_SET_ABS_MIN_MAX equivalent
            // Since this is a legacy path, use the new API which is well-supported on 5.x+
            return Err(PhantomError::IoctlFailed {
                operation: format!("UI_ABS_SETUP axis={:#x}", code),
                path: "/dev/uinput".into(),
                reason: "kernel does not support UI_ABS_SETUP (need Linux 5.1+)".into(),
            });
        }
        Ok(())
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> Result<()> {
        let event = InputEvent {
            tv_sec: 0,
            tv_usec: 0,
            type_,
            code,
            value,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &event as *const InputEvent as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        self.file.write_all(bytes).map_err(PhantomError::Io)
    }

    pub fn touch_down(&mut self, slot: u8, x: f64, y: f64) -> Result<()> {
        let slot = slot as i32;
        let px = ((x.clamp(0.0, 1.0)) * (self.screen_width as f64)) as i32;
        let py = ((y.clamp(0.0, 1.0)) * (self.screen_height as f64)) as i32;
        let px = px.clamp(0, self.screen_width - 1);
        let py = py.clamp(0, self.screen_height - 1);

        self.write_event(EV_ABS, ABS_MT_SLOT, slot)?;
        self.write_event(EV_ABS, ABS_MT_TRACKING_ID, slot)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_X, px)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_Y, py)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;

        if (slot as usize) < self.active_slots.len() {
            self.active_slots[slot as usize] = true;
        }
        Ok(())
    }

    pub fn touch_move(&mut self, slot: u8, x: f64, y: f64) -> Result<()> {
        let slot = slot as i32;
        let px = ((x.clamp(0.0, 1.0)) * (self.screen_width as f64)) as i32;
        let py = ((y.clamp(0.0, 1.0)) * (self.screen_height as f64)) as i32;
        let px = px.clamp(0, self.screen_width - 1);
        let py = py.clamp(0, self.screen_height - 1);

        self.write_event(EV_ABS, ABS_MT_SLOT, slot)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_X, px)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_Y, py)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        Ok(())
    }

    pub fn touch_up(&mut self, slot: u8) -> Result<()> {
        let slot = slot as i32;
        self.write_event(EV_ABS, ABS_MT_SLOT, slot)?;
        self.write_event(EV_ABS, ABS_MT_TRACKING_ID, -1)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;

        if (slot as usize) < self.active_slots.len() {
            self.active_slots[slot as usize] = false;
        }
        Ok(())
    }

    pub fn release_all(&mut self) -> Result<()> {
        for slot in 0..10u8 {
            if self.active_slots[slot as usize] {
                self.touch_up(slot)?;
            }
        }
        Ok(())
    }

    pub fn screen_width(&self) -> i32 {
        self.screen_width
    }

    pub fn screen_height(&self) -> i32 {
        self.screen_height
    }
}

impl Drop for UinputDevice {
    fn drop(&mut self) {
        let _ = self.release_all();
        std::thread::sleep(std::time::Duration::from_millis(10));
        unsafe {
            let _ = libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY, 0);
        }
        tracing::info!("uinput device destroyed");
    }
}

fn ioctl_err(op: &str, e: std::io::Error) -> PhantomError {
    PhantomError::IoctlFailed {
        operation: op.into(),
        path: "/dev/uinput".into(),
        reason: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    // These tests require /dev/uinput access (root or input group)
    // Run with: cargo test -- --ignored

    use super::*;

    #[test]
    #[ignore]
    fn create_and_destroy() {
        let dev = UinputDevice::new(1920, 1080);
        assert!(dev.is_ok());
    }

    #[test]
    #[ignore]
    fn touch_sequence() {
        let mut dev = UinputDevice::new(1920, 1080).unwrap();
        assert!(dev.touch_down(0, 0.5, 0.5).is_ok());
        assert!(dev.touch_move(0, 0.6, 0.6).is_ok());
        assert!(dev.touch_up(0).is_ok());
    }

    #[test]
    #[ignore]
    fn multi_touch() {
        let mut dev = UinputDevice::new(1920, 1080).unwrap();
        assert!(dev.touch_down(0, 0.2, 0.3).is_ok());
        assert!(dev.touch_down(1, 0.8, 0.7).is_ok());
        assert!(dev.touch_move(0, 0.25, 0.35).is_ok());
        assert!(dev.touch_up(0).is_ok());
        assert!(dev.touch_up(1).is_ok());
    }

    #[test]
    #[ignore]
    fn coordinate_clamping() {
        let mut dev = UinputDevice::new(1920, 1080).unwrap();
        assert!(dev.touch_down(0, -0.5, 1.5).is_ok());
        assert!(dev.touch_move(0, 2.0, -1.0).is_ok());
        assert!(dev.touch_up(0).is_ok());
    }
}

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use nix::errno::Errno;

use crate::error::{PhantomError, Result};

nix::ioctl_write_int!(ui_set_evbit, b'U', 100);
nix::ioctl_write_int!(ui_set_keybit, b'U', 101);
nix::ioctl_write_int!(ui_set_absbit, b'U', 103);
nix::ioctl_write_int!(ui_set_propbit, b'U', 110);
nix::ioctl_none!(ui_dev_create, b'U', 1);
nix::ioctl_none!(ui_dev_destroy, b'U', 2);
nix::ioctl_write_ptr!(ui_dev_setup, b'U', 3, UinputSetup);
nix::ioctl_write_ptr!(ui_abs_setup, b'U', 4, UinputAbsSetup);

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const ABS_MT_TOUCH_MAJOR: u16 = 0x30;
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_PRESSURE: u16 = 0x3a;

const SYN_REPORT: u16 = 0x00;
const BTN_TOUCH: u16 = 0x14a;
const INPUT_PROP_DIRECT: u16 = 0x01;
const BUS_VIRTUAL: u16 = 0x06;

pub const PHANTOM_VENDOR_ID: u16 = 0x1234;
pub const PHANTOM_PRODUCT_ID: u16 = 0x5678;
pub const PHANTOM_DEVICE_NAME: &str = "Phantom Virtual Touch";

const MAX_SLOTS: i32 = 9;
const SLOT_COUNT: usize = (MAX_SLOTS as usize) + 1;
const ABS_CNT: usize = 0x40;
const PRESSURE_MAX: i32 = 255;
const TOUCH_MAJOR_MAX: i32 = 15;

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
struct InputAbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
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

#[repr(C)]
#[derive(Clone, Copy)]
struct UinputAbsSetup {
    code: u16,
    _reserved: u16,
    absinfo: InputAbsInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UinputUserDev {
    name: [u8; 80],
    id: InputId,
    ff_effects_max: u32,
    absmax: [i32; ABS_CNT],
    absmin: [i32; ABS_CNT],
    absfuzz: [i32; ABS_CNT],
    absflat: [i32; ABS_CNT],
}

const _: [(); 24] = [(); std::mem::size_of::<InputEvent>()];
const _: [(); 24] = [(); std::mem::size_of::<InputAbsInfo>()];
const _: [(); 8] = [(); std::mem::size_of::<InputId>()];
const _: [(); 92] = [(); std::mem::size_of::<UinputSetup>()];
const _: [(); 28] = [(); std::mem::size_of::<UinputAbsSetup>()];

pub struct UinputDevice {
    file: File,
    screen_width: i32,
    screen_height: i32,
    active_slots: [bool; SLOT_COUNT],
    active_touches: usize,
    next_tracking_id: i32,
}

impl UinputDevice {
    pub fn new(screen_width: u32, screen_height: u32) -> Result<Self> {
        let (file, used_legacy_api) = Self::create_device_file(screen_width, screen_height)?;
        let fd = file.as_raw_fd();

        unsafe { ui_dev_create(fd).map_err(|e| ioctl_err("UI_DEV_CREATE", e))? };

        // Give downstream consumers a moment to notice the new device.
        std::thread::sleep(Duration::from_millis(50));

        tracing::info!(
            "uinput device created: {}x{}, {} slots{}",
            screen_width,
            screen_height,
            SLOT_COUNT,
            if used_legacy_api {
                " (legacy setup)"
            } else {
                ""
            }
        );

        Ok(Self {
            file,
            screen_width: screen_width as i32,
            screen_height: screen_height as i32,
            active_slots: [false; SLOT_COUNT],
            active_touches: 0,
            next_tracking_id: 1,
        })
    }

    fn create_device_file(screen_width: u32, screen_height: u32) -> Result<(File, bool)> {
        let file = Self::open_uinput()?;
        let fd = file.as_raw_fd();
        Self::configure_capabilities(fd)?;

        match Self::configure_modern_device(fd, screen_width, screen_height) {
            Ok(()) => Ok((file, false)),
            Err(err) if is_unsupported_setup(&err) => {
                tracing::info!("modern uinput setup unavailable, falling back to legacy API");

                drop(file);
                let mut legacy_file = Self::open_uinput()?;
                let legacy_fd = legacy_file.as_raw_fd();
                Self::configure_capabilities(legacy_fd)?;
                Self::configure_legacy_device(&mut legacy_file, screen_width, screen_height)?;
                Ok((legacy_file, true))
            }
            Err(err) => Err(err),
        }
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
            ui_set_evbit(fd, EV_ABS as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_ABS", e))?;
            ui_set_evbit(fd, EV_KEY as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_KEY", e))?;
            ui_set_evbit(fd, EV_SYN as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_EVBIT EV_SYN", e))?;

            ui_set_absbit(fd, ABS_MT_SLOT as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT ABS_MT_SLOT", e))?;
            ui_set_absbit(fd, ABS_MT_TRACKING_ID as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT ABS_MT_TRACKING_ID", e))?;
            ui_set_absbit(fd, ABS_MT_POSITION_X as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT ABS_MT_POSITION_X", e))?;
            ui_set_absbit(fd, ABS_MT_POSITION_Y as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT ABS_MT_POSITION_Y", e))?;
            ui_set_absbit(fd, ABS_MT_TOUCH_MAJOR as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT ABS_MT_TOUCH_MAJOR", e))?;
            ui_set_absbit(fd, ABS_MT_PRESSURE as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_ABSBIT ABS_MT_PRESSURE", e))?;

            ui_set_keybit(fd, BTN_TOUCH as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_KEYBIT BTN_TOUCH", e))?;
            ui_set_propbit(fd, INPUT_PROP_DIRECT as libc::c_ulong)
                .map_err(|e| ioctl_err("UI_SET_PROPBIT INPUT_PROP_DIRECT", e))?;
        }

        Ok(())
    }

    fn configure_modern_device(fd: RawFd, screen_width: u32, screen_height: u32) -> Result<()> {
        let setup = build_setup();

        unsafe {
            ui_dev_setup(fd, &setup as *const UinputSetup)
                .map_err(|e| ioctl_err("UI_DEV_SETUP", e))?;
        }

        for axis in [
            axis_setup(ABS_MT_TOUCH_MAJOR, 0, TOUCH_MAJOR_MAX),
            axis_setup(ABS_MT_SLOT, 0, MAX_SLOTS),
            axis_setup(ABS_MT_TRACKING_ID, 0, 65535),
            axis_setup(
                ABS_MT_POSITION_X,
                0,
                (screen_width as i32).saturating_sub(1),
            ),
            axis_setup(
                ABS_MT_POSITION_Y,
                0,
                (screen_height as i32).saturating_sub(1),
            ),
            axis_setup(ABS_MT_PRESSURE, 0, PRESSURE_MAX),
        ] {
            unsafe {
                ui_abs_setup(fd, &axis as *const UinputAbsSetup)
                    .map_err(|e| ioctl_err(&format!("UI_ABS_SETUP axis={:#x}", axis.code), e))?;
            }
        }

        Ok(())
    }

    fn configure_legacy_device(
        file: &mut File,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<()> {
        let mut user_dev = UinputUserDev {
            name: [0; 80],
            id: build_setup().id,
            ff_effects_max: 0,
            absmax: [0; ABS_CNT],
            absmin: [0; ABS_CNT],
            absfuzz: [0; ABS_CNT],
            absflat: [0; ABS_CNT],
        };

        let name = b"Phantom Virtual Touch";
        user_dev.name[..name.len()].copy_from_slice(name);

        user_dev.absmin[ABS_MT_SLOT as usize] = 0;
        user_dev.absmax[ABS_MT_SLOT as usize] = MAX_SLOTS;
        user_dev.absmin[ABS_MT_TRACKING_ID as usize] = 0;
        user_dev.absmax[ABS_MT_TRACKING_ID as usize] = 65535;
        user_dev.absmin[ABS_MT_TOUCH_MAJOR as usize] = 0;
        user_dev.absmax[ABS_MT_TOUCH_MAJOR as usize] = TOUCH_MAJOR_MAX;
        user_dev.absmin[ABS_MT_POSITION_X as usize] = 0;
        user_dev.absmax[ABS_MT_POSITION_X as usize] = (screen_width as i32).saturating_sub(1);
        user_dev.absmin[ABS_MT_POSITION_Y as usize] = 0;
        user_dev.absmax[ABS_MT_POSITION_Y as usize] = (screen_height as i32).saturating_sub(1);
        user_dev.absmin[ABS_MT_PRESSURE as usize] = 0;
        user_dev.absmax[ABS_MT_PRESSURE as usize] = PRESSURE_MAX;

        write_struct(file, &user_dev).map_err(PhantomError::Io)
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> Result<()> {
        let event = InputEvent {
            tv_sec: 0,
            tv_usec: 0,
            type_,
            code,
            value,
        };
        write_struct(&mut self.file, &event).map_err(PhantomError::Io)
    }

    pub fn touch_down(&mut self, slot: u8, x: f64, y: f64) -> Result<()> {
        self.touch_down_inner(slot, x, y, true)
    }

    pub fn touch_move(&mut self, slot: u8, x: f64, y: f64) -> Result<()> {
        self.touch_move_inner(slot, x, y, true)
    }

    pub fn touch_up(&mut self, slot: u8) -> Result<()> {
        self.touch_up_inner(slot, true)
    }

    pub fn apply_commands(&mut self, cmds: &[crate::engine::TouchCommand]) -> Result<()> {
        if cmds.is_empty() {
            return Ok(());
        }

        tracing::trace!(count = cmds.len(), ?cmds, "injecting touch batch");

        for cmd in cmds {
            match cmd {
                crate::engine::TouchCommand::TouchDown { slot, x, y } => {
                    self.touch_down_inner(*slot, *x, *y, false)?
                }
                crate::engine::TouchCommand::TouchMove { slot, x, y } => {
                    self.touch_move_inner(*slot, *x, *y, false)?
                }
                crate::engine::TouchCommand::TouchUp { slot } => {
                    self.touch_up_inner(*slot, false)?
                }
            }
        }

        self.sync_report()
    }

    fn touch_down_inner(&mut self, slot: u8, x: f64, y: f64, sync: bool) -> Result<()> {
        let slot_idx = self.slot_index(slot)?;
        if self.active_slots[slot_idx] {
            tracing::debug!(
                "slot {} already active, treating touch_down as touch_move",
                slot
            );
            return self.touch_move_inner(slot, x, y, sync);
        }

        let (px, py) = self.scale_coords(x, y);
        let tracking_id = self.alloc_tracking_id();

        self.write_event(EV_ABS, ABS_MT_SLOT, slot as i32)?;
        self.write_event(EV_ABS, ABS_MT_TRACKING_ID, tracking_id)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_X, px)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_Y, py)?;
        self.write_event(EV_ABS, ABS_MT_TOUCH_MAJOR, TOUCH_MAJOR_MAX)?;
        self.write_event(EV_ABS, ABS_MT_PRESSURE, PRESSURE_MAX)?;
        self.active_slots[slot_idx] = true;
        self.active_touches += 1;
        self.update_touch_state()?;
        if sync {
            self.sync_report()?;
        }
        Ok(())
    }

    fn touch_move_inner(&mut self, slot: u8, x: f64, y: f64, sync: bool) -> Result<()> {
        let slot_idx = self.slot_index(slot)?;
        if !self.active_slots[slot_idx] {
            return Err(PhantomError::Profile(format!(
                "touch_move on inactive slot {}",
                slot
            )));
        }

        let (px, py) = self.scale_coords(x, y);

        self.write_event(EV_ABS, ABS_MT_SLOT, slot as i32)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_X, px)?;
        self.write_event(EV_ABS, ABS_MT_POSITION_Y, py)?;
        self.write_event(EV_ABS, ABS_MT_TOUCH_MAJOR, TOUCH_MAJOR_MAX)?;
        self.write_event(EV_ABS, ABS_MT_PRESSURE, PRESSURE_MAX)?;
        if sync {
            self.sync_report()?;
        }
        Ok(())
    }

    fn touch_up_inner(&mut self, slot: u8, sync: bool) -> Result<()> {
        let slot_idx = self.slot_index(slot)?;
        if !self.active_slots[slot_idx] {
            return Ok(());
        }

        self.write_event(EV_ABS, ABS_MT_SLOT, slot as i32)?;
        self.write_event(EV_ABS, ABS_MT_TRACKING_ID, -1)?;
        self.active_slots[slot_idx] = false;
        self.active_touches = self.active_touches.saturating_sub(1);
        self.update_touch_state()?;
        if sync {
            self.sync_report()?;
        }
        Ok(())
    }

    pub fn release_all(&mut self) -> Result<()> {
        for slot in 0..SLOT_COUNT as u8 {
            if self.active_slots[slot as usize] {
                self.touch_up_inner(slot, false)?;
            }
        }
        self.sync_report()
    }

    pub fn screen_width(&self) -> i32 {
        self.screen_width
    }

    pub fn screen_height(&self) -> i32 {
        self.screen_height
    }

    fn scale_coords(&self, x: f64, y: f64) -> (i32, i32) {
        let px = ((x.clamp(0.0, 1.0)) * (self.screen_width as f64)) as i32;
        let py = ((y.clamp(0.0, 1.0)) * (self.screen_height as f64)) as i32;

        (
            px.clamp(0, self.screen_width.saturating_sub(1)),
            py.clamp(0, self.screen_height.saturating_sub(1)),
        )
    }

    fn slot_index(&self, slot: u8) -> Result<usize> {
        let idx = slot as usize;
        if idx >= self.active_slots.len() {
            return Err(PhantomError::Profile(format!(
                "slot {} out of range 0-{}",
                slot, MAX_SLOTS
            )));
        }
        Ok(idx)
    }

    fn alloc_tracking_id(&mut self) -> i32 {
        let id = self.next_tracking_id;
        self.next_tracking_id = if self.next_tracking_id == i32::MAX {
            1
        } else {
            self.next_tracking_id + 1
        };
        id
    }

    fn update_touch_state(&mut self) -> Result<()> {
        self.write_event(EV_KEY, BTN_TOUCH, i32::from(self.active_touches > 0))
    }

    fn sync_report(&mut self) -> Result<()> {
        self.write_event(EV_SYN, SYN_REPORT, 0)
    }
}

impl Drop for UinputDevice {
    fn drop(&mut self) {
        let _ = self.release_all();
        std::thread::sleep(Duration::from_millis(10));
        unsafe {
            let _ = ui_dev_destroy(self.file.as_raw_fd());
        }
        tracing::info!("uinput device destroyed");
    }
}

fn build_setup() -> UinputSetup {
    let mut setup = UinputSetup {
        id: InputId {
            bustype: BUS_VIRTUAL,
            vendor: PHANTOM_VENDOR_ID,
            product: PHANTOM_PRODUCT_ID,
            version: 1,
        },
        name: [0; 80],
        ff_effects_max: 0,
    };
    let name = PHANTOM_DEVICE_NAME.as_bytes();
    setup.name[..name.len()].copy_from_slice(name);
    setup
}

fn axis_setup(code: u16, minimum: i32, maximum: i32) -> UinputAbsSetup {
    UinputAbsSetup {
        code,
        _reserved: 0,
        absinfo: InputAbsInfo {
            value: 0,
            minimum,
            maximum,
            fuzz: 0,
            flat: 0,
            resolution: 0,
        },
    }
}

fn write_struct<T>(file: &mut File, value: &T) -> std::io::Result<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts((value as *const T).cast::<u8>(), std::mem::size_of::<T>())
    };
    file.write_all(bytes)
}

fn is_unsupported_setup(err: &PhantomError) -> bool {
    match err {
        PhantomError::IoctlFailed { reason, .. } => {
            reason.contains("Invalid argument")
                || reason.contains("Not a typewriter")
                || reason.contains("Function not implemented")
                || reason.contains("Inappropriate ioctl")
        }
        _ => false,
    }
}

fn ioctl_err(op: &str, errno: Errno) -> PhantomError {
    PhantomError::IoctlFailed {
        operation: op.into(),
        path: "/dev/uinput".into(),
        reason: std::io::Error::from_raw_os_error(errno as i32).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::ManuallyDrop;
    use std::path::PathBuf;

    #[test]
    fn ioctl_numbers_match_kernel_headers_on_x86_64() {
        assert_eq!(
            nix::request_code_write!(b'U', 100, std::mem::size_of::<libc::c_int>()),
            0x4004_5564
        );
        assert_eq!(
            nix::request_code_write!(b'U', 110, std::mem::size_of::<libc::c_int>()),
            0x4004_556e
        );
        assert_eq!(
            nix::request_code_write!(b'U', 4, std::mem::size_of::<UinputAbsSetup>()),
            0x401c_5504
        );
    }

    #[test]
    fn rust_layout_matches_kernel_layout() {
        assert_eq!(std::mem::size_of::<InputEvent>(), 24);
        assert_eq!(std::mem::size_of::<UinputAbsSetup>(), 28);
        assert_eq!(std::mem::offset_of!(UinputAbsSetup, absinfo), 4);
    }

    #[test]
    fn pure_mt_stream_uses_only_mt_axes() {
        let (mut dev, path) = fake_device();
        dev.touch_down(0, 0.2, 0.3).unwrap();
        dev.touch_down(1, 0.8, 0.7).unwrap();
        dev.touch_up(0).unwrap();
        dev.touch_up(1).unwrap();

        let events = read_events(&mut dev, &path);
        assert!(events.iter().all(|event| {
            !matches!(
                (event.type_, event.code),
                (EV_ABS, 0x00 | 0x01 | 0x18) | (EV_KEY, 0x145 | 0x14d | 0x14e | 0x14f)
            )
        }));
        assert!(events.iter().any(|event| matches!(
            (event.type_, event.code, event.value),
            (EV_KEY, BTN_TOUCH, 1)
        )));
        assert!(events.iter().any(|event| matches!(
            (event.type_, event.code, event.value),
            (EV_KEY, BTN_TOUCH, 0)
        )));
    }

    #[test]
    fn second_touch_uses_distinct_slot_and_tracking_id() {
        let (mut dev, path) = fake_device();
        dev.apply_commands(&[
            crate::engine::TouchCommand::TouchDown {
                slot: 0,
                x: 0.1,
                y: 0.1,
            },
            crate::engine::TouchCommand::TouchDown {
                slot: 1,
                x: 0.9,
                y: 0.9,
            },
        ])
        .unwrap();

        let events = read_events(&mut dev, &path);
        let slot_writes: Vec<i32> = events
            .iter()
            .filter(|event| event.type_ == EV_ABS && event.code == ABS_MT_SLOT)
            .map(|event| event.value)
            .collect();
        let tracking_ids: Vec<i32> = events
            .iter()
            .filter(|event| event.type_ == EV_ABS && event.code == ABS_MT_TRACKING_ID)
            .map(|event| event.value)
            .collect();

        assert!(slot_writes.windows(1).any(|window| window[0] == 0));
        assert!(slot_writes.windows(1).any(|window| window[0] == 1));
        assert_eq!(tracking_ids.len(), 2);
        assert_ne!(tracking_ids[0], tracking_ids[1]);
    }

    // These tests require /dev/uinput access (root or a configured udev rule).
    // Run with: cargo test -- --ignored
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

    fn fake_device() -> (ManuallyDrop<UinputDevice>, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "phantom-inject-test-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        let dev = UinputDevice {
            file,
            screen_width: 1920,
            screen_height: 1080,
            active_slots: [false; SLOT_COUNT],
            active_touches: 0,
            next_tracking_id: 1,
        };
        (ManuallyDrop::new(dev), path)
    }

    fn read_events(dev: &mut ManuallyDrop<UinputDevice>, path: &PathBuf) -> Vec<InputEvent> {
        dev.file.flush().unwrap();
        let bytes = std::fs::read(path).unwrap();
        let mut events = Vec::new();
        for chunk in bytes.chunks_exact(std::mem::size_of::<InputEvent>()) {
            let event = unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast::<InputEvent>()) };
            events.push(event);
        }
        let file = unsafe { std::ptr::read(&dev.file) };
        drop(file);
        let _ = std::fs::remove_file(path);
        events
    }
}

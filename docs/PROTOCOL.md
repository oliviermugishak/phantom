# Uinput MT Protocol — Technical Reference

This document details the exact kernel interface for creating a virtual multitouch device and injecting touch events. This is the core of Phantom — if this doesn't work, nothing else matters.

---

## Kernel Interface

Phantom talks to the Linux input subsystem through two file descriptors:
- `/dev/uinput` — device creation and teardown
- The created device's event node — event injection

Everything is done via `ioctl()` and `write()` calls. No libraries, no abstractions.

---

## Device Creation Sequence

### Step 1: Open /dev/uinput

```rust
let fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK)?;
```

`O_NONBLOCK` prevents `open()` from blocking if the device is busy. We always write complete event sequences, so non-blocking writes are fine.

### Step 2: Enable Event Types

```rust
ioctl(fd, UI_SET_EVBIT, EV_ABS)?;    // absolute positioning (touch)
ioctl(fd, UI_SET_EVBIT, EV_KEY)?;     // button state (BTN_TOUCH)
ioctl(fd, UI_SET_EVBIT, EV_SYN)?;     // synchronization events
```

### Step 3: Enable Absolute Axes

```rust
ioctl(fd, UI_SET_ABSBIT, ABS_MT_SLOT)?;
ioctl(fd, UI_SET_ABSBIT, ABS_MT_TRACKING_ID)?;
ioctl(fd, UI_SET_ABSBIT, ABS_MT_POSITION_X)?;
ioctl(fd, UI_SET_ABSBIT, ABS_MT_POSITION_Y)?;
```

### Step 4: Enable Touch Button

```rust
ioctl(fd, UI_SET_KEYBIT, BTN_TOUCH)?;
```

### Step 5: Set Input Property

```rust
ioctl(fd, UI_SET_PROPBIT, INPUT_PROP_DIRECT)?;
```

`INPUT_PROP_DIRECT` tells the kernel this is a direct touch device (finger touches the screen directly), not an indirect device like a touchpad. Android uses this to distinguish touchscreen input from trackpad input.

### Step 6: Configure Axis Ranges

Each axis needs its range configured via `UI_ABS_SETUP` ioctl:

```rust
// Slot: 0 to 9 (10 simultaneous fingers)
ioctl(fd, UI_ABS_SETUP, &uinput_abs_setup {
    code: ABS_MT_SLOT,
    absinfo: input_absinfo {
        value: 0,
        minimum: 0,
        maximum: 9,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    },
})?;

// Tracking ID: 0 to 65535 (enough for unique IDs)
ioctl(fd, UI_ABS_SETUP, &uinput_abs_setup {
    code: ABS_MT_TRACKING_ID,
    absinfo: input_absinfo {
        value: 0,
        minimum: 0,
        maximum: 65535,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    },
})?;

// Position X: 0 to screen_width - 1
ioctl(fd, UI_ABS_SETUP, &uinput_abs_setup {
    code: ABS_MT_POSITION_X,
    absinfo: input_absinfo {
        value: 0,
        minimum: 0,
        maximum: screen_width - 1,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    },
})?;

// Position Y: 0 to screen_height - 1
ioctl(fd, UI_ABS_SETUP, &uinput_abs_setup {
    code: ABS_MT_POSITION_Y,
    absinfo: input_absinfo {
        value: 0,
        minimum: 0,
        maximum: screen_height - 1,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    },
})?;
```

**Critical:** The axis maximums MUST match the actual screen resolution. If the screen is 1920x1080, X max is 1919 and Y max is 1079. Android reads these ranges and scales touch coordinates accordingly. Mismatched ranges cause touches to land at wrong positions.

### Step 7: Configure Device Identity

```rust
ioctl(fd, UI_DEV_SETUP, &uinput_setup {
    name: *b"Phantom Virtual Touch\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    id: input_id {
        bustype: BUS_VIRTUAL,
        vendor: 0x1234,
        product: 0x5678,
        version: 1,
    },
})?;
```

The name is 80 bytes, null-padded. `BUS_VIRTUAL` tells the kernel this isn't real hardware. The vendor/product IDs are arbitrary but should be stable across restarts.

### Step 8: Create Device

```rust
ioctl(fd, UI_DEV_CREATE)?;
```

After this call, a new `/dev/input/eventN` node appears. The kernel assigns the number. The device is now visible to all input consumers, including Waydroid's Android.

---

## Screen Resolution Detection

Phantom reads resolution once at startup. Priority order:

### Method 1: Framebuffer ioctl (preferred)

```rust
let fb_fd = open("/dev/fb0", O_RDONLY)?;
let mut vinfo: fb_var_screeninfo = unsafe { zeroed() };
ioctl(fb_fd, FBIOGET_VSCREENINFO, &mut vinfo)?;
let width = vinfo.xres;
let height = vinfo.yres;
close(fb_fd)?;
```

This works on most systems with DRM/KMS. `/dev/fb0` may not exist on systems using pure Wayland without fbdev emulation.

### Method 2: Sysfs fallback

```rust
let content = std::fs::read_to_string("/sys/class/graphics/fb0/virtual_size")?;
// format: "1920,1080"
let parts: Vec<&str> = content.trim().split(',').collect();
let width: u32 = parts[0].parse()?;
let height: u32 = parts[1].parse()?;
```

### Method 3: Config fallback

If neither method works, read from `~/.config/phantom/config.toml`:

```toml
[screen]
width = 1920
height = 1080
```

Log a warning that hardcoded resolution is being used.

---

## Event Injection

Events are written as `input_event` structs. Each struct is 24 bytes on 64-bit systems.

```rust
struct input_event {
    timeval: time,      // 16 bytes (seconds + microseconds)
    type_: u16,         // event type (EV_ABS, EV_SYN, etc.)
    code: u16,          // event code (ABS_MT_SLOT, etc.)
    value: i32,         // event value
}
```

**Important:** The `timeval` field can be set to zero. The kernel fills in the timestamp. This is standard practice for synthetic events.

### Finger Down

Place finger at slot 2, position (960, 540):

```rust
write(fd, &input_event { type: EV_ABS, code: ABS_MT_SLOT,        value: 2    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_TRACKING_ID, value: 2    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_X,  value: 960  })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_Y,  value: 540  })?;
write(fd, &input_event { type: EV_SYN, code: SYN_REPORT,         value: 0    })?;
```

### Finger Move

Move finger at slot 2 to (980, 530):

```rust
write(fd, &input_event { type: EV_ABS, code: ABS_MT_SLOT,        value: 2    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_X,  value: 980  })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_Y,  value: 530  })?;
write(fd, &input_event { type: EV_SYN, code: SYN_REPORT,         value: 0    })?;
```

Note: `TRACKING_ID` is NOT repeated on moves. Only set it on down/up.

### Finger Up

Lift finger at slot 2:

```rust
write(fd, &input_event { type: EV_ABS, code: ABS_MT_SLOT,        value: 2    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_TRACKING_ID, value: -1   })?;
write(fd, &input_event { type: EV_SYN, code: SYN_REPORT,         value: 0    })?;
```

### Multi-finger Example

Simultaneous touches at slot 0 and slot 3 in a single SYN_REPORT:

```rust
// Slot 0 goes down at (200, 600)
write(fd, &input_event { type: EV_ABS, code: ABS_MT_SLOT,        value: 0    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_TRACKING_ID, value: 0    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_X,  value: 200  })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_Y,  value: 600  })?;

// Slot 3 goes down at (1500, 400)
write(fd, &input_event { type: EV_ABS, code: ABS_MT_SLOT,        value: 3    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_TRACKING_ID, value: 3    })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_X,  value: 1500 })?;
write(fd, &input_event { type: EV_ABS, code: ABS_MT_POSITION_Y,  value: 400  })?;

// Flush both
write(fd, &input_event { type: EV_SYN, code: SYN_REPORT,         value: 0    })?;
```

---

## Device Teardown Sequence

On clean shutdown:

```rust
// 1. Lift all active fingers
for slot in 0..10 {
    if active_slots[slot] {
        write(fd, &input_event { type: EV_ABS, code: ABS_MT_SLOT,        value: slot as i32 })?;
        write(fd, &input_event { type: EV_ABS, code: ABS_MT_TRACKING_ID, value: -1          })?;
    }
}
write(fd, &input_event { type: EV_SYN, code: SYN_REPORT, value: 0 })?;

// 2. Small delay for kernel to process
std::thread::sleep(Duration::from_millis(10));

// 3. Destroy device
ioctl(fd, UI_DEV_DESTROY)?;

// 4. Close fd
close(fd)?;
```

---

## ioctl Reference

| ioctl | Constant | Purpose |
|---|---|---|
| `UI_SET_EVBIT` | 0x40045564 | Enable event type on new device |
| `UI_SET_KEYBIT` | 0x40045565 | Enable key/button code |
| `UI_SET_ABSBIT` | 0x40045567 | Enable absolute axis code |
| `UI_SET_PROPBIT` | 0x40045569 | Enable input property |
| `UI_ABS_SETUP` | 0x401c55c4 | Configure axis range (newer kernels) |
| `UI_DEV_SETUP` | 0x405c5503 | Set device name and ID |
| `UI_DEV_CREATE` | 0x5501 | Create the virtual device |
| `UI_DEV_DESTROY` | 0x5502 | Destroy the virtual device |

On older kernels (< 5.10) that lack `UI_ABS_SETUP`, use individual ioctls:
```
UI_SET_ABS_MIN_MAX → set min/max per axis
```

Phantom should attempt `UI_ABS_SETUP` first, fall back to `UI_SET_ABS_MIN_MAX` if it returns `ENOTTY`.

---

## Timing

There is no minimum delay required between writes. The kernel input subsystem buffers events until `SYN_REPORT`. However:

- **Do not batch unrelated touches in one SYN_REPORT** unless they truly happen simultaneously. Each node's event sequence should get its own `SYN_REPORT`.
- **Joystick move events** should be sent at a steady rate (60Hz recommended). Use a tokio interval timer, not ad-hoc writes.
- **Mouse camera events** can be sent on every mouse delta event (potentially 1000Hz+ on gaming mice). This is fine — the kernel handles it.

---

## Verification

After creating the device, verify it works:

```bash
# Find the device
cat /proc/bus/input/devices | grep -A 4 "Phantom"

# List its capabilities
evtest /dev/input/eventN

# Watch events in real time
evtest /dev/input/eventN
# Should show EV_ABS events for ABS_MT_SLOT, ABS_MT_TRACKING_ID, etc.
```

To verify Waydroid sees the events, open Android's "Pointer Location" developer option and inject a test touch.

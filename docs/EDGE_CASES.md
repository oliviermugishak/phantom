# Known Limits And Workarounds

This file is intentionally practical. It documents what still requires user care.

## Waydroid Does Not See The Touchscreen

Symptom:

- the host shows `Phantom Virtual Touch`
- Waydroid does not react

Workaround:

1. start Phantom first
2. restart the Waydroid session
3. verify with `waydroid shell getevent -lp`

Phantom cannot force Waydroid hotplug behavior from inside this repo.

## Host Desktop Stops Receiving Keyboard And Mouse

Expected behavior while capture is active.

Reason:

- Phantom uses `EVIOCGRAB` on the selected evdev devices

Workaround:

- press `F8`
- `phantom exit-capture`
- `phantom pause`
- `phantom shutdown`
- a second TTY or SSH shell

## New Input Devices Are Ignored After Startup

Current limitation.

Reason:

- Phantom scans devices only once on daemon startup

Workaround:

- restart the daemon after plugging in a new keyboard or mouse

## Wrong Touch Positions

Symptom:

- taps land offset from the visible Android buttons

Common cause:

- Phantom's touchscreen resolution does not match the Waydroid surface you are aiming at

Workaround:

- set `[screen]` in `~/.config/phantom/config.toml`
- or add a matching `screen` block to the profile
- restart the daemon

## Floating Or Dynamic Joysticks

Current limitation.

Phantom's `joystick` node is fixed-center. It works well when the game accepts a stable anchor point, but it does not detect or follow a joystick whose origin changes dynamically.

Workaround:

- use games or layouts with a fixed joystick area
- tune the center position manually

## Mouse Camera Is Not Universal Pointer Emulation

`mouse_camera` is a bounded swipe region with a short-lived synthetic finger. In the GUI this is presented as `Mouse Look`. It is good for FPS or action-camera movement, but it is not a generic cursor replacement.

Workaround:

- keep the region on the camera side of the UI
- use `tap` and `hold_tap` for buttons rather than trying to click through mouse movement

## `SYN_DROPPED` Still Drops Motion Packets

Phantom now drops buffered events until the next `SYN_REPORT` after `SYN_DROPPED` and resyncs key/button state with `EVIOCGKEY`.

What it does not yet do:

- recover lost relative mouse deltas during the overflow window

Practical effect:

- if input overflow happens, held keys and mouse buttons recover cleanly
- camera motion that happened during the overflow window is still lost

Workaround:

- press `F9`
- pause/resume the daemon
- reload the profile
- lower mouse DPI or polling rate if this happens repeatedly

## Multi-Monitor, Rotation, And Windowed Transform Issues

Current limitation.

Phantom does not currently track:

- per-monitor transforms
- rotation changes
- window scaling
- Waydroid window movement across displays

Best practice:

- use one known Waydroid surface geometry
- keep Waydroid fullscreen while tuning and using profiles

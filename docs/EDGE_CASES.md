# Known Limits

This document describes current product boundaries.

These are not normal troubleshooting cases. For common operational failures, see [TROUBLESHOOT.md](TROUBLESHOOT.md).

## 1. A Profile Has At Most 10 Touch-Bearing Nodes

Current touch slots are:

- `0..9`

That means a profile can only contain 10 independently slotted touch-bearing nodes at once.

Affected node types:

- `tap`
- `hold_tap`
- `toggle_tap`
- `joystick`
- `drag`
- `mouse_camera`
- `repeat_tap`

Implication:

- very large layouts such as full PUBG setups may need tradeoffs, layers, or future profile-model extensions

## 2. Tilt And Other Sensors Are Not Supported

Phantom injects touch, not accelerometer or gyroscope input.

That means:

- Temple Run tilt-to-collect-coins is unsupported
- any game that requires sensors and offers no touch alternative is outside the current feature set

## 3. `mouse_camera` Is Not A Desktop Cursor

`mouse_camera` is a bounded drag-based look primitive.

It is good for:

- FPS camera
- action camera
- free-look

It is not:

- a desktop pointer
- a generic Android cursor substitute

## 4. Dedicated Steering-Wheel Input Is Not First-Class Yet

Phantom currently supports:

- fixed joystick
- floating joystick
- drag gestures
- tap/hold/toggle style buttons

That covers many driving games that expose left/right buttons or drag-based steering zones.

It does not yet provide:

- a dedicated analog steering-wheel primitive

## 5. Hotplug Rescan Is Not Implemented

Current limitation:

- Phantom does not dynamically rescan newly attached keyboards or mice after startup

If devices change, restart the daemon.

## 6. Multi-Monitor And Rotation Handling Are Not Supported

Phantom assumes:

- one known fullscreen Android surface
- one stable orientation

It does not manage:

- compositor transforms
- monitor rotation transforms
- multi-display coordinate remapping

## 7. The GUI Reads The User Profile Library, Not The Repo Directory

The GUI loads:

- `~/.config/phantom/profiles/`

It does not live-read:

- `./profiles/`

That is intentional because the installed user library is the operational source of truth.

Use `./install.sh` to seed new shipped profiles into the user directory.

# Known Limits

This document describes current product boundaries.

These are not normal troubleshooting cases. For common operational failures, see [TROUBLESHOOT.md](TROUBLESHOOT.md).

## 1. Runtime Touch Concurrency Is Limited To 10

Profiles may define more than 10 touch-bearing nodes.

The real limit is runtime concurrency:

- Android and the current backends support at most 10 simultaneous active touches
- Phantom now allocates physical touch slots dynamically from logical profile slots
- if gameplay would exceed 10 live touches at once, the extra activation is rejected

Affected node types:

- `tap`
- `toggle_tap`
- `joystick`
- `drag`
- `aim`
- `repeat_tap`

Implication:

- very large layouts are supported, but impossible input combinations that would require more than 10 simultaneous fingers are still outside the runtime model

## 2. Tilt And Other Sensors Are Not Supported

Phantom injects touch, not accelerometer or gyroscope input.

That means:

- Temple Run tilt-to-collect-coins is unsupported
- any game that requires sensors and offers no touch alternative is outside the current feature set

## 3. `aim` Is Not A Desktop Cursor

`aim` is a bounded drag-based look primitive.

It is good for:

- FPS camera
- action camera
- free-look

It is not:

- a desktop pointer
- a generic Android cursor substitute

For UI navigation while capture is active and the mouse is released, Phantom now provides runtime mouse-to-touch behavior separately from `aim`.

## 4. Dedicated Steering-Wheel Input Is Not First-Class Yet

Phantom currently supports:

- fixed joystick
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

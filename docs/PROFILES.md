# Profiles

Profiles are JSON files stored in:

- `~/.config/phantom/profiles/`

They describe how physical keyboard and mouse input should become Android touch behavior.

## 1. Top-Level Schema

```json
{
  "name": "PUBG Mobile",
  "version": 1,
  "screen": { "width": 1920, "height": 1080 },
  "global_sensitivity": 1.0,
  "nodes": []
}
```

Fields:

- `name`
  Human-readable profile name.
- `version`
  Must be `1`.
- `screen`
  Required screen contract for the profile.
- `global_sensitivity`
  Multiplier applied to mouse-look sensitivity.
- `nodes`
  Control definitions.

## 2. Screen Contract

The profile `screen` must match the daemon `screen`.

Why:

- Phantom does not guess transforms at runtime
- profile coordinates are normalized against this explicit surface

If the profile and daemon disagree, profile load is rejected.

## 3. Coordinates

All positions are normalized to `[0.0, 1.0]`.

```text
(0.0, 0.0) ---------------- (1.0, 0.0)
   |                             |
   |        Android surface      |
   |                             |
(0.0, 1.0) ---------------- (1.0, 1.0)
```

Runtime conversion:

```text
pixel_x = rel_x * screen_width
pixel_y = rel_y * screen_height
```

## 4. Slot Rules

Slot-bearing nodes use touch slots `0..9`.

Rules:

- slot-bearing nodes must use unique slots
- joysticks use one slot for the whole stick
- `mouse_camera` also consumes one slot

This is why `phantom audit` is so important.

## 5. Layers

Action nodes may include:

```json
"layer": "combat"
```

Rules:

- empty layer means base layer
- `layer_shift` activates named layers
- a base-layer key may not also be reused in a non-base layer
- the same key may be reused across different non-base layers

## 6. Node Types

### `tap`

```json
{
  "id": "jump",
  "type": "tap",
  "slot": 4,
  "pos": { "x": 0.92, "y": 0.82 },
  "key": "Space"
}
```

Behavior:

- key press -> `TouchDown`
- key release -> `TouchUp`

### `hold_tap`

```json
{
  "id": "fire",
  "type": "hold_tap",
  "slot": 2,
  "pos": { "x": 0.88, "y": 0.62 },
  "key": "MouseLeft"
}
```

Behavior:

- key press -> `TouchDown`
- key held -> finger remains down
- key release -> `TouchUp`

### `toggle_tap`

```json
{
  "id": "scope_toggle",
  "type": "toggle_tap",
  "slot": 3,
  "pos": { "x": 0.76, "y": 0.61 },
  "key": "Q"
}
```

Behavior:

- first press -> `TouchDown`
- second press -> `TouchUp`
- key release does nothing

### `joystick`

```json
{
  "id": "move",
  "type": "joystick",
  "slot": 0,
  "pos": { "x": 0.18, "y": 0.72 },
  "radius": 0.07,
  "keys": {
    "up": "W",
    "down": "S",
    "left": "A",
    "right": "D"
  }
}
```

Behavior:

- first direction press -> finger down at stick center, then move to the offset
- direction changes -> `TouchMove`
- all directions released -> `TouchUp`

Notes:

- diagonals are normalized
- opposing directions cancel
- joystick center is fixed

### `mouse_camera`

```json
{
  "id": "camera",
  "type": "mouse_camera",
  "slot": 1,
  "region": { "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 },
  "sensitivity": 1.2,
  "activation_mode": "while_held",
  "activation_key": "MouseRight",
  "invert_y": false
}
```

In the GUI this appears as `Mouse Look`.

Fields:

- `region`
  Bounded normalized swipe region.
- `sensitivity`
  Node-local multiplier.
- `activation_mode`
  One of:
  - `always_on`
  - `while_held`
  - `toggle`
- `activation_key`
  Required for `while_held` and `toggle`, omitted for `always_on`.
- `invert_y`
  Inverts vertical mouse input.

Behavior:

- `always_on`
  - mouse movement immediately drives the look region while capture and mouse routing are active
- `while_held`
  - movement only drives the look region while the activation key is held
- `toggle`
  - the activation key toggles look mode on and off
- when movement stops, the synthetic finger is released shortly afterward
- disabling look mode emits an immediate `TouchUp`

Important:

- `mouse_camera` is camera/look emulation
- it is not a desktop cursor or generic pointer

### `repeat_tap`

```json
{
  "id": "attack",
  "type": "repeat_tap",
  "slot": 2,
  "pos": { "x": 0.90, "y": 0.75 },
  "key": "MouseLeft",
  "interval_ms": 80
}
```

Behavior:

- key press starts a repeating touch cycle
- key release stops it and releases the slot

### `macro`

```json
{
  "id": "combo",
  "type": "macro",
  "key": "G",
  "sequence": [
    { "action": "down", "pos": { "x": 0.50, "y": 0.30 }, "slot": 6 },
    { "action": "up", "slot": 6, "delay_ms": 50 }
  ]
}
```

Behavior:

- key press starts the sequence
- each step waits for its `delay_ms`
- key release stops the macro and releases active slots

### `layer_shift`

```json
{
  "id": "combat_layer",
  "type": "layer_shift",
  "key": "LeftAlt",
  "layer_name": "combat",
  "mode": "hold"
}
```

Behavior:

- `hold` -> layer active while key is held
- `toggle` -> layer active until pressed again

Deactivating a layer releases touches owned by nodes in that layer.

## 7. Validation Rules

Profiles are rejected if they violate the schema or behavioral rules.

Important rules:

- `version == 1`
- `screen` required
- `global_sensitivity > 0`
- node IDs unique
- slot-bearing nodes use unique slots in `0..9`
- coordinates and regions stay within `[0, 1]`
- joystick radius is in `(0, 1]`
- `mouse_camera.sensitivity > 0`
- `mouse_camera.activation_key` required for `while_held` and `toggle`
- `mouse_camera.activation_key` omitted for `always_on`
- macro `down` steps require `pos`
- all key names must be known to Phantom

## 8. Common Key Names

Examples:

- keyboard: `W`, `A`, `S`, `D`, `Space`, `Enter`, `Esc`, `LeftShift`
- mouse buttons: `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`
- wheel: `WheelUp`, `WheelDown`
- function keys: `F1` through `F12`

## 9. Profile Authoring Guidelines

Good practice:

- keep slot usage simple and explicit
- keep the profile screen exact
- prefer one control concept per node
- use `phantom audit` after every meaningful edit
- use `while_held` or `toggle` for `mouse_camera` when the game needs mode control

## 10. Example Patterns

### PUBG-like

- one joystick
- one `mouse_camera`
- one `hold_tap` for fire
- one `hold_tap` or `toggle_tap` for ADS
- `mouse_camera` mode chosen to match the aiming workflow

The shipped `profiles/pubg.json` uses:

- `MouseLeft` for fire
- `MouseRight` for ADS
- `mouse_camera` in `while_held` mode on `MouseRight`

### Genshin-like

- one joystick
- one `mouse_camera`
- `repeat_tap` for repeated attacks
- `tap` for skill and burst

### Football-like

- one joystick
- `tap` for pass, shoot, switch
- `hold_tap` for sprint or pressure

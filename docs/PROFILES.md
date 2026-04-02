# Profiles

Profiles are JSON files that describe how physical keyboard and mouse input becomes Android touch behavior.

Operationally, the user profile library lives in:

- `~/.config/phantom/profiles/`

The repository also ships starter profiles in:

- `./profiles/`

The GUI reads the user profile library, not the repository directory directly.

## 1. Profile Discovery And Sync

Phantom uses this model:

- shipped profiles live in the repository
- installed profiles live in `~/.config/phantom/profiles/`
- `./install.sh` copies missing shipped profiles into the user profile library
- rerunning `./install.sh` is the supported way to seed newly added shipped profiles

That means:

- repository profiles are the seed library
- user profiles are the active working library
- `phantom-gui` discovers profiles from the user library at startup

## 2. Top-Level Schema

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

## 3. Screen Contract

The profile `screen` must match the daemon `screen`.

Phantom does not guess transforms at runtime. Coordinates are normalized against one explicit surface.

If the profile and daemon disagree, the profile load is rejected.

## 4. Coordinates

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

## 5. Slot Rules

Touch-bearing nodes use unique logical touch slots.

That means:

- slot-bearing nodes must use unique slot ids within the profile
- the profile may define more than 10 touch-bearing nodes
- Phantom allocates physical touch slots dynamically at runtime

Important runtime limit:

- Android and the current backends still support at most 10 simultaneous active touches
- if gameplay would exceed 10 concurrent touches, the backend rejects the extra touch activation

## 6. Layers

Action nodes may include:

```json
"layer": "combat"
```

Rules:

- empty layer means base layer
- `layer_shift` activates named layers
- a base-layer key may not also be reused in a non-base layer
- the same key may be reused across different non-base layers

## 7. Node Types

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
  "mode": "fixed",
  "keys": {
    "up": "W",
    "down": "S",
    "left": "A",
    "right": "D"
  }
}
```

Fields:

- `pos`
  Stick anchor. Required for compatibility and fixed mode.
- `radius`
  Maximum drag distance.
- `mode`
  One of:
  - `fixed`
  - `floating`
- `region`
  Required for `floating`, omitted for `fixed`.

Behavior:

- first direction press -> finger down
- direction changes -> `TouchMove`
- all directions released -> `TouchUp`

Modes:

- `fixed`
  - uses `pos` as the exact center
  - best for visible static sticks
- `floating`
  - chooses a runtime origin inside `region`
  - keeps that origin stable until all movement keys are released
  - best for floating movement zones and football-style drag movement

### `drag`

```json
{
  "id": "lane_left",
  "type": "drag",
  "slot": 2,
  "start": { "x": 0.50, "y": 0.72 },
  "end": { "x": 0.22, "y": 0.72 },
  "key": "A",
  "duration_ms": 90
}
```

Behavior:

- key press -> `TouchDown` at `start`
- Phantom moves the touch toward `end` over `duration_ms`
- the gesture finishes with `TouchUp`
- key release does not cancel the gesture once started

Use cases:

- Temple Run
- Subway Surfers
- sprint-lock drags
- one-shot directional gestures

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

Important:

- `mouse_camera` is touch-drag camera emulation
- it is not desktop pointer emulation
- runtime mouse grab or `F1` alone does not enable camera movement; the loaded profile must contain a `mouse_camera` node

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

- `hold` -> layer active while the key is held
- `toggle` -> layer active until pressed again

## 8. Validation Rules

Profiles are rejected if they violate the schema or runtime rules.

Important rules:

- `version == 1`
- `screen` required
- `global_sensitivity > 0`
- node IDs unique
- slot-bearing nodes use unique logical slots
- coordinates and regions stay within `[0, 1]`
- joystick radius is in `(0, 1]`
- `mouse_camera.sensitivity > 0`
- `mouse_camera.activation_key` required for `while_held` and `toggle`
- `mouse_camera.activation_key` omitted for `always_on`
- all key names must be known to Phantom

## 9. Common Key Names

Examples:

- keyboard: `W`, `A`, `S`, `D`, `Space`, `Enter`, `Esc`, `LeftShift`
- mouse buttons: `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`
- wheel: `WheelUp`, `WheelDown`
- function keys: `F1` through `F12`

## 10. Shipped Starter Profiles

Current shipped library:

- `pubg.json`
- `pubg-mobile-layout1.json`
- `genshin.json`
- `efootball-template.json`
- `temple-run.json`
- `subway-surfers.json`
- `asphalt8.json`
- `asphalt9.json`

Intent:

- `pubg.json`
  compact combat starter
- `pubg-mobile-layout1.json`
  richer PUBG starter based on a real layout screen
- `temple-run.json`
  swipe-only runner starter
- `subway-surfers.json`
  swipe runner starter with hoverboard
- `asphalt8.json`
  keyboard driving starter
- `asphalt9.json`
  keyboard driving starter with 360 control

## 11. Authoring Guidelines

Good practice:

- keep slot usage simple and explicit
- keep the profile screen exact
- prefer one gameplay concept per node
- use `phantom audit` after every meaningful edit
- use `drag` for deliberate swipe gestures
- use `fixed` joystick for visible sticks
- use `floating` joystick for floating movement zones

## 12. Notes On Unsupported Inputs

Profiles currently map touch behavior.

They do not describe:

- accelerometer tilt
- gyroscope motion
- other non-touch Android sensors

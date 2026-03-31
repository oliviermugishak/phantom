# Profile Format

Profiles are JSON files stored in `~/.config/phantom/profiles/`.

They map keyboard and mouse input to one fixed Android touch surface.

## Top-Level Schema

```json
{
  "name": "PUBG Mobile",
  "version": 1,
  "screen": { "width": 1920, "height": 1080 },
  "global_sensitivity": 1.0,
  "nodes": []
}
```

| Field | Required | Notes |
|---|---|---|
| `name` | yes | profile display name |
| `version` | yes | must be `1` |
| `screen` | yes | required fullscreen touch contract |
| `global_sensitivity` | no | default `1.0`, must be positive |
| `nodes` | yes | must not be empty |

## Coordinates

All positions are normalized to `[0.0, 1.0]`.

```text
(0.0, 0.0) ---------------- (1.0, 0.0)
   |                             |
   |        touch surface        |
   |                             |
(0.0, 1.0) ---------------- (1.0, 1.0)
```

At runtime:

```text
pixel_x = rel_x * screen_width
pixel_y = rel_y * screen_height
```

This only works when the daemon `screen` and the profile `screen` match the real Waydroid surface.

## `screen`

```json
"screen": { "width": 1920, "height": 1080 }
```

Rules:

- width and height must be greater than zero
- the values must match the daemon touchscreen resolution
- Phantom rejects the load if the profile and daemon disagree

## Layer Model

Action nodes can carry an optional `layer` string.

Rules:

- empty `layer` means base layer
- `layer_shift` nodes activate named layers
- base-layer keys cannot be reused in alternate layers
- reusing the same key across two non-base layers is allowed

This is how you build alternate game states without turning the editor into a scripting system.

## Node Types

Slot-bearing nodes use slots `0..9`. Those slots must stay unique across all slot-bearing nodes.

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

- key press -> touch down
- key release -> touch up

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

- key press -> touch down
- key held -> finger stays down
- key release -> touch up

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

- first key press -> touch down
- next key press -> touch up
- key release does nothing

This is the simplest way to implement toggle-style holds.

### `joystick`

```json
{
  "id": "move",
  "type": "joystick",
  "slot": 0,
  "pos": { "x": 0.18, "y": 0.72 },
  "radius": 0.07,
  "keys": { "up": "W", "down": "S", "left": "A", "right": "D" }
}
```

Behavior:

- first direction key -> touch down at `pos`, then move to the offset
- direction changes -> touch move
- all direction keys released -> touch up

Notes:

- diagonals are normalized
- opposing directions cancel
- this is a fixed-center joystick

### `mouse_camera`

```json
{
  "id": "look",
  "type": "mouse_camera",
  "slot": 1,
  "region": { "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 },
  "sensitivity": 1.2,
  "invert_y": false
}
```

In the UI this is shown as `Mouse Look`.

Behavior:

- first mouse movement -> finger goes down near the region center
- every mouse delta -> touch move inside the region
- the finger stays active until pause, profile reload, capture exit, or shutdown

Notes:

- the final sensitivity is `node.sensitivity * global_sensitivity`
- this is for look or swipe regions, not desktop pointer emulation

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

- key press starts a repeating down/up cycle
- key release stops the cycle and releases the slot

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
- each step waits for `delay_ms` before running
- `down` steps require `pos`
- releasing the macro key stops the macro and lifts active slots

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

- `mode: "hold"` activates the layer while the key is held
- `mode: "toggle"` turns the layer on or off on each key press
- deactivating a layer releases active touches owned by nodes in that layer

## Common Optional Fields

Action nodes support:

```json
"layer": "combat"
```

If omitted, the node lives in the base layer.

## Validation Rules

Phantom rejects invalid profiles at load time.

Important rules:

- `version == 1`
- `screen` is required
- `global_sensitivity > 0`
- `nodes` must not be empty
- node IDs must be unique
- slot-bearing nodes must use unique slots in `0..9`
- positions and regions must stay inside `[0, 1]`
- `joystick.radius` must be in `(0, 1]`
- `mouse_camera.sensitivity` must be positive
- macro `down` steps require `pos`
- every bound key must be known to Phantom
- base-layer keys cannot be duplicated in alternate layers
- a `layer_shift` key cannot also be an action key

## Common Key Names

Examples:

- keyboard: `W`, `A`, `S`, `D`, `Space`, `LeftShift`, `LeftCtrl`, `Enter`, `Esc`
- mouse buttons: `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`
- mouse wheel: `WheelUp`, `WheelDown`

Matching is case-insensitive.

## Example Patterns

### PUBG Mobile

- `joystick` on the left
- `mouse_camera` on the right
- `hold_tap` for fire
- `toggle_tap` or `hold_tap` for scope
- `tap` for jump, crouch, reload
- `layer_shift` if you want alternate combat bindings

### Genshin Impact

- `joystick`
- `mouse_camera`
- `repeat_tap` for attack spam
- `tap` for skill and burst

### eFootball

- `joystick` for movement
- `tap` for pass, through pass, shoot, switch
- `hold_tap` for sprint or pressure

Use `profiles/efootball-template.json` as a base and tune it in the GUI.

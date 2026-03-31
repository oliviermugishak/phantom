# Profile Format — JSON Specification

A profile maps keyboard/mouse inputs to touch actions. Profiles are stored as JSON files in `~/.config/phantom/profiles/`.

---

## Top-Level Structure

```json
{
  "name": "PUBG Mobile",
  "version": 1,
  "screen": { "width": 1920, "height": 1080 },
  "global_sensitivity": 1.0,
  "nodes": [ ... ]
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Display name for the profile |
| `version` | int | yes | Schema version, currently `1` |
| `screen` | object | no | Override screen resolution. If omitted, auto-detected. |
| `global_sensitivity` | float | no | Multiplier applied to all mouse camera sensitivity. Default `1.0`. |
| `nodes` | array | yes | List of input mapping nodes |

---

## Coordinate System

All positions use **relative coordinates** from `0.0` to `1.0`:

```
(0.0, 0.0) ──────────── (1.0, 0.0)
  │                          │
  │       screen area        │
  │                          │
(0.0, 1.0) ──────────── (1.0, 1.0)
```

At runtime, the daemon converts to pixel coordinates:
```
pixel_x = (rel_x * screen_width) as i32
pixel_y = (rel_y * screen_height) as i32
```

This guarantees profiles work at any resolution without modification.

---

## Node: tap

A simple key-to-touch mapping. Key press = finger down, key release = finger up.

```json
{
  "id": "jump",
  "type": "tap",
  "slot": 3,
  "pos": { "x": 0.92, "y": 0.82 },
  "key": "Space"
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique node identifier |
| `type` | `"tap"` | yes | Node type |
| `slot` | int 0–9 | yes | MT Protocol B slot |
| `pos` | {x, y} | yes | Touch position (relative) |
| `key` | string | yes | Key identifier (see key names) |

**Behavior:**
1. Key pressed → `TouchDown(slot, pos)`
2. Key released → `TouchUp(slot)`

**Use cases:** Jump, reload, crouch, inventory open, any single-action button.

---

## Node: hold_tap

Touch stays down while key is held. Used for continuous actions.

```json
{
  "id": "fire",
  "type": "hold_tap",
  "slot": 2,
  "pos": { "x": 0.88, "y": 0.62 },
  "key": "MouseLeft"
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique node identifier |
| `type` | `"hold_tap"` | yes | Node type |
| `slot` | int 0–9 | yes | MT Protocol B slot |
| `pos` | {x, y} | yes | Touch position (relative) |
| `key` | string | yes | Key identifier |

**Behavior:**
1. Key pressed → `TouchDown(slot, pos)` — finger stays
2. Key held → no additional events (finger already down)
3. Key released → `TouchUp(slot)`

**Use cases:** Fire button, aim-down-sights, sprint, lean.

---

## Node: joystick

Four-key directional input that simulates a virtual joystick. The game sees a finger that moves around a center point.

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

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique node identifier |
| `type` | `"joystick"` | yes | Node type |
| `slot` | int 0–9 | yes | MT Protocol B slot |
| `pos` | {x, y} | yes | Center position of joystick (relative) |
| `radius` | float | yes | Maximum offset from center (relative to screen diagonal) |
| `keys` | object | yes | Direction-to-key mapping |

**Behavior:**

State machine with 4 boolean flags (up, down, left, right):

1. **Any direction key pressed (no finger active):**
   - `TouchDown(slot, pos)` — finger lands at center
   - `TouchMove(slot, offset_position)` — move to computed direction

2. **Direction changes while finger active:**
   - `TouchMove(slot, new_offset_position)` — update position

3. **All direction keys released:**
   - `TouchUp(slot)` — finger lifts

**Offset calculation:**
```
dx = 0.0
dy = 0.0
if up:    dy -= radius
if down:  dy += radius
if left:  dx -= radius
if right: dx += radius

// Diagonal normalization
if two opposing directions pressed: cancel both
if diagonal: vector is already correct (both dx and dy non-zero)

finger_x = center_x + dx
finger_y = center_y + dy
```

**Why the floating joystick problem is solved:**
PUBG (and most mobile games) spawn their joystick at the position of the first touch. By always placing the initial `TouchDown` at the joystick center, the game's joystick aligns with our center position. Subsequent moves are relative to that anchor point.

**Use cases:** Movement, any 4-directional pad.

---

## Node: mouse_camera

Raw mouse movement converted to a continuous touch drag on the screen. Simulates swiping a finger to look around.

```json
{
  "id": "camera",
  "type": "mouse_camera",
  "slot": 1,
  "region": { "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 },
  "sensitivity": 1.2,
  "invert_y": false
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique node identifier |
| `type` | `"mouse_camera"` | yes | Node type |
| `slot` | int 0–9 | yes | MT Protocol B slot |
| `region` | {x, y, w, h} | yes | Screen region for camera control (relative) |
| `sensitivity` | float | yes | Mouse-to-touch sensitivity multiplier |
| `invert_y` | bool | no | Invert Y axis. Default `false`. |

**Behavior:**

This node has a persistent finger that never lifts (while game mode is active):

1. **On first mouse move:**
   - `TouchDown(slot, region_center)` — finger placed at center of region

2. **On every mouse delta event:**
   - Compute new position: `current + (delta * sensitivity * global_sensitivity)`
   - Clamp to region bounds
   - `TouchMove(slot, new_position)`

3. **On game mode toggle off:**
   - `TouchUp(slot)` — finger lifts

**Region format:**
```json
{ "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 }
```
This means the camera touch lives in the right 65% of the screen, full height. Clamping ensures the finger never drifts outside this area.

**Delta computation from raw evdev:**
```
// evdev gives raw REL_X and REL_Y values per event
// These are already relative (delta), not absolute
delta_x = evdev_value as f64 * sensitivity * global_sensitivity
delta_y = evdev_value as f64 * sensitivity * global_sensitivity
if invert_y: delta_y = -delta_y

new_x = clamp(current_x + delta_x, region.x, region.x + region.w)
new_y = clamp(current_y + delta_y, region.y, region.y + region.h)
```

**Why region-based instead of free movement:**
If the camera finger moves outside its designated area, it can accidentally trigger other game buttons. The region constraint prevents this.

**Use cases:** Camera look, aim control.

---

## Node: repeat_tap

Repeatedly fires taps at a fixed interval while a key is held.

```json
{
  "id": "auto_fire",
  "type": "repeat_tap",
  "slot": 5,
  "pos": { "x": 0.75, "y": 0.50 },
  "key": "F",
  "interval_ms": 100
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique node identifier |
| `type` | `"repeat_tap"` | yes | Node type |
| `slot` | int 0–9 | yes | MT Protocol B slot |
| `pos` | {x, y} | yes | Touch position (relative) |
| `key` | string | yes | Key identifier |
| `interval_ms` | int | yes | Milliseconds between taps |

**Behavior:**

1. Key pressed → start repeating cycle:
   - `TouchDown(slot, pos)`
   - Wait `interval_ms`
   - `TouchUp(slot)`
   - Wait `interval_ms` (brief gap to register as distinct tap)
   - Repeat from top
2. Key released → `TouchUp(slot)` (if finger is down), stop repeating

**Timing:**
```
key down ──► DOWN ──► 100ms ──► UP ──► 50ms ──► DOWN ──► 100ms ──► UP ──► ...
                                                                            │
key up ────────────────────────────────────────────────────────────► UP ──┘
```

The gap between UP and next DOWN should be `interval_ms / 2` to maintain the configured tap rate.

**Use cases:** Auto-fire, rapid looting, spam interactions.

---

## Node: macro (sequence)

A timed sequence of taps triggered by a single key press.

```json
{
  "id": "combo",
  "type": "macro",
  "key": "G",
  "sequence": [
    { "action": "down", "pos": { "x": 0.50, "y": 0.30 }, "slot": 6 },
    { "action": "up", "slot": 6, "delay_ms": 50 },
    { "action": "down", "pos": { "x": 0.55, "y": 0.35 }, "slot": 6 },
    { "action": "up", "slot": 6, "delay_ms": 50 }
  ]
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | Unique node identifier |
| `type` | `"macro"` | yes | Node type |
| `key` | string | yes | Key that triggers the macro |
| `sequence` | array | yes | Ordered list of actions |

**Each step in sequence:**

| Field | Type | Required | Description |
|---|---|---|---|
| `action` | `"down"` or `"up"` | yes | Touch down or touch up |
| `pos` | {x, y} | for `"down"` | Touch position |
| `slot` | int 0–9 | yes | Which slot to use |
| `delay_ms` | int | no | Wait before executing this step. Default `0`. |

**Behavior:**

1. Key pressed → execute sequence steps in order, respecting `delay_ms` between steps
2. Key released → if macro is still executing, stop (lift all active fingers from the macro)
3. Macro can only run once per key press (no re-trigger while running)

**Use cases:** Crouch+jump combos, lean+peek+fire sequences, emote chains.

---

## Key Name Reference

Keys are identified by their Linux input subsystem names, with some aliases:

### Keyboard Keys
Letters: `A` through `Z`
Numbers: `0` through `9` (top row), `KP0` through `KP9` (numpad)
Function: `F1` through `F12`
Modifiers: `LEFTCTRL`, `RIGHTCTRL`, `LEFTSHIFT`, `RIGHTSHIFT`, `LEFTALT`, `RIGHTALT`, `LEFTMETA`, `RIGHTMETA`
Navigation: `UP`, `DOWN`, `LEFT`, `RIGHT`, `HOME`, `END`, `PAGEUP`, `PAGEDOWN`
Editing: `SPACE`, `ENTER`, `BACKSPACE`, `DELETE`, `INSERT`, `TAB`, `ESC`
Punctuation: `MINUS`, `EQUAL`, `LEFTBRACE`, `RIGHTBRACE`, `SEMICOLON`, `APOSTROPHE`, `GRAVE`, `BACKSLASH`, `COMMA`, `DOT`, `SLASH`

### Mouse Buttons
`MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`

### Aliases
| Alias | Linux Name |
|---|---|
| `Ctrl` | `LEFTCTRL` |
| `Shift` | `LEFTSHIFT` |
| `Alt` | `LEFTALT` |
| `Win` / `Super` | `LEFTMETA` |
| `Enter` | `ENTER` |
| `Esc` | `ESC` |
| `Space` | `SPACE` |
| `Tab` | `TAB` |

---

## Slot Allocation Rules

1. Each node must have a unique `slot` number (0–9)
2. `mouse_camera` must be slot 0 or 1 (convention, not enforced)
3. `joystick` typically gets the lowest available slot
4. Slots are never re-assigned at runtime
5. Maximum 10 nodes per profile (MT Protocol B slot limit)

Recommended allocation:

| Slot | Suggested Use |
|---|---|
| 0 | Joystick (movement) |
| 1 | Mouse camera |
| 2 | Fire / hold action |
| 3–8 | Tap / hold_tap nodes |
| 9 | Macros / repeat_tap |

---

## Validation Rules

On profile load, the daemon validates:

1. **Schema correctness:** JSON parses, all required fields present
2. **Slot uniqueness:** No two nodes share a slot number
3. **Slot range:** All slots are 0–9
4. **Key validity:** All referenced keys exist in the key name table
5. **Key uniqueness:** No two nodes bind the same key (unless macro which owns its key)
6. **Coordinate range:** All `pos` x/y are 0.0–1.0
7. **Joystick has 4 directions:** `up`, `down`, `left`, `right` all present
8. **Region sanity:** `region.w > 0`, `region.h > 0`, region fits in 0.0–1.0
9. **Repeat interval:** `interval_ms >= 16` (minimum ~60Hz)
10. **Macro sequence non-empty:** At least one step

Validation errors are fatal — daemon refuses to load an invalid profile and logs the specific error.

---

## Example Profile: PUBG Mobile

```json
{
  "name": "PUBG Mobile",
  "version": 1,
  "global_sensitivity": 1.0,
  "nodes": [
    {
      "id": "move",
      "type": "joystick",
      "slot": 0,
      "pos": { "x": 0.18, "y": 0.72 },
      "radius": 0.07,
      "keys": { "up": "W", "down": "S", "left": "A", "right": "D" }
    },
    {
      "id": "camera",
      "type": "mouse_camera",
      "slot": 1,
      "region": { "x": 0.35, "y": 0.0, "w": 0.65, "h": 1.0 },
      "sensitivity": 1.2,
      "invert_y": false
    },
    {
      "id": "fire",
      "type": "hold_tap",
      "slot": 2,
      "pos": { "x": 0.88, "y": 0.62 },
      "key": "MouseLeft"
    },
    {
      "id": "ads",
      "type": "hold_tap",
      "slot": 3,
      "pos": { "x": 0.78, "y": 0.55 },
      "key": "MouseRight"
    },
    {
      "id": "jump",
      "type": "tap",
      "slot": 4,
      "pos": { "x": 0.92, "y": 0.82 },
      "key": "Space"
    },
    {
      "id": "crouch",
      "type": "tap",
      "slot": 5,
      "pos": { "x": 0.85, "y": 0.90 },
      "key": "C"
    },
    {
      "id": "reload",
      "type": "tap",
      "slot": 6,
      "pos": { "x": 0.78, "y": 0.88 },
      "key": "R"
    },
    {
      "id": "prone",
      "type": "tap",
      "slot": 7,
      "pos": { "x": 0.80, "y": 0.95 },
      "key": "Z"
    }
  ]
}
```

---

## Example Profile: Genshin Impact

```json
{
  "name": "Genshin Impact",
  "version": 1,
  "global_sensitivity": 0.8,
  "nodes": [
    {
      "id": "move",
      "type": "joystick",
      "slot": 0,
      "pos": { "x": 0.12, "y": 0.65 },
      "radius": 0.08,
      "keys": { "up": "W", "down": "S", "left": "A", "right": "D" }
    },
    {
      "id": "camera",
      "type": "mouse_camera",
      "slot": 1,
      "region": { "x": 0.30, "y": 0.0, "w": 0.70, "h": 1.0 },
      "sensitivity": 1.0
    },
    {
      "id": "attack",
      "type": "repeat_tap",
      "slot": 2,
      "pos": { "x": 0.90, "y": 0.75 },
      "key": "MouseLeft",
      "interval_ms": 80
    },
    {
      "id": "skill",
      "type": "tap",
      "slot": 3,
      "pos": { "x": 0.85, "y": 0.65 },
      "key": "E"
    },
    {
      "id": "burst",
      "type": "tap",
      "slot": 4,
      "pos": { "x": 0.92, "y": 0.55 },
      "key": "Q"
    },
    {
      "id": "jump",
      "type": "tap",
      "slot": 5,
      "pos": { "x": 0.88, "y": 0.85 },
      "key": "Space"
    },
    {
      "id": "sprint",
      "type": "hold_tap",
      "slot": 6,
      "pos": { "x": 0.82, "y": 0.90 },
      "key": "LEFTSHIFT"
    }
  ]
}
```

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
  Multiplier applied to aim sensitivity.
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

Phantom rounds normalized coordinates to the nearest target pixel at injection
time instead of always biasing toward the upper-left corner.

## 5. Slot Rules

Touch-bearing nodes use unique logical touch slots.

That means:

- slot-bearing nodes must use unique slot ids within the profile
- the profile may define more than 10 touch-bearing nodes
- Phantom allocates physical touch slots dynamically at runtime

Important runtime limit:

- Android and the current backends still support at most 10 simultaneous active touches
- if gameplay would exceed 10 concurrent touches, the backend rejects the extra touch activation
- the GUI auto-assigns logical slot ids from the full `0..255` range, not just `0..9`

## 6. Layers

Action nodes may include:

```json
"layer": "combat"
```

Rules:

- empty layer means base layer
- `layer_shift` activates named layers
- a base-layer key may not also be reused in a non-base layer unless every
  `layer_shift` that can activate that non-base layer uses `suspend_base: true`
- the same key may be reused across different non-base layers

For practical large-profile design, especially shooters with vehicles and parachutes, see [GAME_PATTERNS.md](GAME_PATTERNS.md).

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
- very short keyboard taps are kept down for a tiny minimum runtime pulse so
  Android buttons still register reliably without removing hold behavior

Compatibility:

- legacy `"type": "hold_tap"` entries still load
- Phantom normalizes them to standard `tap` behavior on load

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

Fields:

- `pos`
  Stick anchor.
- `radius`
  Base drag distance. For keyboard movement, Phantom now prefers a strong
  full-throw swipe, driving from the anchor toward the screen edge instead of
  making a short local nudge.

Behavior:

- first direction press -> finger down
- direction changes -> `TouchMove`
- all directions released -> immediate neutral recenter, then a very short delayed
  `TouchUp` so fast re-engage does not require a fresh drag start
- opposite directions on the same axis prefer the most recently pressed key
  instead of collapsing to a neutral stall while both keys are briefly held
- joystick movement now uses a long edge-directed swipe from the configured
  center, which is better for games that expect a stronger drag to start
  running or sprinting

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

### `aim`

```json
{
  "id": "camera",
  "type": "aim",
  "slot": 1,
  "anchor": { "x": 0.75, "y": 0.5 },
  "reach": 0.18,
  "sensitivity": 1.2,
  "curve": "balanced",
  "activation_mode": "while_held",
  "activation_key": "MouseRight",
  "invert_y": false
}
```

Fields:

- `anchor`
  Normalized internal recenter point for the hidden look touch.
- `reach`
  Advanced travel envelope for the hidden look touch. Real mouse aim is allowed
  to use a wider internal envelope than touchpad aim so high-speed camera turns
  re-center less often, but this still should not be treated like a visible
  region size.
- `sensitivity`
  Node-local multiplier.
- `curve`
  Mouse response preset for relative camera input:
  - `balanced`
  - `precision`
  - `linear`
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

- `aim` is touch-drag camera emulation
- it is not desktop pointer emulation
- menu-touch navigation is runtime behavior, not a profile node
- runtime mouse grab or `F1` alone does not enable camera movement; the loaded profile must contain an `aim` node
- aim motion is still emitted immediately from input movement
- real mouse movement keeps per-report relative-event cadence; Phantom combines
  X/Y from the same evdev report, but does not merge separate reports into
  larger aim steps
- real mouse aim now uses a source-specific response curve: tiny relative
  movements are damped for precision while larger sweeps keep strong turn speed
- touchpad roughness is reduced in input translation by splitting large absolute
  touchpad jumps into smaller motion steps before they reach the engine
- touchpad contact start and end now explicitly re-arm aim between swipes, so
  fast repeated swipe contacts do not inherit stale hidden-touch edge state
- large relative-mouse sweeps are no longer limited by the old fixed
  re-segmentation loop; Phantom keeps re-centering the hidden look touch until
  the movement is consumed or a high safety cap is reached
- toggled aim state survives `F1` mouse routing changes
- `while_held` mouse activation keys are resynced when mouse routing is re-enabled
- `while_held` aim now re-centers on release before the next re-engage, so a
  quick RMB release and re-press does not restart from a stale edge position
- older profiles using `type = "mouse_camera"` and `region` still load; Phantom normalizes them to `aim` semantics internally
- for high-paced shooter play, a real mouse is still the recommended hardware path; touchpad aim remains best-effort

For recommended shooter setups such as ADS-driven look, layered contexts, and sprint-lock drag patterns, see [GAME_PATTERNS.md](GAME_PATTERNS.md).

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
- `interval_ms` must be greater than zero
- practical repeat timing is bounded by the daemon tick cadence; Phantom now runs that at `4ms`, so values below that may collapse toward the same effective rate depending on scheduler timing

Use `repeat_tap` when you only need a simple repeating interval.

### `wheel`

```json
{
  "id": "stance_wheel",
  "type": "wheel",
  "up_slot": 8,
  "up_pos": { "x": 0.84, "y": 0.40 },
  "down_slot": 9,
  "down_pos": { "x": 0.84, "y": 0.56 }
}
```

Behavior:

- `WheelUp` injects a one-shot tap on `up_slot` at `up_pos`
- `WheelDown` injects a one-shot tap on `down_slot` at `down_pos`
- the wheel directions are built in; this node does not bind an arbitrary key
- `up_slot` and `down_slot` must be different

Use `wheel` when a shooter or menu-heavy game needs scroll-up and scroll-down
to hit different on-screen targets such as zoom, stance, weapon cycling, or
context actions.

### `macro`

```json
{
  "id": "combo",
  "type": "macro",
  "key": "G",
  "mode": "one_shot",
  "sequence": [
    { "action": "down", "pos": { "x": 0.50, "y": 0.30 }, "slot": 6 },
    { "action": "up", "slot": 6, "delay_ms": 50 }
  ]
}
```

Behavior:

- key press starts the sequence
- each step waits for its `delay_ms`
- `mode = "cancel_on_release"` keeps the old behavior: key release stops the macro and releases active slots immediately
- `mode = "one_shot"` lets the macro continue to completion after the key is released

### `layer_shift`

```json
{
  "id": "combat_layer",
  "type": "layer_shift",
  "key": "LeftAlt",
  "layer_name": "combat",
  "mode": "hold",
  "suspend_base": true
}
```

Behavior:

- `hold` -> layer active while the key is held
- `toggle` -> layer active until pressed again
- `suspend_base = true` -> base-layer action and camera nodes are temporarily
  disabled while the target layer is active, which lets the target layer reuse
  the same keys without firing both contexts at once

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
- `aim.sensitivity > 0`
- `aim.activation_key` required for `while_held` and `toggle`
- `aim.activation_key` omitted for `always_on`
- if a non-base layer reuses a base-layer key, every `layer_shift` that can
  activate that layer must use `suspend_base = true`
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
- `pubg-small.json`
- `genshin.json`
- `efootball-template.json`
- `temple-run.json`
- `subway-surfers.json`
- `asphalt8.json`
- `asphalt9.json`

Intent:

- `pubg.json`
  full PUBG starter based on a real custom-layout screen
- `pubg-small.json`
  compact PUBG starter for reduced layouts and focused testing
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
- use `joystick` for continuous movement from a fixed stick center

## 12. Notes On Unsupported Inputs

Profiles currently map touch behavior.

They do not describe:

- accelerometer tilt
- gyroscope motion
- other non-touch Android sensors

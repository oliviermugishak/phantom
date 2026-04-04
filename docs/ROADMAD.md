# Roadmad

This file is the detailed architecture backlog for Phantom after `0.8.1`.

This file records the specific system decisions, module boundaries, and
implementation checklists for the highest-value next work.

The focus here is only on work that fits Phantom's current architecture and can
be implemented cleanly. It does not include speculative work that would require
an entirely different product or unsupported platform hooks.

## Ground Rules

These decisions apply to all items below.

- keep accurate cursor-to-touch mapping isolated from focus/activation logic
- do not hide runtime behavior behind silent fallback
- keep new control semantics explicit in the profile schema
- do not push engine problems into the GUI
- keep the mouse path fast; do not add smoothing or latency to real relative mouse input
- treat touchpad support as best-effort, not equivalent to a real mouse

## Explicit Non-Goals

These are not acceptable solutions:

- fake auto-double-tap behavior for menu-touch activation
- mixing focus commands into the cursor helper protocol
- letting focus-preparation failure tear down the coordinate backend
- pretending touchpad aim can fully match a real mouse for high-paced shooters
- exposing user-facing "magic" behavior without status fields, logs, or docs

## 1. Owned Menu-Touch Mode

Status:

- completed in `0.8.0`

### Problem

Passive released-mouse menu-touch was architecturally wrong for high-reliability
game navigation:

- the desktop still received the real physical click
- the host could consume that click for activation/focus
- cursor mapping quality did not solve first-click loss

This is a click-ownership problem, not just a coordinate problem.

### Decision

Replace passive released-mouse menu-touch with owned menu-touch.

Phantom should keep the mouse captured during gameplay capture and switch that
owned mouse between:

- `menu_touch`
- `aim`

The owned menu-touch cursor should be seeded from host cursor position when the
mode is entered, then continue from Phantom-owned cursor state.

Because the desktop cursor no longer moves in that mode, Phantom also needs a
dedicated lightweight cursor overlay to visualize where the owned menu-touch
cursor will land.

### Runtime Architecture

Primary files:

- `phantom/src/input.rs`
- `phantom/src/ipc.rs`
- `phantom/src/mouse_touch.rs`
- `phantom/src/main.rs`
- `phantom-gui/src/main.rs`

### Integration Points

Runtime:

- keep the physical mouse grabbed while capture is active
- represent runtime mouse state explicitly as:
  - `menu_touch`
  - `aim`
- seed the owned menu-touch cursor once from host cursor position when entering
  `menu_touch`
- keep `phantom/src/mouse_touch.rs` focused on translating owned cursor motion
  into touch
- launch a dedicated cursor overlay while `mouse_mode == menu_touch`

Status/reporting:

- expose:
  - `menu_touch_backend`
  - `mouse_mode`
- surface these through `phantom status`
- mirror them in GUI runtime status

### Implementation Checklist

- [x] keep physical mouse grab active through capture
- [x] add explicit runtime mouse mode state
- [x] seed owned menu-touch cursor from host position on menu-touch entry
- [x] route menu-touch entirely through Phantom-owned cursor state after that seed
- [x] visualize the owned menu-touch cursor through a dedicated runtime overlay
- [x] extend CLI/GUI status with `mouse_mode`
- [x] document owned menu-touch behavior and limitations
- [x] support touchpad tap-to-click and double-tap-hold drag in owned menu-touch

### Acceptance Criteria

- menu-touch no longer depends on host window activation semantics
- the first click acts directly in the owned menu-touch path
- cursor seeding remains as accurate as the available compositor/helper backend
- status output makes the active mouse mode obvious

## 2. Aim V2

Status:

- implemented in `0.8.x`

### Problem

Current `aim` is functionally better than the old `mouse_camera`, but it still
inherits too much of the old mental model:

- user-facing `reach`
- hidden touch still behaves like a bounded drag region
- large `reach` values encourage layouts that feel like a screen wrapper

### Decision

Keep `aim` as touch-drag camera emulation, but redesign it as a camera-first
primitive rather than a visible-region primitive.

### Runtime Architecture

Primary files:

- `phantom/src/engine.rs`
- `phantom/src/profile.rs`

Goals:

- keep aim immediate for relative mouse input
- keep the hidden touch tightly constrained around its anchor
- make recenter/resegment behavior purely internal
- reduce the importance of user-facing `reach`

### GUI Architecture

Primary file:

- `phantom-gui/src/main.rs`

Goals:

- stop presenting `aim` like a region tool
- present it like a camera/look primitive
- move emphasis toward:
  - sensitivity
  - activation mode
  - activation key
  - invert Y

### Implementation Checklist

- [x] reduce `reach` prominence in the GUI
- [x] show `aim` as a camera tool, not a large editable wrapper
- [x] keep reach as validation-only tuning rather than a warning-heavy UX surface
- [x] document that operational aim travel is intentionally tighter than raw configured reach
- [x] keep old `mouse_camera` profiles loading cleanly
- [x] treat touchpad contact start/end as explicit aim re-arm boundaries so repeated swipes do not inherit stale hidden-touch edge state

### Acceptance Criteria

- aim feels like camera control, not a roaming drag box
- nearby controls are less likely to collide with aim during normal play
- existing profiles stay valid

## 3. Dedicated Wheel Controls

Status:

- implemented in `0.8.x`

### Problem

Wheel-driven actions are common in shooter mappings:

- zoom
- weapon change
- stance change
- repeated utility actions

Plain wheel-as-key is not enough for all of these.

### Decision

Add one explicit paired wheel control first instead of a whole family of wheel
types.

### Implemented Control

- `wheel`

Fields:

- `up_slot`
- `up_pos`
- `down_slot`
- `down_pos`

### Runtime Architecture

Primary files:

- `phantom/src/profile.rs`
- `phantom/src/engine.rs`

Behavior:

- `WheelUp` triggers a one-shot tap at the upper target
- `WheelDown` triggers a one-shot tap at the lower target
- each direction owns its own logical touch slot
- slots must be distinct and validate cleanly

### GUI Architecture

Primary file:

- `phantom-gui/src/main.rs`

Needs:

- one placement tool that edits both up/down targets together
- clear inspector language that wheel directions are built in
- correct logical slot allocation for both targets

### Implementation Checklist

- [x] decide minimal useful wheel primitive
- [x] add schema and validation
- [x] add GUI support
- [x] add audit output support
- [x] document good wheel-based shooter patterns
- [x] add tests for both wheel directions

### Acceptance Criteria

- wheel-heavy profiles become easier to author cleanly
- users no longer need awkward macro workarounds for common wheel actions

## 4. Layer Workflow Improvements

### Problem

Layers are already powerful at runtime, but large shooter profiles are still too
manual to author and maintain.

### Decision

Keep the runtime layer model, improve authoring flow around it.

### Runtime Architecture

No major runtime change is required first. This is primarily a GUI and docs
problem.

### GUI Architecture

Primary file:

- `phantom-gui/src/main.rs`

High-value additions:

- easier duplicate-to-layer flows
- clearer layer assignment/editing
- stronger layer-specific filtering and search
- shooter-context helpers for:
  - `vehicle`
  - `parachute`
  - `loot`
  - `bag`
  - `drive`

### Implementation Checklist

- [ ] add control list search/filter by id, key, type, and layer
- [ ] make layer reassignment faster for large profiles
- [ ] add shooter-oriented layer examples to docs
- [ ] keep runtime semantics unchanged unless a clear need emerges

### Acceptance Criteria

- large PUBG/COD-style profiles are faster to build and maintain
- users stop fighting the editor when managing many contexts

## 5. Control Conflict Analysis

### Problem

Emulator presets feel polished partly because they implicitly avoid control
conflicts. Phantom currently leaves too much of that burden on the user.

### Decision

Add explicit analysis for risky spatial and semantic collisions.

### Runtime / Schema Surface

Primary files:

- `phantom/src/profile.rs`
- `phantom-gui/src/main.rs`

Potential conflict checks:

- aim anchor too close to tappable controls
- joystick region too close to critical tap controls
- drag path crossing other controls
- excessively dense control clusters in the same thumb region

### Implementation Checklist

- [ ] define conflict heuristics in the profile/audit layer
- [ ] surface warnings in CLI audit output
- [ ] surface the same warnings in the GUI
- [ ] document that warnings are advisory, not hard validation failures

### Acceptance Criteria

- risky layouts are visible before live gameplay testing
- Phantom starts offering premium setup guidance instead of only raw freedom

## 6. Macro Improvements

Status:

- partially implemented in `0.8.x`

### Problem

Macros are useful, but still basic for advanced game workflows.

### Decision

Extend them only where the behavior stays explicit and testable.

### High-Value Items

- explicit run modes for cancelable versus one-shot macros
- hold-until-release behavior
- clearer step waveform control

### Runtime Architecture

Primary files:

- `phantom/src/profile.rs`
- `phantom/src/engine.rs`

### Implementation Checklist

- [x] decide the smallest useful macro extension
- [x] keep state transitions explicit
- [x] add targeted tests for cancellation and release behavior
- [x] document exact cancellation semantics
- [ ] consider hold-until-release behavior only if it remains cleaner than a dedicated future primitive

### Acceptance Criteria

- macros become more useful without turning into an opaque scripting language

## 7. Prioritization

Recommended order:

1. Aim V2 cleanup
2. dedicated wheel controls
3. layer workflow improvements
4. macro improvements
5. control conflict analysis if it still has product value after the core workflows settle

## 8. Success Criteria

This roadmap is successful when:

- menu-touch no longer relies on desktop click ownership during capture
- aim behaves like a camera primitive, not a roaming wrapper
- wheel-heavy shooter profiles become easier to author and maintain
- large layered shooter profiles become practical to author
- docs and GUI both reflect the real runtime model

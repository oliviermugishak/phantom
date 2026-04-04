# Roadmad

This file is the detailed architecture backlog for Phantom after `0.7.0`.

It is intentionally deeper than [ROADMAP.md](ROADMAP.md). The short roadmap
tracks product direction at a high level. This file records the specific system
decisions, module boundaries, and implementation checklists for the highest
value next work.

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

- [ ] keep physical mouse grab active through capture
- [ ] add explicit runtime mouse mode state
- [ ] seed owned menu-touch cursor from host position on menu-touch entry
- [ ] route menu-touch entirely through Phantom-owned cursor state after that seed
- [ ] visualize the owned menu-touch cursor through a dedicated runtime overlay
- [ ] extend CLI/GUI status with `mouse_mode`
- [ ] document owned menu-touch behavior and limitations

### Acceptance Criteria

- menu-touch no longer depends on host window activation semantics
- the first click acts directly in the owned menu-touch path
- cursor seeding remains as accurate as the available compositor/helper backend
- status output makes the active mouse mode obvious

## 2. Aim V2

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

- [ ] reduce `reach` prominence in the GUI
- [ ] show `aim` as a camera tool, not a large editable wrapper
- [ ] add validation warnings for extreme `reach` values
- [ ] document that operational aim travel is intentionally tighter than raw configured reach
- [ ] keep old `mouse_camera` profiles loading cleanly

### Acceptance Criteria

- aim feels like camera control, not a roaming drag box
- nearby controls are less likely to collide with aim during normal play
- existing profiles stay valid

## 3. Burst / Turbo Control

### Problem

`repeat_tap` is useful, but it only models a simple interval. That is too weak
for premium shooter workflows such as controlled rapid fire and pulse-shaped tap
behavior.

### Decision

Add a dedicated high-rate pulse control instead of overloading `repeat_tap`.

### Proposed Control

Suggested node name:

- `burst_tap`

Suggested fields:

- `slot`
- `pos`
- `key`
- `initial_delay_ms`
- `press_ms`
- `release_ms`
- `burst_count` optional

### Runtime Architecture

Primary files:

- `phantom/src/profile.rs`
- `phantom/src/engine.rs`

Behavior:

- explicit press waveform, not only interval timing
- clean release behavior when the key is released
- deterministic tick-driven pulse state

### GUI Architecture

Primary file:

- `phantom-gui/src/main.rs`

Needs:

- editor support for the new waveform fields
- clear distinction from `repeat_tap`

### Implementation Checklist

- [ ] add `burst_tap` schema and validation
- [ ] implement engine state machine for pulse timing
- [ ] add GUI editor support
- [ ] add audit output support
- [ ] document when to prefer `burst_tap` vs `repeat_tap`
- [ ] add tests for high-rate pulse timing and release correctness

### Acceptance Criteria

- ultra-rapid fire can be shaped explicitly
- behavior is deterministic and testable
- the user does not need to misuse `repeat_tap` for shooter turbo behavior

## 4. Tap-Hold Primitive

### Problem

Some high-value shooter actions want one key to mean:

- tap = one action
- hold = another action

Current nodes do not model this cleanly.

### Decision

Add a dedicated dual-phase control instead of forcing this into macros or layers.

### Proposed Control

Suggested node name:

- `tap_hold`

Likely fields:

- `tap_slot`
- `tap_pos`
- `hold_slot`
- `hold_pos`
- `key`
- `hold_threshold_ms`

### Runtime Architecture

Primary files:

- `phantom/src/profile.rs`
- `phantom/src/engine.rs`

Behavior:

- tap path must remain low-latency
- hold path must be deterministic
- cancellation rules must be explicit

### GUI Architecture

- editor needs a clear tap-vs-hold explanation
- the inspect panel should make the threshold visible and understandable

### Implementation Checklist

- [ ] add schema and validation
- [ ] implement runtime threshold state machine
- [ ] add GUI editor support
- [ ] add tests for tap, hold, and edge-threshold behavior
- [ ] document strong shooter use cases

### Acceptance Criteria

- common dual-action keys become easy to author
- users no longer need awkward macro or layer hacks for tap-vs-hold bindings

## 5. Dedicated Wheel Controls

### Problem

Wheel-driven actions are common in shooter mappings:

- zoom
- weapon change
- stance change
- repeated utility actions

Plain wheel-as-key is not enough for all of these.

### Decision

Add explicit wheel-oriented control types rather than expecting users to model
everything through generic key behavior.

### Candidates

- `wheel_tap`
- `wheel_repeat`
- `wheel_drag`

### Runtime Architecture

Primary files:

- `phantom/src/profile.rs`
- `phantom/src/engine.rs`
- `phantom/src/input.rs` only if richer wheel semantics are needed

### GUI Architecture

- add editor affordances that make wheel direction explicit

### Implementation Checklist

- [ ] decide minimal useful wheel primitives
- [ ] add schema and validation
- [ ] add GUI support
- [ ] document good wheel-based shooter patterns

### Acceptance Criteria

- wheel-heavy profiles become easier to author cleanly
- users no longer need awkward macro workarounds for common wheel actions

## 6. Layer Workflow Improvements

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

## 7. Control Conflict Analysis

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

## 8. Macro Improvements

### Problem

Macros are useful, but still basic for advanced game workflows.

### Decision

Extend them only where the behavior stays explicit and testable.

### High-Value Items

- cancelable macros
- hold-until-release behavior
- clearer step waveform control

### Runtime Architecture

Primary files:

- `phantom/src/profile.rs`
- `phantom/src/engine.rs`

### Implementation Checklist

- [ ] decide the smallest useful macro extension
- [ ] keep state transitions explicit
- [ ] add targeted tests for cancellation and release behavior
- [ ] document exact cancellation semantics

### Acceptance Criteria

- macros become more useful without turning into an opaque scripting language

## 9. Prioritization

Recommended order:

1. Owned menu-touch mode
2. Aim V2 cleanup
3. Burst / turbo control
4. Tap-hold primitive
5. Control conflict analysis
6. Layer workflow improvements
7. Wheel controls
8. Macro improvements

## 10. Success Criteria

This roadmap is successful when:

- menu-touch no longer relies on desktop click ownership during capture
- aim behaves like a camera primitive, not a roaming wrapper
- Phantom has at least one dedicated shooter-grade rapid-fire primitive
- large layered shooter profiles become practical to author
- users get warnings before creating self-conflicting layouts
- docs and GUI both reflect the real runtime model

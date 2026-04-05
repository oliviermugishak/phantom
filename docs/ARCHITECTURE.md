# Architecture

This document describes the current Phantom architecture as it exists today.

The primary design is:

- Linux-side input capture and mapping
- Android-side touch injection

`uinput` still exists, but it is the fallback path, not the center of the design.

## 1. High-Level Goal

Phantom exists to solve one problem well:

- turn local keyboard and mouse input into predictable Android gameplay touches for fullscreen Waydroid

That leads to four architectural decisions:

1. explicit screen contract
2. explicit runtime state
3. profile-driven touch synthesis
4. Android framework injection as the primary backend

## 2. System Diagram

```text
Host Linux
─────────────────────────────────────────────────────────────────────
 physical keyboard/mouse
          │
          ▼
  evdev capture
  phantom/src/input.rs
          │
          ▼
  InputEvent stream
          │
          ▼
  keymap engine
  phantom/src/engine.rs
          │
          ▼
  TouchCommand stream
          │
          ├───────────────────────────────┐
          │                               │
          ▼                               ▼
 android_socket backend               uinput backend
 phantom/src/android_inject.rs        phantom/src/inject.rs
          │                               │
          ▼                               ▼
     TCP connection                  /dev/uinput device
          │                               │
          ▼                               ▼

Waydroid Android container
─────────────────────────────────────────────────────────────────────
 app_process server
 contrib/android-server/src/com/phantom/server/PhantomServer.java
          │
          ▼
 MotionEvent construction
          │
          ▼
 InputManager.injectInputEvent()
          │
          ▼
 Android input dispatch
          │
          ▼
 game
```

## 3. Main Components

### 3.1 Input Capture

File:

- `phantom/src/input.rs`

Responsibilities:

- discover relevant `evdev` devices
- classify keyboard vs mouse capability sets
- read raw input events
- translate them into Phantom `InputEvent`s
- maintain runtime grab state for keyboard and mouse independently
- recover key/button state after `SYN_DROPPED`

Why it matters:

- everything downstream depends on clean, stable semantic input
- the project needs to distinguish observation from routing

### 3.2 Engine

File:

- `phantom/src/engine.rs`

Responsibilities:

- load and execute profile semantics
- maintain node state across time
- consume `InputEvent`
- emit `TouchCommand`

The engine is intentionally synchronous and explicit. It is a state machine, not a rules DSL or scripting runtime.

### 3.3 Profile Model

File:

- `phantom/src/profile.rs`

Responsibilities:

- profile schema
- validation
- audit output
- semantic rules such as slot uniqueness and activation requirements

This is the contract between the GUI, the daemon, and test tooling.

### 3.4 IPC Control Plane

File:

- `phantom/src/ipc.rs`

Responsibilities:

- JSON-over-Unix-socket daemon control
- status responses for CLI and GUI
- runtime operations like load, capture, pause, and mouse routing
- explicit runtime mouse-mode status

Important runtime boundary:

- menu-touch is a runtime-owned mouse mode, not a profile node
- the owned menu-touch cursor is seeded from host cursor position when capture enters menu-touch
- after that seed, menu-touch uses Phantom-owned cursor state and does not depend on desktop click delivery
- a separate lightweight cursor overlay visualizes the owned menu-touch cursor while that mode is active
- on Wayland compositors, that cursor overlay uses a layer-shell surface with an empty input region so Phantom does not steal mouse input back from Waydroid
- touchpad tap gestures are synthesized inside Phantom while menu-touch owns the mouse, because the desktop is no longer responsible for translating them

This is what makes the GUI and CLI first-class runtime controls instead of file-only tools.

### 3.5 Daemon Orchestration

File:

- `phantom/src/main.rs`

Responsibilities:

- startup
- config load
- backend selection
- event loop
- runtime hotkeys
- command-line mode vs daemon mode

This file is the runtime orchestrator, not the place where business logic should accumulate long term.

### 3.6 Android Backend

Files:

- `phantom/src/android_inject.rs`
- `phantom/src/waydroid.rs`
- `contrib/android-server/src/com/phantom/server/PhantomServer.java`

Responsibilities:

- discover container reachability
- stage the Android server jar into the container
- launch `app_process`
- maintain a TCP connection
- encode and decode the touch protocol
- reconstruct MotionEvents inside Android

### 3.7 `uinput` Backend

File:

- `phantom/src/inject.rs`

Responsibilities:

- create a virtual touchscreen
- translate `TouchCommand` into MT Protocol B events

This backend remains important as:

- a compatibility path
- a low-level debugging path
- a reference for the abstract touch interface

But it is not the primary product path anymore.

### 3.8 GUI

File:

- `phantom-gui/src/main.rs`

Responsibilities:

- visual editing
- binding capture
- canvas manipulation
- live daemon control
- runtime status display

The GUI is a native editor with runtime awareness, not a thin JSON wrapper.

## 4. Current Runtime State Model

Phantom has these important runtime states:

- daemon running
- capture active
- mouse routed
- keyboard routed
- engine paused
- active layers

These states are separated deliberately.

Why:

- capture determines whether gameplay input should flow at all
- mouse routing determines whether mouse-originated events should reach the game
- pause determines whether the engine should emit touch commands

This separation is what makes the system usable instead of brittle.

## 5. Touch Model

The engine does not speak backend-specific events.

It emits:

- `TouchDown`
- `TouchMove`
- `TouchUp`

During runtime, Phantom now applies gameplay touch commands per translated input event instead of coalescing unrelated key transitions into one larger backend batch. That keeps release and re-engage boundaries explicit for controls like fixed joysticks. Joystick startup also uses an explicit commit boundary between `TouchDown` and the first `TouchMove` so visible sticks behave like a real drag rather than a teleported touch.

That abstraction is the key boundary in the codebase.

Backends then decide how to realize those commands:

- `android_socket` -> MotionEvents
- `uinput` -> kernel MT events

Two important recent consequences of that abstraction:

- `joystick` can now support both fixed-center and floating-zone behavior without changing backend semantics
- `drag` can model swipe games and sprint-lock style gestures without introducing a new transport contract

The engine stays responsible for gesture meaning. The backend stays responsible for touch realization.

## 6. `aim` Design

`aim` is the camera/look primitive.

It is not a general cursor.

Current modes:

- `always_on`
- `while_held`
- `toggle`

The engine stores:

- whether aim is enabled
- whether a synthetic finger is currently down
- current pointer position around the anchor
- last motion time

Why the enabled state exists:

- `always_on` should behave continuously
- `while_held` should behave like a temporary camera mode
- `toggle` should preserve mode state across movement pauses

Runtime note:

- aim still reacts immediately to mouse/touchpad movement
- absolute-touchpad translation now suppresses fresh-contact reseed jumps before
  the engine sees motion, and keeps tiny single-step motion available for held
  drags and careful cursor work
- touchpad contact start and end are now explicit engine-visible boundaries for
  aim, so repeated swipe contacts can lift and re-arm the hidden look touch
  cleanly instead of inheriting stale edge position
- the engine also keeps aim travel tighter around its anchor than the raw
  profile reach alone would suggest, so the hidden touch is less likely to roam
  into nearby controls

When capture is active and gameplay aim is inactive, the daemon runs a separate owned menu-touch path for menu navigation. That path is runtime-only and is not expressed as a profile node. When Phantom enters that mode it seeds its internal cursor from host cursor position if possible, then continues from Phantom-owned cursor state while the mouse remains captured. A tiny always-on-top cursor overlay is launched for that mode so the operator can see where the owned cursor is even though the desktop cursor itself is no longer moving.

Current menu-touch backend order:

- prefer Hyprland compositor-native cursor/client geometry for the initial seed
- then fall back to X11/XWayland helper mapping for the initial seed
- then fall back to Phantom's existing internal cursor position if no host seed is available

## 7. Why `android_socket` Is The Primary Backend

The earlier `uinput` path has one structural weakness for Waydroid:

- the host is forced to emulate a kernel multitouch device exactly the way Android expects to consume it later

That is fragile.

The Android backend avoids that by moving the final touch semantics into Android itself.

Benefits:

- Android owns pointer bookkeeping
- no kernel-device discovery requirement inside Waydroid
- much better multi-touch behavior for real games
- architecture closer to scrcpy-style injection

## 8. Why `app_process`

The Android server is launched with `app_process` because the project needs to execute inside Android's own runtime and call framework APIs directly.

That gives Phantom access to:

- `MotionEvent`
- `InputManager.injectInputEvent()`

without turning the project into a rooted system modification exercise.

## 9. Why TCP

The current host-to-container transport is TCP.

Reasons:

- easy to debug with normal tools
- explicit listener and connection lifecycle
- no dependence on a shared filesystem path contract
- good enough latency for this use case

The transport is deliberately simple because the interesting logic is not the framing. The interesting logic is the input state and Android injection semantics.

## 10. Why The Screen Contract Is Explicit

Phantom requires a known screen size because the mapper is not trying to guess layout transforms at runtime.

That decision removes an entire class of drift:

- wrong scaling
- wrong coordinate transforms
- silent mismatch between profile and runtime

## 11. Why `uinput` Still Exists

`uinput` is still worth keeping because:

- it provides a compatibility path
- it is useful for low-level comparison and debugging
- it keeps the abstract touch model honest

But project decisions should be driven by the Android backend first.

## 12. File Ownership By Concern

If you need to change:

- input capture: `phantom/src/input.rs`
- profile schema: `phantom/src/profile.rs`
- control semantics: `phantom/src/engine.rs`
- runtime commands: `phantom/src/ipc.rs` and `phantom/src/main.rs`
- Android backend transport: `phantom/src/android_inject.rs`
- Android launch behavior: `phantom/src/waydroid.rs`
- GUI editing or runtime widgets: `phantom-gui/src/main.rs`
- Android container server behavior: `contrib/android-server/src/com/phantom/server/PhantomServer.java`

## 13. Operational Boundaries

The architecture explicitly does not solve:

- automatic floating joystick discovery
- UI recognition
- sensor injection such as accelerometer tilt
- monitor transforms
- rotation transforms
- hotplug rescans
- generic automation workflows

Those are outside the intended system boundary.

## 14. Maintainability Rule

Any behavioral change that affects:

- startup order
- profile schema
- runtime state meaning
- backend protocol
- operator workflow

must update the docs in the same change.

That is not optional if the project is to stay maintainable.

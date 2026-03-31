# Total Scope

This document is the current product scope for Phantom after the runtime and GUI rebuild.

## 1. Product Definition

Phantom is a native fullscreen Waydroid gaming mapper for one Linux device and one known touch resolution.

It is not trying to be:

- a generic Android automation platform
- a desktop cursor remapper
- a compositor plugin
- a full Android emulator

That narrower shape is intentional.

## 2. Current Implemented Scope

### 2.1 Runtime

Implemented:

- evdev keyboard and mouse capture
- exclusive `EVIOCGRAB`
- uinput direct-touch injection
- fixed fullscreen screen contract
- live profile load from disk
- live in-memory profile push over IPC
- capture enter and exit at runtime
- pause and resume at runtime
- layer switching
- toggle touch nodes

Daemon control paths:

- CLI
- IPC
- GUI toolbar
- runtime hotkeys

Daemon hotkeys:

- `F8` toggles capture
- `F9` toggles pause

### 2.2 Supported Mapping Primitives

- `tap`
- `hold_tap`
- `toggle_tap`
- `joystick`
- `mouse_camera`
- `repeat_tap`
- `macro`
- `layer_shift`

In product language, `mouse_camera` is presented as `Mouse Look`.

### 2.3 GUI

The GUI is now a real mapper workflow, not just a JSON editor.

Implemented:

- native desktop app with `egui`
- open, create, save, save-as
- screenshot-first canvas
- placement tools for common control types
- direct drag editing for points
- direct drag and resize handles for mouse-look regions
- inline rename
- key capture by pressing keyboard or mouse input
- layer editing
- macro step editing
- runtime status panel
- `Push Live`
- capture buttons
- pause/resume buttons through daemon requests

### 2.4 Screen Contract

This is now enforced.

Rules:

- daemon startup requires a known screen size from config or default profile
- real profiles require a `screen` block
- profile load fails if the daemon `screen` and profile `screen` do not match

This is the right design for the target use case.

## 3. What The Product Can Realistically Do Now

For fixed-layout mobile games, Phantom can now cover the normal gameplay loop well.

### 3.1 PUBG-like Layouts

Supported well:

- WASD movement
- mouse look
- fire
- jump
- crouch
- reload
- repeat taps
- alternate control layers
- toggle actions

### 3.2 eFootball-like Layouts

Supported well:

- left stick movement
- pass
- through pass
- shoot
- switch
- sprint or pressure holds

### 3.3 General Rule

If the game control can be expressed as:

- a fixed tap
- a hold
- a toggle hold
- a fixed joystick
- a bounded swipe region
- a repeated tap
- a short scripted macro

then Phantom can map it.

## 4. What Is Still Hard Or Missing

### 4.1 Still Not Solved

- dynamic or floating joystick detection
- automatic UI recognition
- windowed-mode transform handling
- multi-monitor correctness
- rotation-aware remapping
- hotplug rescan for new keyboards or mice
- compositor-driven cursor hiding

### 4.2 Current Mouse Lock Reality

The gameplay lock model is now usable:

- capture on -> desktop loses the grabbed mouse and keyboard
- capture off -> desktop input returns

What is still not done:

- compositor-native cursor hiding
- a runtime overlay
- a separate lightweight gameplay HUD process

### 4.3 Current UI Gaps

The editor is much better now, but still not final-polish.

Still missing:

- zoom and pan on the canvas
- conflict visualization while editing
- better modifier-key capture for standalone left/right modifiers
- profile templates inside the app
- calibration wizard
- a richer preset browser

## 5. Why This Is Closer To Emulator Keymappers

GameLoop-style tools feel smooth because they own:

- the guest display size
- the render surface
- the capture lifecycle
- the mapper UI

Phantom still does not own the emulator itself, but it now covers the host-side qualities that matter most:

- deterministic resolution
- explicit capture and release
- live profile push
- mouse look
- layers
- toggles
- a real visual editor

That is the right equivalent for the Waydroid architecture.

## 6. Current Product Direction

Do not broaden the scope.

The correct direction is:

- fullscreen only
- one configured resolution
- native UI
- game-first mapping workflow
- strong runtime controls

That is how Phantom becomes a practical immersive tool instead of a permanently unfinished “general mapper.”

## 7. Next Roadmap

### P0: Finish The Fullscreen Gaming Loop

- add a dedicated overlay or HUD for capture state
- add better modifier-key capture and explicit binding search
- add conflict warnings in the editor
- add more complete starter profiles for PUBG and eFootball

### P1: Improve Placement And Tuning

- add zoom and pan
- add alignment guides and snapping
- add calibration workflow
- add richer macro editing presets

### P2: Runtime Polish

- add a lightweight launcher flow
- add better diagnostics when Waydroid does not expose the device
- add optional profile auto-push on selected edit operations instead of only save or button press

## 8. Bottom Line

Phantom is now on the right architecture for the stated goal:

- fullscreen Waydroid gaming
- locked screen size
- keyboard and mouse touch mapping
- live editor workflow

The remaining work is mostly polish, templates, and runtime UX. The core product shape is no longer the blocker.

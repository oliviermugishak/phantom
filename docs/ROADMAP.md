# Roadmap

This is the current product direction, not a promise of dates.

## Near Term

- keep capture, grab, and mouse-mode ownership explicit and testable
- keep Wayland helpers alive across `none` replies and sudo session inference
- keep packaged installs usable without a source checkout
- keep the GUI able to seed the user profile library from shipped starters
- type-while-captured, themed menu-touch cursor, and wheel swipe shipped in 1.1.0

## Next

- Android in-surface HUD so GNOME can see an owned cursor
- analog steering as a first-class node
- richer hotplug (remove/rebind mid-capture)

## Overlay Direction

The current host-side `F10` preview and menu-touch cursor overlay are experimental.

Preferred long-term direction:

- an Android-side in-surface overlay drawn in the same space the game sees
- host-side layer-shell HUDs remain a debug aid, not the gameplay surface

Known current limits:

- the cursor overlay is Wayland layer-shell only
- the legacy fullscreen preview can steal input
- compositor fullscreen rules can hide overlay surfaces

## Input Ownership

Still worth tightening:

- transactional capture transitions against the event loop
- device hotplug rescans
- richer recovery after repeated `SYN_DROPPED`
- a dedicated analog steering primitive

## Tactics Worth Adding Later

These are useful gameplay patterns that should become first-class only when they
need more than a composition of existing nodes:

- double-tap as an explicit node, if macros become too noisy
- held drag that stays down until key release
- analog steering-wheel / slider
- flick-stick look
- sensor/tilt injection, which is a separate subsystem

Until then, prefer the existing primitives documented in
[GAME_PATTERNS.md](GAME_PATTERNS.md).

## Explicitly Out Of Scope For Now

- UI recognition
- multi-monitor and rotation transforms
- generic desktop automation

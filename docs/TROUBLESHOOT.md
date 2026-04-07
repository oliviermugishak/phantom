# Troubleshoot

This document covers common operational problems and their direct fixes.

For product boundaries and missing features, see [EDGE_CASES.md](EDGE_CASES.md).

## GUI Does Not Show A New Shipped Profile

Symptom:

- a profile exists in the repository `profiles/` directory
- it does not appear in `phantom-gui`

Cause:

- the GUI reads `~/.config/phantom/profiles/`, not the repository directory

Fix:

```bash
./install.sh
phantom-gui
```

Why:

- `./install.sh` copies missing shipped profiles into the user profile library without overwriting existing user edits

## `F2` Works But `F1`, `F8`, Or `F10` Do Not

Symptom:

- shutdown works
- capture or mouse-toggle hotkeys do not
- overlay toggle does not work

Likely cause:

- Fn Lock is off

Fix:

- enable Fn Lock so the top row emits real `F1`, `F8`, `F9`, and `F10` function keys

## Capture Toggle Leaves Keys Stuck Or Keyboard Input Feels Broken

Symptoms:

- after `F8` or `phantom enter-capture` / `phantom exit-capture`, some keys appear stuck
- desktop input feels wrong until you restart the daemon
- held movement keys do not rebuild cleanly when capture is turned on

Current behavior:

- capture transitions now flush Phantom's desktop keyboard relay before ownership changes
- entering capture rebuilds currently held keyboard-driven hold controls from the real pressed-key state
- edge-trigger controls such as `toggle_tap`, `drag`, and `macro` are not replayed automatically on capture entry

If it still feels wrong:

- confirm you are running a current build
- use `phantom status` to verify `capture` and `mouse mode`
- if a problem happens right after a dropped-event warning, retest with `PHANTOM_TRACE_DETAIL=1` and inspect whether the affected device is producing repeated `SYN_DROPPED`

## `F10` Works But No Overlay Appears

Symptom:

- the daemon accepts `F10`
- no preview window appears

Important:

- the current `F10` preview is experimental
- on Wayland, Phantom first tries a compact passthrough HUD
- if that path cannot be used, it falls back to the older fullscreen preview window
- it is not an Android in-surface overlay

Checks:

- inspect `~/.config/phantom/overlay.log`
- verify the desktop session allows always-on-top fullscreen windows
- if the log says `neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set`, the overlay child was launched without a usable desktop display environment

If it still does not behave reliably:

- treat the overlay as unavailable on that desktop session
- use it only as a debug feature, not a gameplay feature

## Waydroid Is Running But Phantom Still Cannot Connect

Check:

```bash
sudo waydroid status
```

Bad state:

- `Session: RUNNING`
- `Container: FROZEN`

Good state:

- `Session: RUNNING`
- `Container: RUNNING`

Fix:

```bash
waydroid show-full-ui
```

or open the target game first so the container wakes up fully.

## Android Server Auto-Launch Times Out

Check:

```bash
sudo waydroid shell -- sh -c 'tail -n 100 /data/local/tmp/phantom-server.log'
```

Common causes:

- Waydroid was not running before Phantom started
- container is frozen
- Android jar path is wrong
- the jar is not a dex jar

Verify the jar contains `classes.dex`.

## Touches Land In The Wrong Place

Cause:

- screen contract mismatch

Check:

- daemon screen in `phantom status`
- profile screen in `phantom audit`
- actual Waydroid surface size

Fix:

- align the profile and daemon screen contracts with the real fullscreen Android surface

## Menu-Touch Cursor Does Not Appear

Symptom:

- capture is on
- mouse mode is `menu_touch`
- clicks may still work, but you do not see the owned cursor

Checks:

- inspect `~/.config/phantom/cursor-overlay.log`
- confirm `phantom status` shows:
  - `capture: true`
  - `mouse mode: menu_touch`
- verify the desktop session allows small always-on-top transparent windows
- on Wayland/Hyprland, the cursor overlay now expects layer-shell support rather than a normal transparent toplevel

Important:

- this cursor overlay is separate from the `F10` debug preview
- it exists only to visualize Phantom's owned menu-touch cursor while capture is active

## Aim Does Not Work

Check:

- mouse routing is enabled
- capture is enabled
- `aim` anchor and reach are reasonable
- `aim` activation mode matches the intended workflow
- activation key is present if mode is `while_held` or `toggle`
- the loaded profile actually contains an `aim` node

Useful cases:

- start with `always_on` to prove the aim node itself is correct
- then switch to `while_held` or `toggle`

Note:

- touchpads now work, but they may still feel less smooth than a real mouse because Phantom must derive relative motion from absolute touchpad coordinates
- Phantom now suppresses fresh-contact touchpad jumps before motion reaches aim and keeps tiny single-step movement available for held drags and careful cursor movement
- a real mouse is still the best path for the highest-end fast aim-heavy play, but touchpad behavior should now be less jumpy without adding tick-latency to aim
- this is intentional: Phantom does not add extra smoothing to the real relative-mouse path, because that would trade aim feel for latency
- if real-mouse aim feels jumpy, verify you are on the current build; Phantom
  now handles relative mouse motion one evdev report at a time, with X/Y from
  the same report kept together instead of being emitted as separate aim jumps
- the current mouse path also damps tiny relative reports instead of applying the
  same full scale to every movement, which improves precision without turning
  large camera sweeps into a slow drag
- that shaping is now per axis, so a fast vertical recoil pull should not
  magnify tiny accidental left/right noise into a sideways camera snap

## Aim Stops After `F1` Mouse Toggle

Expected behavior now:

- `F1` lifts the active aim finger
- `toggle` aim stays enabled across the routing change
- `while_held` aim is resynced from the real current mouse-button state when routing is re-enabled

If it still feels wrong:

- check whether the profile uses `toggle` or `while_held`
- verify the activation key is a real mouse button such as `MouseRight`
- test with a real mouse to separate touchpad-feel issues from routing-state issues
- for touchpad play, keep the aim anchor in clear space; Phantom now re-arms aim
  between touch contacts, but a real mouse is still the better hardware path

## Menus Need Touch Instead Of Raw Mouse

Some games accept taps and drags in menus but ignore plain desktop mouse input.

Use this workflow:

- enter capture
- stay in menu-touch mode instead of switching to gameplay aim
- navigate with left click and drag

Expected behavior:

- left click becomes touch down / up
- moving while held becomes drag
- touchpad single-tap now synthesizes a left click in owned menu-touch
- touchpad double-tap-and-hold now begins a held left click so drag can continue from the pad
- `F1` switches back to gameplay aim when needed
- on touchpads, the first contact on the pad should seed cleanly instead of jerking the owned cursor

## Menu Touch Lands Away From The Visible Cursor

Check:

- `phantom status` while capture is active and mouse mode is `menu_touch`
- look for `menu touch backend`

Expected:

- `owned-hyprland-seeded+x11-seeded+virtual` means Phantom seeded the owned cursor from compositor-native Hyprland data
- `owned-x11-seeded+virtual` means Phantom seeded the owned cursor from X11/XWayland helper data
- `owned-virtual` means Phantom had no exact host seed and reused its internal cursor

What it means:

- Phantom only depends on the host cursor for the initial seed when menu-touch mode begins
- after that seed, the owned Phantom cursor is moved from raw mouse motion while the mouse stays captured

If you still see drift:

- enter menu-touch while the visible host cursor is already over the area you want to start from
- test from the same desktop session where Hyprland or `DISPLAY` helper data is available
- use `phantom status` to verify the backend instead of assuming

## Menu Touch Needs Two Clicks Before The Action Happens

Check:

- `phantom status` while capture is active and mouse mode is `menu_touch`
- look for:
  - `mouse mode`
  - `menu touch backend`

What it means:

- `mouse mode: menu_touch`
  - Phantom owns the mouse and should inject touch directly
- `mouse mode: aim`
  - clicks are being routed through gameplay aim semantics instead of menu touch

Important:

- this is separate from cursor accuracy
- Phantom no longer relies on a first host click for activation in the main menu-touch path
- if you still see a two-click pattern, the remaining issue is likely game-specific UI behavior rather than desktop focus preparation

## PUBG Sprint-Lock Drag Does Not Feel Right

Check:

- drag `start`
- drag `end`
- `duration_ms`

Fix:

- lower `duration_ms` for a faster snap
- adjust the drag end point to the exact sprint-lock point in your on-screen layout

## Temple Run Or Subway Surfers Swipe Feels Weak

Check:

- `duration_ms`
- swipe start point
- swipe end point

Fix:

- use a short drag duration, usually around `70-100 ms`
- move the swipe farther if the game wants a more deliberate gesture

## Temple Run Tilt Does Not Work

This is not a profile bug.

Reason:

- Phantom injects touch
- tilt is accelerometer input

Current status:

- tilt is unsupported

## New Profile Loads But The Game Still Ignores It

Check:

- the profile actually loaded in `phantom status`
- the profile screen matches the daemon screen
- the on-screen coordinates are from the same layout the game is currently using

If a layout was moved in-game, the profile must be updated too.

## Desktop Interaction Feels Broken After Starting The Daemon

Check:

- capture state
- mouse routing state

Use:

```bash
phantom exit-capture
phantom release-mouse
```

Remember:

- Phantom reserves daemon hotkeys
- gameplay capture should be the state that routes gameplay, not your normal desktop workflow

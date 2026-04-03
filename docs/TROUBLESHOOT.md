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

## `F10` Works But No Overlay Appears

Symptom:

- the daemon accepts `F10`
- no preview window appears

Important:

- the current `F10` overlay is an experimental host-side debug window
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
- Phantom now filters large touchpad re-anchor jumps and preserves aim mode across `F1`, but a real mouse is still the best path for fast aim-heavy games

## Aim Stops After `F1` Mouse Toggle

Expected behavior now:

- `F1` lifts the active aim finger
- `toggle` aim stays enabled across the routing change
- `while_held` aim is resynced from the real current mouse-button state when routing is re-enabled

If it still feels wrong:

- check whether the profile uses `toggle` or `while_held`
- verify the activation key is a real mouse button such as `MouseRight`
- test with a real mouse to separate touchpad-feel issues from routing-state issues

## Menus Need Touch Instead Of Raw Mouse

Some games accept taps and drags in menus but ignore plain desktop mouse input.

Use this workflow:

- enter capture
- leave the mouse released instead of grabbing gameplay aim
- navigate with left click and drag

Expected behavior:

- left click becomes touch down / up
- moving while held becomes drag
- `F1` grabs the mouse back for gameplay aim when needed

## Menu Touch Lands Away From The Visible Cursor

Check:

- `phantom status` while capture is active and the mouse is released
- look for `menu touch backend`

Expected:

- `hyprland-client-absolute+x11-helper+virtual-fallback` is the accurate compositor-native path on Hyprland
- `x11-helper+virtual-fallback` is the accurate host-cursor path
- `virtual-cursor` is the fallback path and can still drift from the visible desktop cursor

What it means:

- on X11/XWayland, Phantom can query the real visible cursor and map touches exactly to the active host window
- on sessions where exact cursor position is not available, Phantom falls back to its internal cursor path

If you still see drift:

- confirm the game window is actually the active X11/XWayland window
- test from the same desktop session where `DISPLAY` is available
- use `phantom status` to verify the backend instead of assuming

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

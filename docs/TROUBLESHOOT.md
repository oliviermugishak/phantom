# Troubleshoot

These are operational failures, not missing product features. For "Phantom will
never do X", see [EDGE_CASES.md](EDGE_CASES.md). For how a healthy session is
supposed to look, see [OPERATIONS.md](OPERATIONS.md).

## Why is the GUI missing a profile that exists in the repo?

The GUI reads `~/.config/phantom/profiles/`, not `./profiles/`.

On first launch it copies missing shipped files into that directory. If it
still looks empty, use Settings → "Seed shipped profiles", or rerun
`./install.sh`. Neither path overwrites a profile you already edited.

Restart the GUI after seeding. It reloads the user library at startup.

## Why does `F2` work but `F1`, `F8`, or `F10` do nothing?

The laptop top row is probably sending media keys. Enable Fn Lock so those
keys emit real function-key events.

`F2` often still works because some firmware always sends it as a function
key. That pattern is a firmware issue, not a dead Phantom binding.

## Why are keys stuck after I toggle capture?

Capture transitions flush the desktop keyboard relay, then rebuild hold-style
controls from the real pressed-key set when you enter capture. Leaving capture
replays currently held keys back to the desktop.

If something is still stuck:

- confirm you are on a current build
- check `phantom status` for `capture` and `mouse mode`
- if a `SYN_DROPPED` warning happened just before it, rerun with
  `PHANTOM_TRACE_DETAIL=1` and see whether that device is dropping events
  repeatedly

Edge-trigger nodes (`toggle_tap`, `drag`, `macro`) are not replayed on capture
entry. That is intentional.

## Why does `F10` accept the hotkey but show nothing?

`F10` is an experimental host preview, not an Android overlay. On Wayland it
tries a compact layer-shell HUD; otherwise it falls back to a fullscreen
window. Either path can be hidden or rejected by the compositor.

Read `~/.config/phantom/overlay.log`. If it says neither `WAYLAND_DISPLAY` nor
`DISPLAY` is set, the daemon was not started from the graphical session with
`sudo -E`.

If the preview is unreliable on that desktop, treat it as unavailable. Do not
play through it.

## Why can't Phantom connect when Waydroid looks running?

```bash
sudo waydroid status
```

`Session: RUNNING` plus `Container: FROZEN` is the bad state. Open
`waydroid show-full-ui` or the game itself so the container thaws.

You want both session and container `RUNNING` before `phantom --daemon`.

## Why does Android auto-launch time out?

```bash
sudo waydroid shell -- sh -c 'tail -n 100 /data/local/tmp/phantom-server.log'
```

Usual causes: Waydroid was down when Phantom started, the container is frozen,
the jar path is wrong, or the jar is not a dex jar. The file must contain
`classes.dex`.

Packaged installs should resolve `/usr/lib/phantom/phantom-server.jar` or
`../lib/phantom/` next to the binary without a source checkout.

## Why do touches land in the wrong place?

The profile screen and the daemon screen must match the real fullscreen
Android surface. Phantom will not invent a transform.

Compare `phantom status`, `phantom audit <profile>`, and
`waydroid prop get persist.waydroid.width` / `height`. Fix the contract, then
reload the profile.

## Why is there no owned cursor in menu-touch?

Clicks can still work. The sprite is a separate Wayland layer-shell overlay.

- Hyprland / Sway / niri / KDE: read `~/.config/phantom/cursor-overlay.log`
- GNOME: no wlr-layer-shell, so no host cursor. This is a known limit.
- X11: no cursor overlay. Injection still works.
- any compositor: start the daemon with `sudo -E` from the graphical session

`phantom status` should show `capture: true` and `mouse mode: menu_touch`.
This overlay is not the `F10` preview.

## Why doesn't aim move the camera?

Capture must be on, mouse mode must be `aim` (`F1`), and the loaded profile
must contain an `aim` node. `F1` alone is not camera movement.

Start with `always_on` to prove the node, then switch to `while_held` or
`toggle`. Use a real mouse for shooter work. Touchpad aim is derived from
absolute pad coordinates and will always feel like a fallback.

## Why did aim stop after I pressed `F1`?

`F1` lifts the look finger and switches to menu-touch. Toggle-look stays
armed. `while_held` is resynced from the real mouse-button state when you
switch back to aim.

If it still feels wrong, check whether the profile is `toggle` or `while_held`,
and whether the activation key is actually `MouseRight` (or whatever you bound).

## How do I navigate menus that ignore the desktop mouse?

Enter capture and stay in menu-touch. Left click is a tap. Drag while held is
a drag. Wheel is a short swipe. Unused keys type into Android.

`F1` is how you go back to gameplay aim.

## Why does menu-touch miss the visible cursor?

Check `menu touch backend` in `phantom status`:

- `owned-hyprland-seeded+virtual` — seeded from Hyprland
- `owned-x11-seeded+virtual` — seeded from X11 / XWayland
- `owned-virtual` — no host seed; Phantom reused its last internal point

Enter capture while the host pointer is already over the Waydroid window.
After that seed, Phantom drives its own cursor from raw mouse motion.

## Why do I need two clicks in a menu?

If `mouse mode` is `aim`, you are not in menu-touch. Press `F1`.

If you are already in menu-touch and still see a two-click pattern, that is
usually the game UI, not Phantom waiting for window focus.

## Why does a PUBG sprint-lock or runner swipe feel weak?

That is almost always the `drag` geometry, not the backend. Shorten
`duration_ms` (often 70–100 ms) and move `end` to the exact on-screen target.

Temple Run tilt-to-collect is not a drag problem. Tilt is accelerometer
input. Phantom does not inject sensors.

## Why did a new profile load but the game ignore it?

Confirm `phantom status` shows that profile, the screen matches, and the
in-game layout has not been moved since you authored the coordinates.

## Why does the desktop feel broken after I start the daemon?

You are probably still in capture. Leave it:

```bash
phantom exit-capture
```

The daemon keeps the keyboard grabbed for hotkeys even with capture off, and
relays typing to the desktop. Gameplay capture is not a desktop workflow.

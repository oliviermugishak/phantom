# Phantom

Phantom maps a Linux keyboard and mouse onto Android touch for fullscreen
[Waydroid](https://waydro.id/). It is for people who want to play Android games
from a desk without pretending the desktop pointer is a finger.

The host captures `evdev` input, a JSON profile decides what those events mean,
and an Android-side `app_process` server injects `MotionEvent`s (and, when
capture is on, unused-key `KeyEvent`s) inside the container.

- Source: https://github.com/oliviermugishak/phantom
- Author: Olivier Mugisha K ([@oliviermugishak](https://github.com/oliviermugishak))

## What problem does this solve?

Android games expect fingers. A Linux desktop gives you keys and a mouse.
Phantom sits between them.

It does **not** try to recognize UI, guess screen transforms, or inject
accelerometer tilt. It does one job: turn explicit bindings into predictable
touches on a known Android surface.

The recommended backend is `touch_backend = "android_socket"`. The older
`uinput` path still exists as a fallback.

## What do I need before I start?

- Linux, preferably a Wayland session
- Waydroid, already able to show a fullscreen Android UI
- access to `/dev/input/event*` (root via `sudo -E`, or your user in group `input`)
- a known Waydroid screen size written into `~/.config/phantom/config.toml`

You only need `/dev/uinput` if you use the legacy `uinput` backend.

Hyprland, Sway, niri, and KDE are the compositors where the owned cursor overlay
works. GNOME can still inject menu-touch, but it will not show that overlay.
A real mouse is still the right hardware for shooter aim. Touchpad aim is
best-effort.

## Who runs the daemon, and who runs the GUI?

These are two different processes. Mixing their privileges is the usual way
people break a first install.

| Process | Who starts it | Privilege |
|---|---|---|
| `phantom --daemon` | you, from the **graphical** session | `sudo -E`, or unprivileged after udev + group `input` |
| `phantom-gui` | you, as the desktop user | never sudo |
| `phantom status`, `load`, `enter-capture` | desktop user | talks to `$XDG_RUNTIME_DIR/phantom.sock` |

`sudo -E` matters. Plain `sudo` or `pkexec` drops `WAYLAND_DISPLAY`,
`XDG_RUNTIME_DIR`, and cursor-theme variables. Then the overlay cannot see the
compositor and the owned cursor disappears.

Do not enable the system systemd unit as your daily driver. Waydroid is a user
session. After udev is installed and you are in group `input`, the optional
user unit at `/usr/lib/systemd/user/phantom.service` is the only systemd path
that matches how Phantom actually works.

AppImage is a GUI plus a portable jar. It cannot start the privileged daemon
from its FUSE mount. Use a `.deb`, Arch package, tarball, or `./install.sh`
for the daemon.

## How do I install it?

**From source**, after you have Rust and Android SDK command-line tools:

```bash
git clone https://github.com/oliviermugishak/phantom.git
cd phantom
./install.sh
```

That puts `phantom` and `phantom-gui` in `~/.local/bin`, the Android jar in
`~/.local/share/phantom/android/`, a sudo-visible launcher in `/usr/local/bin`
when possible, and seeds missing profiles into `~/.config/phantom/profiles/`.

**From a GitHub Release**, pick the asset that matches how you install software:

- Debian / Ubuntu: `phantom_<version>_amd64.deb`
- Arch: `phantom-<version>-*.pkg.tar.zst`, or the in-tree AUR recipe `phantom-bin`
- portable: `phantom-v<version>-linux-x86_64.tar.gz`
- GUI tryout only: the AppImage

Packaged installs already contain `phantom-server.jar`. You do not build it.
The daemon finds it under `/usr/lib/phantom/` or `../lib/phantom/` next to the
binary. The first GUI or daemon start writes `~/.config/phantom/config.toml`
only if that file is missing, and copies shipped profiles into the user library
the same way.

Full setup, including udev and Android SDK, is in [docs/INSTALL.md](docs/INSTALL.md).

## How do I start a session?

1. Start Waydroid and open the UI:

```bash
waydroid session start
waydroid show-full-ui
sudo waydroid status
```

You want `Session: RUNNING` and `Container: RUNNING`. If the container is
`FROZEN`, open the game first. A frozen container is the most common
"Phantom cannot connect" report.

2. Set `[screen]` in `~/.config/phantom/config.toml` to the real Waydroid
   surface. `waydroid prop get persist.waydroid.width` and `...height` are the
   usual source of truth.

3. Start the daemon from the same graphical session:

```bash
sudo -E phantom --daemon
```

Add `--trace` when you need lifecycle logs. Use
`PHANTOM_TRACE_DETAIL=1` only when you are chasing raw evdev bugs.

4. In a normal user shell:

```bash
phantom status
phantom load ~/.config/phantom/profiles/pubg.json
phantom-gui
```

Push Live from the GUI, then enter capture.

## What do capture, aim, and menu-touch actually mean?

They are independent on purpose.

- **Daemon running** means the process is alive and holding the keyboard for
  hotkeys. Capture can be off. Your desktop should still type, because Phantom
  relays the keyboard through a virtual "Phantom Desktop Keyboard".
- **Capture on** (`F8`) means gameplay input is owned. The mouse is grabbed.
  You start in **menu-touch**: left click is a finger, drag is a drag, wheel is
  a short swipe, and unused keys type into Android.
- **Aim** (`F1`, while capture is on) means the owned mouse feeds the profile's
  `aim` node. No `aim` node means `F1` will not magically look around.
- **Pause** (`F9`) freezes touch output without tearing down grabs.

`F10` is an experimental host-side preview of the current profile. It is not
an in-game overlay and it is not for playing. `F2` shuts the daemon down.

On many laptops the top row only emits real `F1`/`F8`/`F10` when Fn Lock is on.
If `F2` works and the others do not, check that first.

## Where do profiles live, and why is the GUI empty?

There are two directories:

- shipped starters in the repo or package: `profiles/` or `/usr/share/phantom/profiles/`
- the library the GUI actually reads: `~/.config/phantom/profiles/`

The GUI never live-reads the repository. First launch, Settings → "Seed shipped
profiles", or `./install.sh` copies missing files into the user library and
does not overwrite edits.

Shipped starters are layouts, not finished configs for every device:

`pubg.json`, `pubg-small.json`, `genshin.json`, `efootball-template.json`,
`temple-run.json`, `subway-surfers.json`, `asphalt8.json`, `asphalt9.json`

A profile is rejected if its `screen` does not match the daemon screen. That
is intentional. Phantom will not guess a transform.

How to author nodes is in [docs/PROFILES.md](docs/PROFILES.md). How to structure
a shooter or a swipe runner is in [docs/GAME_PATTERNS.md](docs/GAME_PATTERNS.md).

## What can I bind, and what should I not expect?

Profile primitives: `tap`, `toggle_tap`, `joystick`, `drag`, `aim`,
`repeat_tap`, `wheel`, `macro`, `layer_shift`. Legacy `hold_tap` and
`mouse_camera` still load and are normalized.

Useful compositions you do not need a new node type for:

- ADS look: `aim` `while_held` on `MouseRight`
- sprint lock: `drag` from the stick center to the lock point
- turbo fire: `repeat_tap`
- vehicle remaps: `layer_shift` with `suspend_base`

Phantom injects touch (and unused Android keys while captured). It does not
inject tilt, gyro, or a dedicated analog steering wheel. If the game has no
touch alternative for a sensor, that is outside this project.

## Why does the owned cursor vanish, or land in the wrong place?

On Hyprland the overlay is a layer-shell arrow taken from your Xcursor theme.
It hides after five seconds of no movement. GNOME has no wlr-layer-shell, so
there is no host cursor there. X11 has no cursor overlay either. Injection
still works.

If the overlay is missing on a compositor that should support it, start the
daemon with `sudo -E` and read `~/.config/phantom/cursor-overlay.log`. If
menu-touch hits the wrong place, enter capture while the host pointer is
already over the Waydroid window, then check `phantom status` for
`menu touch backend`.

## Which command do I run for what?

```bash
phantom --daemon          # start the mapper; use sudo -E from a desktop session
phantom status            # capture, mouse mode, grab, menu-touch backend
phantom load <profile>    # replace the live profile
phantom enter-capture     # same as F8 on
phantom exit-capture      # same as F8 off
phantom toggle-mouse      # same as F1: aim <-> menu-touch
phantom audit <profile>   # validate without loading
phantom-gui               # editor and runtime controls
```

`grab-mouse` / `release-mouse` are the older names for switching aim and
menu-touch. They do not ungrab the physical mouse while capture is on.

## Where should I read next?

Operator path:

1. [docs/INSTALL.md](docs/INSTALL.md)
2. [docs/OPERATIONS.md](docs/OPERATIONS.md)
3. [docs/TROUBLESHOOT.md](docs/TROUBLESHOOT.md)
4. [docs/PROFILES.md](docs/PROFILES.md)
5. [docs/GAME_PATTERNS.md](docs/GAME_PATTERNS.md)

Maintainer path:

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/TESTING.md](docs/TESTING.md)
- [docs/RELEASING.md](docs/RELEASING.md)
- [docs/ROADMAP.md](docs/ROADMAP.md)
- [AGENTS.md](AGENTS.md) and [CONTRIBUTING.md](CONTRIBUTING.md)

The project stays explicit: known screen, named runtime states, deterministic
profiles, Android-first injection. Convenience never wins if it hides those.

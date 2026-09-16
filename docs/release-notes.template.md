# Phantom x.y.z

## Who this is for

Linux + Waydroid users who want keyboard/mouse mapped to Android touch.

## What to download

- Arch: `.pkg.tar.zst` or a future AUR `phantom-bin`
- Debian/Ubuntu: `.deb`
- Portable: `.tar.gz`
- GUI-only tryout: AppImage (the privileged daemon still needs a real install)

## First run

1. `waydroid session start && waydroid show-full-ui`
2. `sudo -E phantom --daemon` from the graphical session, or `phantom --daemon` after udev + group `input`
3. `phantom-gui` as your desktop user, never with sudo
4. Push Live → Enter Capture → `F1` for aim

## Android server

The jar is inside the package. You do not build it. The daemon copies it into Waydroid on first connect.

## Highlights

- 

## Packaging / install

- 

## Breaking changes

- 

## Upgrade notes

- Add your user to group `input` and reload udev if you want an unprivileged daemon.
- First GUI/daemon launch writes `~/.config/phantom/config.toml` only if it is missing.
- AppImage cannot Start Daemon from its FUSE mount.

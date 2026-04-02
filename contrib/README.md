# contrib

This directory contains support assets for packaging, installation, and environment integration.

These files are not the main runtime logic, but they matter for making Phantom usable on real systems.

Contents:

- `android-server/`
  Android-side server source and build script for the primary `android_socket` backend.
- `99-phantom.rules`
  Example `udev` rules for input and `uinput` device access.
- `phantom.service`
  Example `systemd` service unit for the daemon.
- `waydroid/`
  Waydroid-specific integration assets such as the touchscreen IDC file.

## What Belongs Here

Use `contrib/` for:

- optional system integration assets
- packaging helpers
- installation examples
- platform-specific support files that are not core Rust code

Do not use it for:

- primary product documentation
- generated build artifacts
- random design notes

## Android Server

The most important subdirectory is:

- [android-server/](android-server/)

That subproject is the Android-side injection server used by the primary backend.

Start here for details:

- [android-server/README.md](android-server/README.md)

## Integration Assets

### `99-phantom.rules`

Use this if you want a cleaner non-root permission model for input and `uinput` access on compatible systems.

### `phantom.service`

Use this as a starting point if you want to manage Phantom through `systemd`.

Treat it as an example, not a universal default. Review paths, privileges, and service ordering for your machine.

### `waydroid/`

Waydroid-specific support material lives here. The included IDC file helps Android classify Phantom’s virtual device correctly when the relevant path is used.

## Build Artifacts

Generated outputs do not belong in version control.

In particular:

- `contrib/android-server/build/` is build output
- the source of truth is the Java source plus `build.sh`

If you need the Android jar, build it locally or use `./install.sh`.

## Relationship To The Rest Of The Repo

- root product docs live in [../README.md](../README.md) and [../docs/](../docs/)
- daemon/runtime code lives in [../phantom/](../phantom/)
- editor/runtime control code lives in [../phantom-gui/](../phantom-gui/)

# Releasing Phantom

Phantom now ships release automation through GitHub Actions and GitHub Releases.

## Release Trigger

Releases are tag-driven.

Use this flow:

1. bump the workspace version in `Cargo.toml`
2. update `CHANGELOG.md`
3. commit the release changes
4. create and push a tag such as `v1.0.0`

The release workflow verifies that the pushed tag matches the workspace version.

## Release Assets

The release workflow publishes these assets to GitHub Releases:

- `phantom-v<version>-linux-x86_64.tar.gz`
- `phantom_<version>_amd64.deb`
- `phantom-<version>-<pkgrel>-x86_64.pkg.tar.zst`
- `phantom-v<version>-linux-x86_64.AppImage`
- `SHA256SUMS`

The same staged install tree is used for every package format so the shipped
contents remain consistent.

## Staged Layout

The package staging script installs:

- `usr/bin/phantom`
- `usr/bin/phantom-gui`
- `usr/lib/phantom/phantom-server.jar`
- `usr/share/phantom/config.example.toml`
- `usr/share/phantom/profiles/*.json`
- `usr/share/phantom/contrib/*`
- `usr/share/doc/phantom/*`
- `usr/share/licenses/phantom/LICENSE`

Important runtime detail:

- Phantom now auto-resolves the Android server jar from:
  - the configured `android.server_jar`
  - the user install under `~/.local/share/phantom/android/`
  - `../lib/phantom/phantom-server.jar` relative to the running binary
  - `/usr/lib/phantom/phantom-server.jar`
  - a source-tree build under `contrib/android-server/build/`

That is what makes packaged installs and release tarballs work without a
source checkout.

## Local Packaging

Build the shared staged tree:

```bash
bash packaging/stage.sh
```

Build the generic tarball:

```bash
bash packaging/build-tarball.sh
```

Build the Debian package:

```bash
bash packaging/build-deb.sh
```

Build the Arch package on an Arch system or container as a non-root user:

```bash
bash packaging/build-arch-package.sh
```

Build the AppImage after making `linuxdeploy`, `appimagetool`, and the GTK
plugin available on `PATH`:

```bash
bash packaging/build-appimage.sh
```

Write release checksums:

```bash
bash packaging/write-checksums.sh
```

## Arch Distribution

There are two distinct Arch delivery paths:

1. AUR package
2. your own pacman repository

They are not the same.

### AUR

The AUR hosts package recipes, not your built binaries directly.

Typical path:

1. publish the GitHub release assets
2. adapt `packaging/arch/PKGBUILD.in` into an AUR package such as `phantom-bin`
3. point its source URL at the GitHub release tarball
4. generate `.SRCINFO`
5. publish the AUR git repo

### Custom pacman repo

If you want direct `pacman -S phantom`, build the Arch package and generate a
repository database with:

```bash
bash packaging/arch/repo-add.sh phantom /path/to/repo
```

Then host that directory over HTTPS and document the repository stanza users
must add to `/etc/pacman.conf`.

## CI

The CI workflow validates:

- `cargo fmt --all -- --check`
- `cargo test --quiet`
- `cargo clippy --quiet --all-targets --all-features -- -D warnings`
- `cargo build --release --quiet`
- `./contrib/android-server/build.sh`

The release workflow reruns those checks before packaging and publishing.

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
- `usr/lib/udev/rules.d/99-phantom.rules`
- `usr/lib/systemd/user/phantom.service`
- `usr/share/phantom/contrib/phantom.service` (system-unit example only)

Do not enable the system unit as the product default. Daily use is
`sudo -E phantom --daemon` from a graphical session, or the user unit after
udev and group `input`.

The GUI and daemon write `~/.config/phantom/config.toml` from the shipped
example only if that file is missing. The GUI also seeds missing shipped
profiles into `~/.config/phantom/profiles/` on first launch. Packaged
installs do not need `./install.sh` for a first-run library.

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

The in-tree recipe is `packaging/aur/PKGBUILD` (`phantom-bin`). It is not
published from this repository. After a GitHub release exists, fill the real
tarball sha256, regenerate `.SRCINFO`, and push that recipe to the AUR by
hand.

`packaging/arch/PKGBUILD.in` is only for the GitHub `.pkg.tar.zst` asset. Do
not confuse the two.

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

The CI workflow also builds a tarball and Debian package and runs
`packaging/smoke-test.sh`.

Android SDK setup uses `android-actions/setup-android@v4` with
`packages: platform-tools`. Do not request the obsolete `tools` package;
current `sdkmanager` no longer ships it.

CI and release are separate workflows on purpose.

- CI runs on pull requests and branch pushes. It does not run on `v*` tags.
- Release runs only when you push a `v*` tag. It rebuilds, packages, and
  publishes. It does not wait for the CI workflow.

That means: push the commit, wait for CI to go green, then tag. Do not tag a
commit whose branch CI has not passed.

The release workflow reruns fmt/test/clippy/build before packaging so a tag
cannot publish an untested tree even if someone skips that wait.

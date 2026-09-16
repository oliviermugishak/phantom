#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$SCRIPT_DIR/common.sh"

fail() {
    printf 'smoke-test: %s\n' "$*" >&2
    exit 1
}

stage_root="$(stage_root_dir)"
if [[ ! -d "$stage_root" ]]; then
    bash "$SCRIPT_DIR/stage.sh" --skip-build >/dev/null
fi

[[ -x "$stage_root/usr/bin/phantom" ]] || fail "missing staged phantom"
[[ -x "$stage_root/usr/bin/phantom-gui" ]] || fail "missing staged phantom-gui"
[[ -f "$stage_root/usr/lib/phantom/phantom-server.jar" ]] || fail "missing staged jar"
[[ -f "$stage_root/usr/share/phantom/config.example.toml" ]] || fail "missing example config"
[[ -d "$stage_root/usr/share/phantom/profiles" ]] || fail "missing staged profiles"
[[ -f "$stage_root/usr/share/applications/phantom-gui.desktop" ]] || fail "missing desktop file"
[[ -f "$stage_root/usr/lib/udev/rules.d/99-phantom.rules" ]] || fail "missing udev rules"

contains() {
    printf '%s\n' "$1" | grep -q -- "$2"
}

tarball="$(release_dir)/$(tarball_asset_name)"
if [[ -f "$tarball" ]]; then
    # Read the listing once. `tar | grep -q` under `pipefail` is a false
    # negative: grep exits early, tar hits EPIPE, the pipeline fails.
    tarball_list="$(tar -tzf "$tarball")"
    contains "$tarball_list" '/bin/phantom$' || fail "tarball missing bin/phantom"
    contains "$tarball_list" '/lib/phantom/phantom-server.jar$' || fail "tarball missing jar"
    contains "$tarball_list" '/share/phantom/' || fail "tarball missing share/phantom"
fi

deb="$(release_dir)/$(deb_asset_name)"
if [[ -f "$deb" ]]; then
    control="$(dpkg-deb -f "$deb" 2>/dev/null || true)"
    contains "$control" '^Recommends: waydroid' || fail "deb missing Recommends: waydroid"
    contains "$control" 'libasound2t64 | libasound2' || fail "deb missing t64 OR-depends for alsa"
    contains "$control" 'libgtk-3-0t64 | libgtk-3-0' || fail "deb missing t64 OR-depends for gtk"
    description="$(printf '%s\n' "$control" | sed -n '/^Description:/,/^[^ ]/p')"
    if contains "$description" 'Recommends:'; then
        fail "Recommends leaked into Description"
    fi
fi

appimage="$(release_dir)/$(appimage_asset_name)"
if [[ -f "$appimage" ]]; then
    extract_dir="$(mktemp -d)"
    trap 'rm -rf "$extract_dir"' EXIT
    if command -v "$appimage" >/dev/null 2>&1; then
        (
            cd "$extract_dir"
            APPIMAGE_EXTRACT_AND_RUN=1 "$appimage" --appimage-extract >/dev/null 2>&1 || true
        )
    fi
    if [[ -f "$extract_dir/squashfs-root/AppRun" ]]; then
        grep -q 'phantom)' "$extract_dir/squashfs-root/AppRun" || fail "AppRun lost daemon routing"
        grep -q -- '--daemon' "$extract_dir/squashfs-root/AppRun" || fail "AppRun lost --daemon routing"
    fi
fi

printf 'smoke-test: ok\n'

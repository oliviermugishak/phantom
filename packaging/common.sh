#!/usr/bin/env bash
set -euo pipefail

PACKAGING_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PACKAGING_DIR/.." && pwd)"
DIST_DIR="${DIST_DIR:-$REPO_ROOT/dist}"
PACKAGE_ITERATION="${PACKAGE_ITERATION:-1}"

workspace_version() {
    awk '
        /^\[workspace\.package\]$/ { in_pkg = 1; next }
        /^\[/ { in_pkg = 0 }
        in_pkg && /^[[:space:]]*version[[:space:]]*=/ {
            line = $0
            sub(/^[^"]*"/, "", line)
            sub(/".*$/, "", line)
            print line
            exit
        }
    ' "$REPO_ROOT/Cargo.toml"
}

host_arch() {
    uname -m
}

deb_arch() {
    case "$(host_arch)" in
        x86_64) printf 'amd64\n' ;;
        aarch64) printf 'arm64\n' ;;
        *)
            printf 'unsupported Debian architecture: %s\n' "$(host_arch)" >&2
            exit 1
            ;;
    esac
}

appimage_arch() {
    case "$(host_arch)" in
        x86_64) printf 'x86_64\n' ;;
        aarch64) printf 'aarch64\n' ;;
        *)
            printf 'unsupported AppImage architecture: %s\n' "$(host_arch)" >&2
            exit 1
            ;;
    esac
}

stage_root_dir() {
    printf '%s\n' "$DIST_DIR/stage/root"
}

portable_root_name() {
    printf 'phantom-v%s-linux-%s\n' "$(workspace_version)" "$(host_arch)"
}

portable_root_dir() {
    printf '%s\n' "$DIST_DIR/portable/$(portable_root_name)"
}

release_dir() {
    printf '%s\n' "$DIST_DIR/release"
}

tarball_asset_name() {
    printf '%s.tar.gz\n' "$(portable_root_name)"
}

deb_asset_name() {
    printf 'phantom_%s_%s.deb\n' "$(workspace_version)" "$(deb_arch)"
}

appimage_asset_name() {
    printf 'phantom-v%s-linux-%s.AppImage\n' "$(workspace_version)" "$(host_arch)"
}

ensure_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "$1" >&2
        exit 1
    fi
}

prepare_dir() {
    rm -rf "$1"
    mkdir -p "$1"
}

build_release_artifacts() {
    ensure_command cargo

    printf 'building Rust release binaries...\n'
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"

    printf 'building Android server jar...\n'
    "$REPO_ROOT/contrib/android-server/build.sh"
}

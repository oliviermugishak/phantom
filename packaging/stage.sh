#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$SCRIPT_DIR/common.sh"

stage_root="$(stage_root_dir)"
skip_build=0

while (($#)); do
    case "$1" in
        --root)
            stage_root="$2"
            shift 2
            ;;
        --skip-build)
            skip_build=1
            shift
            ;;
        *)
            printf 'error: unknown option: %s\n' "$1" >&2
            exit 1
            ;;
    esac
done

ensure_command install

if (( !skip_build )); then
    build_release_artifacts
fi

prepare_dir "$stage_root"

install -Dm755 "$REPO_ROOT/target/release/phantom" \
    "$stage_root/usr/bin/phantom"
install -Dm755 "$REPO_ROOT/target/release/phantom-gui" \
    "$stage_root/usr/bin/phantom-gui"
install -Dm644 "$REPO_ROOT/contrib/android-server/build/phantom-server.jar" \
    "$stage_root/usr/lib/phantom/phantom-server.jar"

install -Dm644 "$REPO_ROOT/config.example.toml" \
    "$stage_root/usr/share/phantom/config.example.toml"
install -Dm644 "$REPO_ROOT/contrib/phantom.service" \
    "$stage_root/usr/share/phantom/contrib/phantom.service"
install -Dm644 "$REPO_ROOT/contrib/99-phantom.rules" \
    "$stage_root/usr/share/phantom/contrib/99-phantom.rules"

for profile in "$REPO_ROOT"/profiles/*.json; do
    install -Dm644 "$profile" \
        "$stage_root/usr/share/phantom/profiles/$(basename "$profile")"
done

install -Dm644 "$REPO_ROOT/README.md" \
    "$stage_root/usr/share/doc/phantom/README.md"
install -Dm644 "$REPO_ROOT/CHANGELOG.md" \
    "$stage_root/usr/share/doc/phantom/CHANGELOG.md"
install -Dm644 "$REPO_ROOT/docs/INSTALL.md" \
    "$stage_root/usr/share/doc/phantom/INSTALL.md"
install -Dm644 "$REPO_ROOT/docs/RELEASING.md" \
    "$stage_root/usr/share/doc/phantom/RELEASING.md"
install -Dm644 "$REPO_ROOT/LICENSE" \
    "$stage_root/usr/share/licenses/phantom/LICENSE"

install -Dm644 "$REPO_ROOT/packaging/linux/phantom-gui.desktop" \
    "$stage_root/usr/share/applications/phantom-gui.desktop"
install -Dm644 "$REPO_ROOT/packaging/linux/phantom.svg" \
    "$stage_root/usr/share/icons/hicolor/scalable/apps/phantom.svg"

printf '%s\n' "$stage_root"

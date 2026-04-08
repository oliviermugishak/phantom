#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$SCRIPT_DIR/common.sh"

skip_build=0

while (($#)); do
    case "$1" in
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
ensure_command tar

if (( skip_build )); then
    bash "$SCRIPT_DIR/stage.sh" --skip-build >/dev/null
else
    bash "$SCRIPT_DIR/stage.sh" >/dev/null
fi

stage_root="$(stage_root_dir)"
portable_root="$(portable_root_dir)"
prepare_dir "$portable_root"
mkdir -p "$(release_dir)"

cp -a "$stage_root/usr/bin" "$portable_root/bin"
cp -a "$stage_root/usr/lib" "$portable_root/lib"
cp -a "$stage_root/usr/share" "$portable_root/share"
install -m644 "$REPO_ROOT/README.md" "$portable_root/README.md"
install -m644 "$REPO_ROOT/CHANGELOG.md" "$portable_root/CHANGELOG.md"
install -m644 "$REPO_ROOT/LICENSE" "$portable_root/LICENSE"

tarball_path="$(release_dir)/$(tarball_asset_name)"
rm -f "$tarball_path"
tar -C "$DIST_DIR/portable" -czf "$tarball_path" "$(basename "$portable_root")"

printf '%s\n' "$tarball_path"

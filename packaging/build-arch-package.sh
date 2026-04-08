#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$SCRIPT_DIR/common.sh"

tarball_path="$(release_dir)/$(tarball_asset_name)"
skip_build=0

while (($#)); do
    case "$1" in
        --tarball)
            tarball_path="$2"
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

ensure_command makepkg
ensure_command sha256sum
ensure_command sed
ensure_command install

if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    printf 'error: build-arch-package.sh must run as a non-root build user\n' >&2
    exit 1
fi

if [[ ! -f "$tarball_path" ]]; then
    if (( skip_build )); then
        printf 'error: tarball not found: %s\n' "$tarball_path" >&2
        exit 1
    fi
    bash "$SCRIPT_DIR/build-tarball.sh" >/dev/null
fi

build_dir="$DIST_DIR/arch/build"
prepare_dir "$build_dir"
mkdir -p "$(release_dir)"

tarball_name="$(basename "$tarball_path")"
tarball_sha="$(sha256sum "$tarball_path" | awk '{print $1}')"
pkgbuild_path="$build_dir/PKGBUILD"

sed \
    -e "s|@VERSION@|$(workspace_version)|g" \
    -e "s|@PKGREL@|$PACKAGE_ITERATION|g" \
    -e "s|@ARCH@|$(host_arch)|g" \
    -e "s|@TARBALL_NAME@|$tarball_name|g" \
    -e "s|@TARBALL_SHA256@|$tarball_sha|g" \
    -e "s|@PORTABLE_DIR@|$(portable_root_name)|g" \
    "$REPO_ROOT/packaging/arch/PKGBUILD.in" >"$pkgbuild_path"

cp "$tarball_path" "$build_dir/$tarball_name"

(
    cd "$build_dir"
    makepkg --force --cleanbuild --nodeps --nosign
)

find "$build_dir" -maxdepth 1 -type f -name '*.pkg.tar.zst' -exec cp -f {} "$(release_dir)/" \;

find "$(release_dir)" -maxdepth 1 -type f -name '*.pkg.tar.zst' -print

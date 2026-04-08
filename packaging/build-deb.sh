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
ensure_command dpkg-deb

if (( skip_build )); then
    bash "$SCRIPT_DIR/stage.sh" --skip-build >/dev/null
else
    bash "$SCRIPT_DIR/stage.sh" >/dev/null
fi

compute_deb_depends() {
    if command -v dpkg-shlibdeps >/dev/null 2>&1; then
        local output
        if output="$(
            cd "$1" && dpkg-shlibdeps -O usr/bin/phantom usr/bin/phantom-gui 2>/dev/null
        )" && [[ "$output" == shlibs:Depends=* ]]; then
            printf '%s\n' "${output#shlibs:Depends=}"
            return 0
        fi
    fi

    printf '%s\n' \
        "libasound2, libc6, libegl1, libgcc-s1, libgl1, libgtk-3-0, libstdc++6, libwayland-client0, libwayland-cursor0, libx11-6, libxcursor1, libxfixes3, libxi6, libxinerama1, libxkbcommon0, libxrandr2"
}

stage_root="$(stage_root_dir)"
version="$(workspace_version)"
package_root="$DIST_DIR/deb/phantom_${version}_$(deb_arch)"
prepare_dir "$package_root"

cp -a "$stage_root/." "$package_root/"
mkdir -p "$package_root/DEBIAN"

depends="$(compute_deb_depends "$package_root")"

cat >"$package_root/DEBIAN/control" <<EOF
Package: phantom
Version: ${version}
Section: utils
Priority: optional
Architecture: $(deb_arch)
Maintainer: Phantom Maintainers <maintainers@phantom.invalid>
Depends: ${depends}
Homepage: https://github.com/oliviermugishak/phantom
Description: Waydroid keyboard-and-mouse to Android multitouch mapper
 Phantom maps Linux keyboard and mouse input into Android touch gestures for
 Waydroid. This package ships the phantom daemon, the phantom-gui editor,
 the Android touch server jar, starter profiles, and packaging-time docs.
EOF

mkdir -p "$(release_dir)"
deb_path="$(release_dir)/$(deb_asset_name)"
rm -f "$deb_path"
dpkg-deb --build "$package_root" "$deb_path" >/dev/null

printf '%s\n' "$deb_path"

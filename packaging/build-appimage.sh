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

linuxdeploy_bin="${LINUXDEPLOY:-linuxdeploy}"
appimagetool_bin="${APPIMAGETOOL:-appimagetool}"

ensure_command install
ensure_command "$linuxdeploy_bin"
ensure_command "$appimagetool_bin"

if (( skip_build )); then
    bash "$SCRIPT_DIR/stage.sh" --root "$DIST_DIR/appimage/AppDir" --skip-build >/dev/null
else
    bash "$SCRIPT_DIR/stage.sh" --root "$DIST_DIR/appimage/AppDir" >/dev/null
fi

appdir="$DIST_DIR/appimage/AppDir"
install -Dm755 "$REPO_ROOT/packaging/linux/AppRun" "$appdir/AppRun"
mkdir -p "$(release_dir)"

desktop_file="$appdir/usr/share/applications/phantom-gui.desktop"
icon_file="$appdir/usr/share/icons/hicolor/scalable/apps/phantom.svg"
output_path="$(release_dir)/$(appimage_asset_name)"

export APPIMAGE_EXTRACT_AND_RUN=1
ARCH="$(appimage_arch)"
export ARCH

linuxdeploy_args=(
    --appdir "$appdir"
    --desktop-file "$desktop_file"
    --icon-file "$icon_file"
    --executable "$appdir/usr/bin/phantom-gui"
)

if command -v linuxdeploy-plugin-gtk.sh >/dev/null 2>&1 || command -v linuxdeploy-plugin-gtk >/dev/null 2>&1; then
    export DEPLOY_GTK_VERSION="${DEPLOY_GTK_VERSION:-3}"
    linuxdeploy_args+=(--plugin gtk)
fi

"$linuxdeploy_bin" "${linuxdeploy_args[@]}"
rm -f "$output_path"
"$appimagetool_bin" "$appdir" "$output_path"

printf '%s\n' "$output_path"

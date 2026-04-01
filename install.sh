#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA_DIR="$DATA_HOME/phantom"
ANDROID_DIR="$DATA_DIR/android"
CONFIG_DIR="$CONFIG_HOME/phantom"
PROFILE_DIR="$CONFIG_DIR/profiles"
INSTALLED_JAR="$ANDROID_DIR/phantom-server.jar"
CONFIG_PATH="$CONFIG_DIR/config.toml"

usage() {
    cat <<'EOF'
Usage:
  ./install.sh        Build and install Phantom and Phantom GUI into the current user environment
  ./install.sh -u     Uninstall Phantom binaries and installed Android server jar
  ./install.sh -h     Show this help

Environment overrides:
  PREFIX         Install prefix for binaries (default: ~/.local)
  XDG_DATA_HOME  Data root for installed Android server jar
  XDG_CONFIG_HOME Config root for user config and profiles
EOF
}

escape_sed_replacement() {
    printf '%s' "$1" | sed 's/[&|]/\\&/g'
}

ensure_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "$1" >&2
        exit 1
    fi
}

copy_profile_if_missing() {
    local source="$1"
    local file_name
    file_name="$(basename "$source")"
    local target="$PROFILE_DIR/$file_name"
    if [[ ! -e "$target" ]]; then
        install -Dm644 "$source" "$target"
        printf 'installed profile: %s\n' "$target"
    fi
}

create_config_if_missing() {
    if [[ -e "$CONFIG_PATH" ]]; then
        printf 'keeping existing config: %s\n' "$CONFIG_PATH"
        return
    fi

    install -d "$CONFIG_DIR"
    local escaped_jar
    escaped_jar="$(escape_sed_replacement "$INSTALLED_JAR")"
    sed \
        "s|/absolute/path/to/ttplayer/contrib/android-server/build/phantom-server.jar|$escaped_jar|g" \
        "$REPO_ROOT/config.example.toml" >"$CONFIG_PATH"
    printf 'created config: %s\n' "$CONFIG_PATH"
}

install_phantom() {
    ensure_command cargo
    ensure_command sed
    ensure_command install

    printf 'building Rust binaries...\n'
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"

    printf 'building Android server jar...\n'
    "$REPO_ROOT/contrib/android-server/build.sh"

    install -d "$BIN_DIR" "$ANDROID_DIR" "$PROFILE_DIR"
    install -m755 "$REPO_ROOT/target/release/phantom" "$BIN_DIR/phantom"
    install -m755 "$REPO_ROOT/target/release/phantom-gui" "$BIN_DIR/phantom-gui"
    rm -f "$BIN_DIR/phantom-studio"
    install -m644 "$REPO_ROOT/contrib/android-server/build/phantom-server.jar" "$INSTALLED_JAR"

    for profile in "$REPO_ROOT"/profiles/*.json; do
        copy_profile_if_missing "$profile"
    done

    create_config_if_missing

    printf '\ninstalled:\n'
    printf '  %s\n' "$BIN_DIR/phantom"
    printf '  %s\n' "$BIN_DIR/phantom-gui"
    printf '  %s\n' "$INSTALLED_JAR"
    printf '  %s\n' "$CONFIG_PATH"

    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *)
            printf '\nwarning: %s is not currently on PATH\n' "$BIN_DIR"
            printf 'add this to your shell profile:\n'
            printf '  export PATH="%s:$PATH"\n' "$BIN_DIR"
            ;;
    esac
}

uninstall_phantom() {
    rm -f "$BIN_DIR/phantom" "$BIN_DIR/phantom-gui" "$BIN_DIR/phantom-studio" "$INSTALLED_JAR"
    rmdir "$ANDROID_DIR" 2>/dev/null || true
    rmdir "$DATA_DIR" 2>/dev/null || true

    printf 'removed installed binaries and Android server jar\n'
    printf 'kept user config and profiles in: %s\n' "$CONFIG_DIR"
}

main() {
    local uninstall=0

    while (($#)); do
        case "$1" in
            -u|--uninstall)
                uninstall=1
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                printf 'error: unknown option: %s\n\n' "$1" >&2
                usage >&2
                exit 1
                ;;
        esac
        shift
    done

    if ((uninstall)); then
        uninstall_phantom
    else
        install_phantom
    fi
}

main "$@"

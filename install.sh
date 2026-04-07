#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
SYSTEM_BIN_DIR="${SYSTEM_BIN_DIR:-/usr/local/bin}"
INSTALL_SYSTEM_WRAPPER="${INSTALL_SYSTEM_WRAPPER:-1}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
DATA_DIR="$DATA_HOME/phantom"
ANDROID_DIR="$DATA_DIR/android"
CONFIG_DIR="$CONFIG_HOME/phantom"
PROFILE_DIR="$CONFIG_DIR/profiles"
INSTALLED_JAR="$ANDROID_DIR/phantom-server.jar"
CONFIG_PATH="$CONFIG_DIR/config.toml"
SYSTEM_PHANTOM_WRAPPER="$SYSTEM_BIN_DIR/phantom"
PROMPT_OVERRIDES=0
OVERWRITE_CONFIG=0
OVERWRITE_PROFILES=0

usage() {
    cat <<'EOF'
Usage:
  ./install.sh        Build and install Phantom and Phantom GUI into the current user environment
  ./install.sh -o     Prompt before overwriting existing config and shipped profiles
  ./install.sh -u     Uninstall Phantom binaries and installed Android server jar
  ./install.sh -h     Show this help

Environment overrides:
  PREFIX         Install prefix for binaries (default: ~/.local)
  SYSTEM_BIN_DIR System launcher directory for sudo-visible phantom wrapper (default: /usr/local/bin)
  INSTALL_SYSTEM_WRAPPER  Set to 0 to skip installing the sudo-visible phantom wrapper
  XDG_DATA_HOME  Data root for installed Android server jar
  XDG_CONFIG_HOME Config root for user config and profiles
EOF
}

prompt_yes_no() {
    local prompt="$1"
    local reply
    read -r -p "$prompt [y/N] " reply
    [[ "$reply" =~ ^([yY]|[yY][eE][sS])$ ]]
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

can_write_path_directly() {
    local target="$1"
    if [[ -e "$target" ]]; then
        [[ -w "$target" ]]
    else
        can_write_dir_directly "$(dirname "$target")"
    fi
}

can_write_dir_directly() {
    local dir="$1"
    if [[ -d "$dir" ]]; then
        [[ -w "$dir" ]]
    else
        [[ -w "$(dirname "$dir")" ]]
    fi
}

install_system_phantom_wrapper() {
    if [[ "$INSTALL_SYSTEM_WRAPPER" != "1" ]]; then
        printf 'skipping system phantom wrapper (INSTALL_SYSTEM_WRAPPER=%s)\n' "$INSTALL_SYSTEM_WRAPPER"
        return
    fi

    local tmp_wrapper
    tmp_wrapper="$(mktemp)"
    cat >"$tmp_wrapper" <<EOF
#!/usr/bin/env bash
exec "$BIN_DIR/phantom" "\$@"
EOF

    if can_write_path_directly "$SYSTEM_PHANTOM_WRAPPER"; then
        install -d "$SYSTEM_BIN_DIR"
        install -m755 "$tmp_wrapper" "$SYSTEM_PHANTOM_WRAPPER"
        printf 'installed sudo-visible launcher: %s\n' "$SYSTEM_PHANTOM_WRAPPER"
    elif command -v sudo >/dev/null 2>&1; then
        sudo install -d "$SYSTEM_BIN_DIR"
        sudo install -m755 "$tmp_wrapper" "$SYSTEM_PHANTOM_WRAPPER"
        printf 'installed sudo-visible launcher: %s\n' "$SYSTEM_PHANTOM_WRAPPER"
    else
        printf 'warning: could not install %s and sudo is unavailable\n' "$SYSTEM_PHANTOM_WRAPPER" >&2
        printf 'sudo may not find phantom unless you adjust secure_path or run sudo %s/phantom\n' "$BIN_DIR" >&2
    fi

    rm -f "$tmp_wrapper"
}

remove_system_phantom_wrapper() {
    if [[ -e "$SYSTEM_PHANTOM_WRAPPER" ]]; then
        if can_write_path_directly "$SYSTEM_PHANTOM_WRAPPER"; then
            rm -f "$SYSTEM_PHANTOM_WRAPPER"
        elif command -v sudo >/dev/null 2>&1; then
            sudo rm -f "$SYSTEM_PHANTOM_WRAPPER"
        fi
    fi
}

install_profile() {
    local source="$1"
    local file_name
    file_name="$(basename "$source")"
    local target="$PROFILE_DIR/$file_name"
    if ((OVERWRITE_PROFILES)); then
        install -Dm644 "$source" "$target"
        printf 'overwrote profile: %s\n' "$target"
    elif [[ ! -e "$target" ]]; then
        install -Dm644 "$source" "$target"
        printf 'installed profile: %s\n' "$target"
    fi
}

install_config() {
    if [[ -e "$CONFIG_PATH" ]] && (( !OVERWRITE_CONFIG )); then
        sync_installed_server_jar_in_config
        printf 'keeping existing config: %s\n' "$CONFIG_PATH"
        return
    fi

    install -d "$CONFIG_DIR"
    local escaped_jar
    escaped_jar="$(escape_sed_replacement "$INSTALLED_JAR")"
    sed \
        "s|/absolute/path/to/phantom-server.jar|$escaped_jar|g" \
        "$REPO_ROOT/config.example.toml" >"$CONFIG_PATH"
    if ((OVERWRITE_CONFIG)); then
        printf 'overwrote config: %s\n' "$CONFIG_PATH"
    else
        printf 'created config: %s\n' "$CONFIG_PATH"
    fi
}

config_server_jar_value() {
    sed -nE 's/^[[:space:]]*server_jar[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$1" | head -n1
}

sync_installed_server_jar_in_config() {
    [[ -e "$CONFIG_PATH" ]] || return

    local current_jar
    current_jar="$(config_server_jar_value "$CONFIG_PATH")"

    case "$current_jar" in
        "$INSTALLED_JAR"|"")
            return
            ;;
        "/absolute/path/to/phantom-server.jar"|*/contrib/android-server/build/phantom-server.jar)
            local escaped_jar tmp_config
            escaped_jar="$(escape_sed_replacement "$INSTALLED_JAR")"
            tmp_config="$(mktemp)"
            sed -E \
                "s|^([[:space:]]*server_jar[[:space:]]*=[[:space:]]*\").*(\".*)$|\1$escaped_jar\2|" \
                "$CONFIG_PATH" >"$tmp_config"
            mv "$tmp_config" "$CONFIG_PATH"
            printf 'updated android server jar in existing config: %s\n' "$CONFIG_PATH"
            ;;
    esac
}

prompt_override_choices() {
    if (( !PROMPT_OVERRIDES )); then
        return
    fi

    if [[ ! -t 0 ]]; then
        printf 'error: -o requires an interactive terminal for overwrite prompts\n' >&2
        exit 1
    fi

    if [[ -e "$CONFIG_PATH" ]]; then
        if prompt_yes_no "Override existing config at $CONFIG_PATH?"; then
            OVERWRITE_CONFIG=1
        fi
    fi

    local existing_shipped_profile=0
    local profile
    for profile in "$REPO_ROOT"/profiles/*.json; do
        if [[ -e "$PROFILE_DIR/$(basename "$profile")" ]]; then
            existing_shipped_profile=1
            break
        fi
    done

    if ((existing_shipped_profile)); then
        if prompt_yes_no "Override currently shipped profiles in $PROFILE_DIR?"; then
            OVERWRITE_PROFILES=1
        fi
    fi
}

install_phantom() {
    ensure_command cargo
    ensure_command sed
    ensure_command install

    prompt_override_choices

    printf 'building Rust binaries...\n'
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"

    printf 'building Android server jar...\n'
    "$REPO_ROOT/contrib/android-server/build.sh"

    install -d "$BIN_DIR" "$ANDROID_DIR" "$PROFILE_DIR"
    install -m755 "$REPO_ROOT/target/release/phantom" "$BIN_DIR/phantom"
    install -m755 "$REPO_ROOT/target/release/phantom-gui" "$BIN_DIR/phantom-gui"
    rm -f "$BIN_DIR/phantom-studio"
    install -m644 "$REPO_ROOT/contrib/android-server/build/phantom-server.jar" "$INSTALLED_JAR"
    install_system_phantom_wrapper

    for profile in "$REPO_ROOT"/profiles/*.json; do
        install_profile "$profile"
    done

    install_config

    printf '\ninstalled:\n'
    printf '  %s\n' "$BIN_DIR/phantom"
    printf '  %s\n' "$BIN_DIR/phantom-gui"
    printf '  %s\n' "$INSTALLED_JAR"
    printf '  %s\n' "$CONFIG_PATH"
    if [[ "$INSTALL_SYSTEM_WRAPPER" == "1" ]]; then
        printf '  %s\n' "$SYSTEM_PHANTOM_WRAPPER"
    fi

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
    remove_system_phantom_wrapper
    rmdir "$ANDROID_DIR" 2>/dev/null || true
    rmdir "$DATA_DIR" 2>/dev/null || true

    printf 'removed installed binaries, sudo-visible phantom launcher, and Android server jar\n'
    printf 'kept user config and profiles in: %s\n' "$CONFIG_DIR"
}

main() {
    local uninstall=0

    while (($#)); do
        case "$1" in
            -o|--overwrite)
                PROMPT_OVERRIDES=1
                ;;
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

    if ((uninstall && PROMPT_OVERRIDES)); then
        printf 'error: -o cannot be combined with -u\n' >&2
        exit 1
    fi

    if ((uninstall)); then
        uninstall_phantom
    else
        install_phantom
    fi
}

main "$@"

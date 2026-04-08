#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$SCRIPT_DIR/../common.sh"

repo_name="${1:-phantom}"
repo_dir="${2:-$DIST_DIR/pacman-repo}"

ensure_command repo-add
ensure_command install

mkdir -p "$repo_dir"

shopt -s nullglob
release_root="$(release_dir)"
packages=("$release_root"/*.pkg.tar.zst)
shopt -u nullglob

if ((${#packages[@]} == 0)); then
    printf 'error: no Arch packages found in %s\n' "$(release_dir)" >&2
    exit 1
fi

install -m644 "${packages[@]}" "$repo_dir/"

(
    cd "$repo_dir"
    repo-add "${repo_name}.db.tar.gz" ./*.pkg.tar.zst
    ln -sf "${repo_name}.db.tar.gz" "${repo_name}.db"
    ln -sf "${repo_name}.files.tar.gz" "${repo_name}.files"
)

printf '%s\n' "$repo_dir"

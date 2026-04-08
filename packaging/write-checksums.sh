#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$SCRIPT_DIR/common.sh"

ensure_command sha256sum

output_dir="$(release_dir)"
mkdir -p "$output_dir"

(
    cd "$output_dir"
    rm -f SHA256SUMS
    shopt -s nullglob
    files=(*)
    shopt -u nullglob

    if ((${#files[@]} == 0)); then
        printf 'error: no release assets found in %s\n' "$output_dir" >&2
        exit 1
    fi

    for file in "${files[@]}"; do
        [[ "$file" == "SHA256SUMS" ]] && continue
        sha256sum "$file"
    done | sort >SHA256SUMS
)

printf '%s\n' "$output_dir/SHA256SUMS"

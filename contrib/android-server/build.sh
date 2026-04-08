#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$ROOT/build"
SRC_DIR="$ROOT/src"

detect_d8() {
  if [[ -n "${D8:-}" && -x "${D8:-}" ]]; then
    printf '%s\n' "$D8"
    return 0
  fi

  if command -v d8 >/dev/null 2>&1; then
    command -v d8
    return 0
  fi

  local sdk_root=""
  if [[ -n "${ANDROID_SDK_ROOT:-}" && -d "${ANDROID_SDK_ROOT:-}" ]]; then
    sdk_root="$ANDROID_SDK_ROOT"
  elif [[ -n "${ANDROID_HOME:-}" && -d "${ANDROID_HOME:-}" ]]; then
    sdk_root="$ANDROID_HOME"
  elif [[ -d "$HOME/Android/Sdk" ]]; then
    sdk_root="$HOME/Android/Sdk"
  fi

  if [[ -n "$sdk_root" ]]; then
    local build_tools_d8=""
    build_tools_d8="$(
      find "$sdk_root/build-tools" -maxdepth 2 -type f -name d8 2>/dev/null \
        | sort -V \
        | tail -n 1
    )"
    if [[ -n "$build_tools_d8" && -x "$build_tools_d8" ]]; then
      printf '%s\n' "$build_tools_d8"
      return 0
    fi

    if [[ -x "$sdk_root/cmdline-tools/latest/bin/d8" ]]; then
      printf '%s\n' "$sdk_root/cmdline-tools/latest/bin/d8"
      return 0
    fi
  fi

  return 1
}

detect_android_jar() {
  if [[ -n "${ANDROID_JAR:-}" && -f "${ANDROID_JAR:-}" ]]; then
    printf '%s\n' "$ANDROID_JAR"
    return 0
  fi

  local sdk_root=""
  if [[ -n "${ANDROID_SDK_ROOT:-}" && -d "${ANDROID_SDK_ROOT:-}" ]]; then
    sdk_root="$ANDROID_SDK_ROOT"
  elif [[ -n "${ANDROID_HOME:-}" && -d "${ANDROID_HOME:-}" ]]; then
    sdk_root="$ANDROID_HOME"
  elif [[ -d "$HOME/Android/Sdk" ]]; then
    sdk_root="$HOME/Android/Sdk"
  fi

  if [[ -z "$sdk_root" ]]; then
    return 1
  fi

  local latest_jar=""
  latest_jar="$(
    find "$sdk_root/platforms" -maxdepth 2 -name android.jar 2>/dev/null \
      | sort -V \
      | tail -n 1
  )"

  if [[ -n "$latest_jar" && -f "$latest_jar" ]]; then
    printf '%s\n' "$latest_jar"
    return 0
  fi

  return 1
}

ANDROID_JAR="$(detect_android_jar || true)"
if [[ -z "$ANDROID_JAR" ]]; then
  echo "Could not find android.jar" >&2
  echo "Install an Android platform under ~/Android/Sdk or set ANDROID_JAR explicitly" >&2
  exit 1
fi

D8_BIN="$(detect_d8 || true)"
if [[ -z "$D8_BIN" ]]; then
  echo "Could not find d8" >&2
  echo "Set D8 explicitly or install Android command-line tools with d8 under ~/Android/Sdk" >&2
  exit 1
fi

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/classes"

echo "Using ANDROID_JAR=$ANDROID_JAR"
echo "Using D8_BIN=$D8_BIN"

javac \
  -source 8 \
  -target 8 \
  -cp "$ANDROID_JAR" \
  -d "$OUT_DIR/classes" \
  "$SRC_DIR/com/phantom/server/PhantomServer.java"

jar cf "$OUT_DIR/classes.jar" -C "$OUT_DIR/classes" .

"$D8_BIN" \
  --release \
  --lib "$ANDROID_JAR" \
  --min-api 26 \
  --output "$OUT_DIR/phantom-server.jar" \
  "$OUT_DIR/classes.jar"

echo "Built $OUT_DIR/phantom-server.jar"

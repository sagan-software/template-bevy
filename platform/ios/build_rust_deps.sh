#!/usr/bin/env bash
set -euo pipefail
cd "$SRCROOT/../.."
python3 scripts/reserve.py 3
profile=debug
flags=()
if [[ "$CONFIGURATION" != Debug ]]; then profile=release; flags=(--release); fi
executables=()
cargo_target=$(nix develop -c cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
for arch in $ARCHS; do
  case "$arch:${LLVM_TARGET_TRIPLE_SUFFIX:-}" in
    arm64:-simulator) target=aarch64-apple-ios-sim ;;
    arm64:*) target=aarch64-apple-ios ;;
    x86_64:-simulator) target=x86_64-apple-ios ;;
    *) echo "Unsupported iOS architecture: $arch" >&2; exit 2 ;;
  esac
  nix develop -c cargo build -p factory-hello --bin factory-hello "${flags[@]}" --target "$target"
  executables+=("$cargo_target/$target/$profile/factory-hello")
done
lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" "${executables[@]}"

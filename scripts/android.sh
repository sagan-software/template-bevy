#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/reserve.py 5
: "${ANDROID_HOME:?Use nix develop .#android}"
export GRADLE_USER_HOME="$PWD/local/gradle"
export ANDROID_USER_HOME="$PWD/local/android"
mkdir -p "$ANDROID_USER_HOME"
cargo ndk -t arm64-v8a --platform 31 -o platform/android/app/src/main/jniLibs rustc -p factory-hello --lib --crate-type cdylib
(cd platform/android && ./gradlew --no-daemon assembleDebug bundleDebug)
mkdir -p dist/android
cp platform/android/app/build/outputs/apk/debug/app-debug.apk dist/android/game-factory-debug.apk
cp platform/android/app/build/outputs/bundle/debug/app-debug.aab dist/android/game-factory-debug.aab

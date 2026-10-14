#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/reserve.py 2
app=$(python3 -c 'import tomllib; print(tomllib.load(open("crates/app/Cargo.toml", "rb"))["package"]["name"])')
cargo build --release --target wasm32-unknown-unknown -p "$app" --no-default-features --features factory-demo,web
cargo_target=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
mkdir -p dist/web
bindgen_version=$(python3 -c 'import tomllib; print(next(p["version"] for p in tomllib.load(open("Cargo.lock", "rb"))["package"] if p["name"] == "wasm-bindgen"))')
bindgen=wasm-bindgen
if [[ $(wasm-bindgen --version) != "wasm-bindgen $bindgen_version" ]]; then
    bindgen="$PWD/local/tools/bin/wasm-bindgen"
    if [[ ! -x $bindgen ]] || [[ $($bindgen --version) != "wasm-bindgen $bindgen_version" ]]; then
        cargo install wasm-bindgen-cli --version "$bindgen_version" --locked --root "$PWD/local/tools" --target-dir "$cargo_target/tools-build"
    fi
fi
"$bindgen" --target web --out-dir dist/web --out-name game_factory "$cargo_target/wasm32-unknown-unknown/release/$app.wasm"
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int dist/web/game_factory_bg.wasm -o dist/web/game_factory_bg.wasm
cp web/* dist/web/
cp -r assets dist/web/
python3 - <<'ZIP'
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
with ZipFile('dist/game-factory-web.zip', 'w', ZIP_DEFLATED) as archive:
    for path in sorted(Path('dist/web').rglob('*')):
        if path.is_file():
            archive.write(path, path.relative_to('dist/web'))
ZIP

#!/usr/bin/env bash
set -euo pipefail
if [[ -z "${BUTLER_API_KEY:-}" ]]; then
  echo "Disabled: BUTLER_API_KEY is unset."
  exit 0
fi
: "${ITCH_TARGET:?Set account/game:html5 explicitly}"
butler push dist/game-factory-web.zip "$ITCH_TARGET"

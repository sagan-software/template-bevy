#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p local
nohup setsid python3 -m http.server 8761 -d dist/web --bind 0.0.0.0 >local/serve.log 2>&1 </dev/null &
sleep 1
echo "$!" >local/serve.pid

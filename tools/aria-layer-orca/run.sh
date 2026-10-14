#!/usr/bin/env bash
# Run Record Desk (or another hosted web application) under actual Orca in a
# disposable rootless Podman container, and keep Orca's speech transcript.
#
#   bash tools/aria-layer-orca/run.sh [WEB_DIR]
#
# WEB_DIR defaults to Record Desk's built web/ directory. Output goes to
# STOBIE_EVIDENCE_DIR (default .tools/runtime/aria-layer-orca); speech.txt
# lists what Orca generated. There is no audio device, so this shows speech
# generation, not audible output.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/tools/aria-layer-orca"
WEB="$(cd "${1:-$ROOT/consumers/record-desk/web}" && pwd)"
OUT="${STOBIE_EVIDENCE_DIR:-$ROOT/.tools/runtime/aria-layer-orca}"
PORT="${STOBIE_ORCA_PORT:-8766}"
IMAGE=localhost/stobie-aria-layer-orca:1
rm -rf "$OUT" && mkdir -p "$OUT"
podman image exists "$IMAGE" || podman build -t "$IMAGE" -f "$HERE/Containerfile" "$HERE"
CHROME="$(cd "$ROOT" && node -e "import('playwright').then(p => console.log(p.chromium.executablePath()))")"
node "$ROOT/tools/aria-layer-orca/serve.mjs" "$WEB" "$PORT" &
SERVER=$!
trap 'kill "$SERVER" 2>/dev/null || true' EXIT
podman run --rm --network=host --userns=keep-id \
  -v "$HERE:/work:ro" -v "$OUT:/out" -v "$(dirname "$CHROME"):/chromium:ro" \
  -e APP_URL="http://127.0.0.1:$PORT/" "$IMAGE" bash /work/session.sh
grep -o "SPEECH OUTPUT: '[^']*'" "$OUT/orca-debug.txt" | sed "s/^SPEECH OUTPUT: //" >"$OUT/speech.txt"
cat "$OUT/versions.txt" "$OUT/speech.txt"

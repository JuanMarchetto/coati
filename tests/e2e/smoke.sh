#!/usr/bin/env bash
set -euo pipefail

# Resolve repo root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

echo "=== Coati Phase 1 smoke test ==="

# --- Prereq: ollama ---
if ! command -v ollama >/dev/null 2>&1; then
    echo "FAIL: ollama not installed. Install from https://ollama.com"
    exit 1
fi

# Check ollama is reachable; if not, try to start the server briefly
if ! curl -fs http://localhost:11434/api/tags >/dev/null 2>&1; then
    echo "INFO: ollama server not reachable; starting in background"
    ollama serve >/tmp/coati-smoke-ollama.log 2>&1 &
    OLLAMA_PID=$!
    for i in $(seq 1 20); do
        if curl -fs http://localhost:11434/api/tags >/dev/null 2>&1; then break; fi
        sleep 0.5
    done
    trap 'kill $OLLAMA_PID 2>/dev/null || true' EXIT
fi

# --- Prereq: a model ---
MODEL="${COATI_SMOKE_MODEL:-gemma3}"
if ! ollama list 2>/dev/null | awk 'NR>1 {print $1}' | grep -q "^${MODEL}"; then
    echo "INFO: pulling model ${MODEL} (this may take a while)"
    ollama pull "${MODEL}" || { echo "FAIL: could not pull ${MODEL}"; exit 1; }
fi

# --- Prereq: built release binary ---
echo "--- building release binary ---"
cargo build --release --bin coati

COATI="./target/release/coati"

# --- Smoke test 1: simple question ---
echo "--- smoke test 1: simple question ---"
OUT=$(echo "what is 2 plus 2, one word answer" | timeout 90 "$COATI" ask || true)
echo "response: $OUT"
[[ -n "$OUT" ]] || { echo "FAIL: empty response from simple ask"; exit 1; }

# --- Smoke test 2: daemon ping ---
echo "--- smoke test 2: daemon ping ---"
SOCK="/tmp/coati-smoke-$$.sock"
rm -f "$SOCK"
"$COATI" serve --socket "$SOCK" >/tmp/coati-smoke-daemon.log 2>&1 &
DAEMON_PID=$!
# wait for socket
for i in $(seq 1 40); do
    if [[ -S "$SOCK" ]]; then break; fi
    sleep 0.1
done
if [[ ! -S "$SOCK" ]]; then
    echo "FAIL: daemon socket never appeared"
    kill $DAEMON_PID 2>/dev/null || true
    cat /tmp/coati-smoke-daemon.log
    exit 1
fi

if command -v nc >/dev/null 2>&1; then
    PONG=$(echo '{"type":"ping"}' | nc -U -q1 "$SOCK" 2>/dev/null || echo "")
    echo "daemon response: $PONG"
    [[ "$PONG" == *pong* ]] || { echo "FAIL: daemon did not respond with pong"; kill $DAEMON_PID 2>/dev/null; exit 1; }
else
    echo "INFO: nc not available, skipping direct socket verification (socket existence is sufficient signal)"
fi

kill $DAEMON_PID 2>/dev/null || true
rm -f "$SOCK"

echo "=== all smoke tests passed ==="

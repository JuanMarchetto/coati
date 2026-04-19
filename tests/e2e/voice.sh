#!/usr/bin/env bash
# tests/e2e/voice.sh — dogfood test for voice pipeline on silence.
# Requires: coati binary built with --features voice, a model installed,
# and `sox` or `ffmpeg` on PATH for synthesizing a silent WAV.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$ROOT/target/release/coati"
[ -x "$BIN" ] || BIN="$ROOT/target/debug/coati"

if [ ! -x "$BIN" ]; then
  echo "error: coati binary not found — run 'cargo build --features voice -p coati-cli'"
  exit 1
fi

WAV="$(mktemp --suffix=.wav)"
trap 'rm -f "$WAV"' EXIT

# Synthesize 1 second of silence at 16kHz mono.
if command -v sox >/dev/null 2>&1; then
  sox -n -r 16000 -c 1 "$WAV" trim 0.0 1.0
elif command -v ffmpeg >/dev/null 2>&1; then
  ffmpeg -y -f lavfi -i anullsrc=r=16000:cl=mono -t 1 -acodec pcm_s16le "$WAV" -loglevel error
else
  echo "skip: need sox or ffmpeg to synthesize WAV"
  exit 0
fi

MODEL="${COATI_VOICE_MODEL:-base.en}"
echo "Transcribing $WAV with model=$MODEL"
OUT="$("$BIN" voice transcribe "$WAV" --model "$MODEL")"
echo "Output: '$OUT'"

# Silence should produce empty or very short output.
LEN=${#OUT}
if [ "$LEN" -gt 200 ]; then
  echo "FAIL: unexpected long transcript on silence ($LEN chars)"
  exit 1
fi
echo "PASS"

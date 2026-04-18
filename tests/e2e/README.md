# Phase 1 Smoke Tests

Exercises the Coati CLI end-to-end against a live ollama.

## Prereqs
- ollama installed
- (auto-handled) ollama running — script starts it if not
- (auto-handled) a model — defaults to `gemma3`, override with `COATI_SMOKE_MODEL=qwen2.5`
- `nc` is nice-to-have but not required

## Run
```bash
./tests/e2e/smoke.sh
```

## What it checks
1. `coati ask` returns a non-empty response to a simple question
2. `coati serve` daemon starts and replies to `{"type":"ping"}` with `pong`

Run from anywhere; script resolves the repo root.

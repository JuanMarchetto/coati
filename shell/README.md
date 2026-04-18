# Coati shell plugins

Thin zsh / bash / fish integrations that expose `coati <intent>` and `??`.

## Install

```bash
./shell/install.sh              # auto-detect
./shell/install.sh --shell zsh  # force
```

Appends a single `source` line to your rc file. Idempotent.

## What gets installed

- `coati <natural language>` — sends the intent to the local coati daemon, shows a proposed command with one-line reasoning, prompts `[y/N]` before running. Sudo commands get an extra warning line.
- `??` — explains the previous command (exit code auto-captured). Prints cause and a fix suggestion.
- `preexec`/`precmd` hooks capture: `pwd`, last command, last exit code, git branch, shell name. All sent as a `ShellContext` JSON only to the local Unix socket.

## Dependencies

- `coati` binary on `$PATH`
- No `jq` required — parsing uses `sed` for the small flat JSON objects emitted by the daemon

## Tests

```bash
./shell/tests/run.sh   # bats + mock daemon
```

## Architecture notes

The shell plugins never call the network. They only invoke the local `coati` binary, which in turn talks only to the local Unix socket daemon. Any sudo command requires an explicit `y` keypress; confirm-default-no is enforced at the shell layer so even if a malicious proposal slipped through, the user sees it before anything runs.

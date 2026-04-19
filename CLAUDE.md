# coati

> Your Linux copilot — a local, private, voice-capable system agent, shipped as an installable suite, not a distro.

**Why the name:** Coati = a small, curious, problem-solving mammal native to the Americas. The metaphor is an agent that pokes around your system, sniffs out what's wrong, and helps you fix it. Short (5 letters), phonetically distinct (no Pingo/Penguin collisions), clean in AI/dev-tool namespace as of the naming gauntlet.

## What this is

A single opinionated bundle that turns any modern Linux install (Ubuntu / Fedora / Arch) into an AI-native workstation. One installer. No distro switch. No cloud dependency by default.

**The product is the agent, not the OS.** The distribution channel is a package, not an ISO.

## Product surfaces

Four integrated surfaces, one backend:

1. **Shell plugin** (bash/zsh/fish) — inline completions, `??` explain-last-error, natural-language → command with confirm prompt, context-aware of `pwd`, recent history, git state.
2. **Desktop chat** (Tauri tray app) — hotkey-summoned window, persistent conversation, drag-drop files/screenshots, streaming responses.
3. **Voice daemon** — local wake-word (`porcupine` or `openwakeword`) → `whisper.cpp` STT → intent dispatch → spoken or visual response. Push-to-talk fallback.
4. **Agent backend** — long-running `systemd --user` service that holds context, exposes system tools as typed function-calls, routes requests from all three surfaces through one reasoning loop.

## Why not a distro

| | Distro | This suite |
|---|---|---|
| User friction | Reinstall OS | `curl \| sh` |
| Maintenance | Track upstream forever | Own packages only |
| Compatibility | Break things | Works on all major distros |
| MVP effort | 4-6 months | ~8-10 weeks |
| Audience | Distro-hoppers | Every Linux dev |

A distro is a delivery gimmick. The value is the agent, and the agent should meet users where they already are.

## Tech stack

- **Agent runtime:** Rust + [`rig.rs`](https://github.com/0xPlaygrounds/rig) for typed tool-calling. Rust is the right call here: system tools (exec, fs, pkg, logs) benefit from strong typing and no runtime deps.
- **Inference:** Local via `ollama` (default model: Gemma 3 or Qwen 2.5). Optional remote (Anthropic/OpenAI) for heavier reasoning, user-toggled.
- **STT:** `whisper.cpp` (small/base model) for transcription. `porcupine` or `openwakeword` for always-listening wake word.
- **TTS (optional):** `piper` — small, fast, local neural TTS.
- **Desktop app:** Tauri (Rust + webview) — small binary, native feel, same backend IPC as CLI.
- **Shell plugin:** thin shell script + Rust binary that speaks to the agent over Unix socket / dbus.
- **Packaging:** `.deb`, `.rpm`, AUR, Flatpak. Ubuntu LTS is launch target.

## MVP scope (8-10 weeks solo + AI-assisted)

- [ ] Agent backend with 5 core tools: `exec`, `read_file`, `list_dir`, `query_logs`, `explain_error`
- [ ] Shell plugin for zsh + bash with `??` and natural-language command
- [ ] Tauri tray chat with hotkey summon
- [ ] Voice: push-to-talk only (no wake word yet) → whisper.cpp → agent
- [ ] `.deb` package + install script for Ubuntu 24.04+
- [ ] Landing page + 2-minute demo video

## v1.0 scope (+3 months after MVP)

- Wake-word always-listening mode (opt-in, LED-visible)
- Plugin system for third-party tool modules (git, docker, nix, kubectl)
- Fedora + Arch packaging
- Model manager UI (download, switch, benchmark)
- Conversation history + local vector memory
- TTS replies (piper)

## Guiding principles

1. **Local by default.** Zero network calls unless the user opts in. A private agent is the whole point.
2. **Typed tools, not prompt hacks.** Every system action is a rig.rs function signature with explicit permissions, not a string parsed out of a reply.
3. **Confirm before sudo.** The agent can *propose* destructive actions; a human confirms. Autopilot is a v2 setting, off by default.
4. **Plugin-first architecture.** Ship a thin core; let the ecosystem write the long tail. `git-agent`, `docker-agent`, `nix-agent` should be community modules from day one.
5. **Brand the agent, not the AI.** The word "AI" is infrastructure now. Position as "your Linux copilot" or similar. Name centers the agent.

## Prior art to study (not compete with directly)

- **Warp Terminal** — closed-source, AI-native shell. Good UX reference, bad license story.
- **Raycast** (macOS) — gold standard for installable-suite shape. No Linux equivalent wins this market.
- **ShellGPT / aichat / aider** — single-surface CLI wrappers. Prove demand, don't cover desktop + voice.
- **GNOME AI extensions** — toy-grade, desktop-only, no shell or voice.
- **Albert / Rofi + scripts** — DIY launcher kits users cobble together. The gap this project fills.

## Portfolio framing

This is a credibility artifact, not (initially) a revenue play. What it signals:
- End-to-end shipping ability (systems + AI + desktop + packaging)
- Rust competence at the agent and tauri layers
- Understanding of local-first AI architecture
- Taste: choosing a suite over a distro shows product judgment

Demo gold: a 60-second video of voice → agent → `sudo systemctl restart nginx` (with confirm) is viral-tier content on r/linux, HN, and YouTube.

## Monetization (future, optional)

- Free + open-source core, MIT or Apache-2.
- Pro tier: cloud-synced conversation history, premium tool modules, team features, priority support.
- Not the priority pre-v1. Focus on adoption + credibility first.

**Phase 2 (shell integration) shipped 2026-04-18** — zsh/bash/fish plugins, `coati <intent>` with confirm-before-sudo, `??` explain-last-error, bats tests, CI job. Next: Phase 3 (Tauri desktop tray + chat window).

**Phase 4 (push-to-talk voice) shipped 2026-04-18** — F9 hold-to-talk in desktop chat via whisper-rs local transcription; models downloaded with SHA-256 verify; audio never leaves the machine. Next: Phase 5 (packaging + launch).

## Working with Claude in this repo

- Treat this CLAUDE.md as the source of truth for product vision and scope.
- Before expanding scope, ask: does this serve the MVP or does it drift toward v1.0+?
- The "agent backend" is the core; other surfaces are UIs over it. When in doubt, strengthen the backend.
- Prefer Rust for anything that runs long-lived or touches the system. Shell scripts only for thin glue.
- Local-first is a hard constraint, not a preference. Any cloud call requires an explicit user opt-in.

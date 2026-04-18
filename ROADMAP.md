# Coati — Build Roadmap

> Master plan. Phase-level strategy. Detailed execution plans live in `plans/`.

**Goal:** Ship a local, private, voice-capable Linux system agent as an installable suite in 8-10 weeks.

**Approach:** Build in 5 sequenced phases. Each phase ends with working, testable software that could stand alone. Phase 1 (agent backend) is the foundation; everything else depends on it.

---

## Phase 1 — Agent Backend (foundation)

**Duration:** weeks 1-3 (~17 working days)
**Depends on:** nothing
**Deliverable:** `coati ask "..."` works end-to-end on the command line with real LLM reasoning and typed tool calls.
**Detailed plan:** [plans/2026-04-17-phase-1-agent-backend.md](plans/2026-04-17-phase-1-agent-backend.md)

**Checkpoint criteria (must pass to exit Phase 1):**
- [ ] `echo "why did nginx fail" | coati ask` returns an AI response after reading logs via the `query_logs` tool
- [ ] All 5 core tools (`exec`, `read_file`, `list_dir`, `query_logs`, `explain_error`) are implemented with tests
- [ ] `coati serve` runs as a daemon exposing a Unix socket for future IPC
- [ ] Config file at `~/.config/coati/config.toml` controls model, endpoint, and tool allowlist
- [ ] CI (GitHub Actions) runs all tests on every push
- [ ] `cargo test --workspace` passes on a fresh clone

---

## Phase 2 — Shell Integration

**Duration:** week 4 (~5 working days)
**Depends on:** Phase 1 (uses Unix socket IPC)
**Deliverable:** zsh + bash plugins that let users invoke Coati inline.

**Scope:**
- `??` command after a failed command → explain the error
- `coati <natural language>` → proposed command + confirm prompt
- Context capture: `pwd`, last exit code, last command, git state
- oh-my-zsh plugin layout + manual bash install script

**Checkpoint criteria:**
- [ ] In a fresh zsh shell: `ls /nonexistent` then `??` produces an explanation
- [ ] `coati restart nginx` shows `sudo systemctl restart nginx` with `[y/N]` prompt
- [ ] Works in bash and zsh identically
- [ ] Completion/confirmation flow has zero network calls beyond the local agent

---

## Phase 3 — Desktop Tray + Chat (parallelizable with Phase 2 if time)

**Duration:** weeks 5-6 (~10 working days)
**Depends on:** Phase 1
**Deliverable:** Tauri tray app with hotkey-summoned chat window.

**Scope:**
- Tauri scaffold (Rust backend + web frontend; consider solid.js or vanilla for tiny bundle)
- Tray icon using simplified 16px glyph variant (needs design work — current logo too detailed at tray size)
- Global hotkey (default Ctrl+Space, configurable)
- Chat window with streaming responses
- Conversation history (SQLite in `~/.local/share/coati/`)
- Model selector dropdown (queries ollama for installed models)
- IPC to agent backend via Unix socket

**Checkpoint criteria:**
- [ ] Tray icon appears on GNOME and KDE
- [ ] Hotkey summons chat window from any focused app
- [ ] Chat streams responses token-by-token
- [ ] History persists across restarts
- [ ] Window closes to tray, not taskbar

---

## Phase 4 — Voice (Push-to-Talk MVP)

**Duration:** week 7 (~5 working days)
**Depends on:** Phase 3 (uses tray for indicator)
**Deliverable:** Hold-hotkey-to-talk voice commands, transcribed locally via whisper.cpp.

**Scope:**
- Bundle whisper.cpp `small.en` model (~500MB) in the package
- Rust bindings (`whisper-rs` crate) or subprocess
- Push-to-talk: hold F9 (configurable) → record → transcribe → pipe to agent
- Tray icon pulses red while recording
- No wake word yet (v1.0 scope)

**Checkpoint criteria:**
- [ ] Hold F9, say "what's the disk usage", release → agent responds with disk usage
- [ ] Latency from release-to-response-start: under 2s on a CPU-only mid-range laptop
- [ ] Audio never leaves the machine (verify with netstat during testing)

---

## Phase 5 — Packaging + Launch

**Duration:** weeks 8-10 (~10 working days)
**Depends on:** all prior phases
**Deliverable:** `curl -fsSL coati.sh/install.sh | sh` on fresh Ubuntu 24.04 produces a working setup.

**Scope:**
- `.deb` package build via `cargo-deb`
- Install script that: adds apt source, installs package, pulls default ollama model, registers systemd user service
- Landing page (coati.sh — static, Astro or Zola)
- Documentation site (mdBook or similar)
- 2-minute demo video (OBS screen capture: voice → sudo action with confirm, end-to-end)
- Launch channels: Show HN, r/linux, r/LocalLLaMA, Hacker News, Twitter

**Checkpoint criteria:**
- [ ] Fresh Ubuntu 24.04 VM → install script → working Coati in under 5 minutes
- [ ] All links in landing page resolve
- [ ] Demo video published and embedded
- [ ] Telemetry/analytics for launch day set up (Plausible or simple logs)

---

## Critical Path + Parallelization

```
Phase 1 ──────────┬─► Phase 2 ──┐
                  │              ├─► Phase 4 ─► Phase 5
                  └─► Phase 3 ───┘
```

Phase 2 and Phase 3 can run in parallel if Phase 1 finishes ahead of schedule. Default plan is sequential for focus.

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| rig.rs API churn mid-build | Medium | Medium | Pin version; build thin wrapper layer so swap is local |
| ollama performance on user hardware varies wildly | High | Low | Document min specs; let user pick smaller model; benchmark on launch |
| whisper.cpp latency unacceptable on older CPUs | Medium | High | Offer optional cloud STT as opt-in; ship `tiny.en` as fallback |
| Tauri global hotkey conflicts across DEs | High | Medium | Make hotkey fully configurable; ship per-DE defaults |
| `.deb` on Fedora/Arch users → bad first impression | High | Medium | Launch Ubuntu-only; add rpm/AUR in v1.1 |
| Users expect wake word and feel cheated without it | Medium | Low | Be explicit in launch copy: "PTT now, wake word in v1.0" |
| Someone clones and rebrands before launch | Low | Low | Trademark "coati" as part of Phase 5 packaging |
| Project loses momentum mid-build | Medium | Critical | Ship demo video at end of each phase; commit publicly to dates |

---

## Working Norms for This Project

- **Commits:** small, focused, conventional-commits style (`feat:`, `fix:`, `chore:`)
- **Tests:** every public function has at least one unit test before it's merged. Integration tests at phase boundaries.
- **TDD:** write the failing test first. See `plans/` for per-phase TDD flows.
- **Branches:** feature branches off `main`; main is always shippable.
- **CI:** runs on every push. Fails fast. No merging red.
- **Local-first:** zero network calls without explicit user opt-in. Audited at phase boundaries.
- **Confirm before sudo:** autopilot is not in scope for MVP or v1.0.

---

## Success Metrics (Launch +30 days)

- [ ] 1,000+ GitHub stars
- [ ] 100+ `.deb` downloads
- [ ] 10+ third-party plugin attempts (signal of platform appeal)
- [ ] Demo video: 10K+ views on YouTube or X
- [ ] 1 front-page HN moment (goal, not promise)

# coati

Your Linux copilot — a local, private, voice-capable system agent.

Ships as a one-installer suite, not a distro. Runs on Ubuntu 24.04+.

## Status

- **Phase 1** ✅ shipped (`v0.0.1-phase1`) — agent backend, 5 typed system tools, Unix-socket daemon
- **Phase 2** ✅ shipped (`v0.0.2-phase2`) — zsh, bash, fish shell plugins with `coati <intent>` and `??`
- **Phase 3** 🚧 next — Tauri tray + chat window

## Quick start

```bash
# 1. Prereqs: Rust 1.82+, ollama (https://ollama.com)
# 2. Build
git clone https://github.com/JuanMarchetto/coati
cd coati
cargo build --release
sudo cp target/release/coati /usr/local/bin/

# 3. First-run setup (TUI: hardware detection, model picker, ollama pull, config write)
coati setup

# 4. Install shell plugin
./shell/install.sh           # auto-detects zsh/bash/fish
. ~/.zshrc                   # reload (or restart terminal)
```

## Usage

```text
$ coati "show disk usage"
$ df -h
  → shows disk usage per mounted filesystem
Run? [y/N] y
Filesystem      Size  Used Avail Use% Mounted on
...

$ ls /nonexistent
ls: cannot access '/nonexistent': No such file or directory
$ ??
No such file or directory: /nonexistent does not exist.
Try: find / -name nonexistent 2>/dev/null
```

## Desktop (optional)

Coati ships an optional Tauri tray app. The CLI and shell plugins work
without it — install only if you want a chat window.

**Prereqs (Ubuntu/Debian):**

```
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

**Install:**

```
cargo build -p coati-desktop --release --features desktop
./scripts/install-desktop.sh
```

**Run:**

```
coati-desktop &
```

**Features:**
- Tray icon with Open Chat / Settings / Quit
- Global hotkey (default Ctrl+Space) — configurable at `~/.config/coati/config.toml`
- Streaming chat via local Unix socket (same daemon the CLI uses)
- Conversation history at `~/.local/share/coati/history.db`
- Confirm-before-sudo for `PROPOSE:` commands from the model

See [CLAUDE.md](CLAUDE.md) for product vision and [ROADMAP.md](ROADMAP.md) for the build plan.

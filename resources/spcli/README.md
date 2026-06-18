# spcli + Claude skill (in-repo copy)

This folder is the in-repo copy of the **spcli** command-line client and its
**Claude skill**, so the team can install it without leaving the repo. It lets
you operate the platform through Claude — just ask things like *"list my
accounts"*, *"create a transaction"*, *"what CFDIs do I have"* and Claude runs
`spcli` for you.

```
SKILL.md              the Claude skill (copy into ~/.claude/skills/spcli/)
reference.md          the full command reference
bin/macos-arm64/spcli prebuilt binary for Apple Silicon Macs
```

> Prebuilt binaries for **Linux** and **Windows** (and Intel Macs) are produced
> by CI and attached to the GitHub **Release** (tag `spcli-v*`). Download the
> archive for your OS there — it includes a one-step installer. This in-repo
> copy ships the Apple-Silicon binary; on other OSes use the release or build
> from source (`cargo build --release -p spcli`).

## Install

### macOS (Apple Silicon) — from this folder
```bash
mkdir -p ~/.local/bin && cp bin/macos-arm64/spcli ~/.local/bin/spcli
chmod +x ~/.local/bin/spcli
xattr -d com.apple.quarantine ~/.local/bin/spcli 2>/dev/null || true   # clear Gatekeeper
mkdir -p ~/.claude/skills/spcli
cp SKILL.md reference.md ~/.claude/skills/spcli/
# ensure ~/.local/bin is on PATH (zsh):
grep -q '.local/bin' ~/.zshrc || echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

### Linux / Intel Mac / Windows — from the GitHub Release
1. Download `spcli-<tag>-<your-os>.{tar.gz,zip}` from the repo's Releases page.
2. Extract it and run the included installer:
   - macOS/Linux: `./install.sh`
   - Windows (PowerShell): `powershell -ExecutionPolicy Bypass -File .\install.ps1`
   The installer puts `spcli` on your PATH and the skill in `~/.claude/skills/spcli`.

### Any OS — build from source
```bash
cargo build --release -p spcli       # binary at target/release/spcli
mkdir -p ~/.claude/skills/spcli && cp SKILL.md reference.md ~/.claude/skills/spcli/
```

## First use
Open Claude in any project and ask a platform question. The first time, Claude
asks for the **login URL** (`https://app.alfredorivera.dev`), your **usuario**
(login id — the CLI flag is `--email`), and your **TOTP secret**. It logs in
once and keeps an encrypted local session.

Verify directly any time:
```bash
spcli --json status
spcli --json manifest     # full machine-readable command list
```

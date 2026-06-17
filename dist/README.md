# spcli + Claude skill

This package lets you operate the Sonora Precision platform through **Claude**,
using the `spcli` command-line client. After installing, just open Claude and ask
things like *"list my accounts"*, *"create a transaction"*, or *"what CFDIs do I
have"* — Claude runs `spcli` for you.

## What's inside

```
spcli (or spcli.exe)     the platform CLI for your OS
install.sh / install.ps1 one-step installer (binary → PATH, skill → ~/.claude/skills)
skills/spcli/SKILL.md     the Claude skill
skills/spcli/reference.md the full command reference
README.md                 this file
```

## Install

**macOS / Linux**

```bash
./install.sh
```

**Windows** (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

The installer copies `spcli` onto your `PATH` and the skill into
`~/.claude/skills/spcli` (so it's available in every Claude session).

> **macOS note:** the binary is unsigned. The installer clears the Gatekeeper
> quarantine flag automatically; if macOS still blocks it, run
> `xattr -d com.apple.quarantine "$(command -v spcli)"` or allow it in
> System Settings → Privacy & Security.

## Use it

1. Open Claude in any project.
2. Ask a platform question — e.g. *"list my accounts in the test company"*.
3. The **first time**, Claude will ask you for:
   - the login URL: **`https://app.alfredorivera.dev`** (the app/login host —
     not a tenant URL),
   - your **email**,
   - your **TOTP secret** (the base32 secret behind your authenticator code).
   It logs you in once and remembers the session (encrypted, locally).

You can also run `spcli` directly:

```bash
spcli --json status
spcli --json login --base-url https://app.alfredorivera.dev --email you@example.com --totp-secret YOURSECRET
spcli --json company use <your-company-slug>
spcli --json finance accounts list
spcli --json manifest        # full list of commands
```

## Security

`spcli` stores your session and TOTP secret in an **encrypted** file under your
user config dir — never commit it or share it. To wipe it: `spcli reset-auth --yes`.
Rotate your TOTP secret server-side if a machine is compromised.

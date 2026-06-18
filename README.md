# alfredodev

Guia rapida para correr el proyecto en local.

## Requisitos

- Rust (cargo) instalado.
- MongoDB en ejecucion (por defecto en `mongodb://localhost:27017`).
- (Opcional) `typst` en el PATH si quieres usar el editor/preview de PDF.

## Configuracion

Variables de entorno soportadas:

- `MONGODB_URI` (default: `mongodb://localhost:27017`)
- `MONGODB_DB` (default: `totp`)
- `USERS_FILE` (default: `./data/users.json`)
- `TYPST_BIN` (default: `typst`)

Puedes crear un archivo `.env` en la raiz con algo como:

```env
MONGODB_URI=mongodb://localhost:27017
MONGODB_DB=totp
USERS_FILE=./data/users.json
TYPST_BIN=typst
```

## Datos iniciales

Al iniciar, si la base esta vacia, se hace seed automatico usando el JSON en `data/users.json`.
El usuario por defecto es:

- `alfredo@example.com`
- secreto TOTP: `KVSYYQOFAACHZYGG7HIA53SUPXHUT4X2`

Con ese secreto puedes registrar un codigo TOTP en tu app de autenticacion (Google Authenticator, 1Password, etc.) y usarlo para el login.

## Correr el servidor

```bash
cargo run
```

El servidor escucha en:

```
http://0.0.0.0:8090
```

## Rutas principales

- `GET /` pagina de login
- `POST /login` valida `{email, code}` con TOTP
- Rutas protegidas bajo sesion:
  - `/admin/...`
  - `/account`
  - `/pdf`
  - `/tiempo`

## Development workflow

This repository uses OpenSpec-style spec-driven workflow for non-trivial changes. See:

- `openspec/README.md`
- `openspec/config.yaml`
- `openspec/specs/`

The harness is local: isolated MongoDB databases, in-memory Axum routers, safe fixtures, and integration tests. It does not use Harness.io.

## CLI / Claude skill (`spcli`)

`spcli` is the first-party command-line client for the platform. It logs in once
with a TOTP secret, keeps an encrypted local session, and exposes ~115 commands
with stable JSON output. It powers a **Claude skill** so you (and your teammates)
can just ask Claude things like *"list my accounts"*, *"create a transaction"*,
or *"what CFDIs do I have"* and Claude runs `spcli` for you.

Full command reference: [`docs/spcli.md`](docs/spcli.md). In-repo copy of the
binary + skill: [`resources/spcli/`](resources/spcli/).

### Install the skill (per OS)

The skill is two files (`SKILL.md`, `reference.md`) copied into
`~/.claude/skills/spcli/`, plus the `spcli` binary on your `PATH`.

**macOS (Apple Silicon)** — use the in-repo binary:
```bash
cd resources/spcli
mkdir -p ~/.local/bin && cp bin/macos-arm64/spcli ~/.local/bin/ && chmod +x ~/.local/bin/spcli
xattr -d com.apple.quarantine ~/.local/bin/spcli 2>/dev/null || true
mkdir -p ~/.claude/skills/spcli && cp SKILL.md reference.md ~/.claude/skills/spcli/
grep -q '.local/bin' ~/.zshrc || echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc && source ~/.zshrc
```

**Linux / Intel Mac / Windows** — download the prebuilt bundle for your OS from
the repo's **Releases** (tags `spcli-v*`, built by CI), extract it, and run the
included installer (`./install.sh`, or on Windows
`powershell -ExecutionPolicy Bypass -File .\install.ps1`). It puts `spcli` on
your `PATH` and installs the skill automatically.

**Any OS — from source:**
```bash
cargo build --release -p spcli      # binary at target/release/spcli
mkdir -p ~/.claude/skills/spcli && cp resources/spcli/SKILL.md resources/spcli/reference.md ~/.claude/skills/spcli/
```

### Use it

Open Claude in any project and ask a platform question. The **first time**,
Claude asks for the login URL (`https://app.alfredorivera.dev`), your **usuario**
(login id — the CLI flag is still `--email`), and your **TOTP secret**; it logs
in once and remembers the session. Or drive it directly:

```bash
spcli --json status
spcli --json login --base-url https://app.alfredorivera.dev --email you@example.com --totp-secret YOURSECRET
spcli --json company use <your-company-slug>
spcli --json finance accounts list
spcli --json manifest          # full machine-readable command catalog
```

See [`docs/spcli.md`](docs/spcli.md) for full auth, company selection, and
credential-storage details.

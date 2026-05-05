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


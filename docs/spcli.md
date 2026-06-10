# spcli

`spcli` is the command-line client for the application APIs. It is designed for human operators first, with stable JSON output and command metadata so a future AI skill can automate it safely.

## Authentication

Configure the CLI once with the server URL, email, and TOTP secret:

```bash
cargo run --bin spcli -- login \
  --base-url http://localhost:8090 \
  --email alfredo@example.com \
  --totp-secret YOUR_BASE32_TOTP_SECRET
```

The CLI stores credentials outside the repository at the user's config path, for example:

```text
~/.config/spcli/credentials.bin
```

The credential file is a binary encrypted envelope with restrictive permissions where supported. It is not plaintext JSON and should not reveal credentials when opened in a text editor.

If a protected request receives `401 Unauthorized`, `spcli` generates a fresh TOTP code from the stored secret, logs in again, updates the local session cookie, and retries the request once.

## Commands

Check the current session:

```bash
cargo run --bin spcli -- status
```

List companies available to the user:

```bash
cargo run --bin spcli -- company list
```

Select active company context:

```bash
cargo run --bin spcli -- company use acme
```

Clear only the current session cookie:

```bash
cargo run --bin spcli -- logout
```

Remove the stored credential file:

```bash
cargo run --bin spcli -- reset-auth --yes
```

Print machine-readable command metadata:

```bash
cargo run --bin spcli -- --json manifest
```

List read-only API data for the selected company:

```bash
cargo run --bin spcli -- finance accounts list
cargo run --bin spcli -- finance categories list
cargo run --bin spcli -- finance contacts list
cargo run --bin spcli -- finance forecasts list
cargo run --bin spcli -- finance recurring-plans list
cargo run --bin spcli -- finance planned-entries list
cargo run --bin spcli -- finance transactions list
cargo run --bin spcli -- cfdi list
cargo run --bin spcli -- projects list
cargo run --bin spcli -- projects statuses list
cargo run --bin spcli -- projects concepts list --project-id 64f000000000000000000000
cargo run --bin spcli -- resources list
cargo run --bin spcli -- resources logs list
cargo run --bin spcli -- resources usages list
```

Read one finance master-data record by MongoDB ObjectId:

```bash
cargo run --bin spcli -- finance accounts get 64f000000000000000000000
cargo run --bin spcli -- finance categories get 64f000000000000000000000
cargo run --bin spcli -- finance contacts get 64f000000000000000000000
cargo run --bin spcli -- finance forecasts get 64f000000000000000000000
cargo run --bin spcli -- finance recurring-plans get 64f000000000000000000000
cargo run --bin spcli -- finance planned-entries get 64f000000000000000000000
cargo run --bin spcli -- finance transactions get 64f000000000000000000000
cargo run --bin spcli -- cfdi get 12345678-1234-1234-1234-1234567890ab
cargo run --bin spcli -- projects get 64f000000000000000000000
cargo run --bin spcli -- resources get 64f000000000000000000000
cargo run --bin spcli -- resources logs get 64f000000000000000000000
```

Create finance master-data records:

```bash
cargo run --bin spcli -- finance accounts create --name "BBVA" --account-type bank --currency MXN
cargo run --bin spcli -- finance categories create --name "Services" --flow-type expense
cargo run --bin spcli -- finance contacts create --name "Customer SA" --contact-type customer --rfc XAXX010101000
cargo run --bin spcli -- finance forecasts create --generated-at 2026-01-01 --start-date 2026-01-01 --end-date 2026-12-31 --currency MXN --projected-income-total 1000 --projected-expense-total 500 --projected-net 500
```

Update or delete finance master-data records:

```bash
cargo run --bin spcli -- finance accounts update 64f000000000000000000000 --name "BBVA" --account-type bank --currency MXN
cargo run --bin spcli -- finance categories update 64f000000000000000000000 --name "Services" --flow-type expense
cargo run --bin spcli -- finance contacts update 64f000000000000000000000 --name "Customer SA" --contact-type customer
cargo run --bin spcli -- finance forecasts update 64f000000000000000000000 --generated-at 2026-01-01 --start-date 2026-01-01 --end-date 2026-12-31 --currency MXN --projected-income-total 1000 --projected-expense-total 500 --projected-net 500
cargo run --bin spcli -- finance contacts delete 64f000000000000000000000 --yes
```

Query the time timeline. `--from` and `--to` accept RFC3339 datetimes or `YYYY-MM-DD` dates, which are sent as midnight UTC:

```bash
cargo run --bin spcli -- time timeline --mode month --from 2026-01-01 --to 2026-12-31
```

Preview Typst PDF content from a file, inline string, or stdin:

```bash
cargo run --bin spcli -- pdf preview --input invoice.typ --output invoice.pdf
cargo run --bin spcli -- pdf preview --source "= Hello"
```

## JSON Output

Use `--json` for automation:

```bash
cargo run --bin spcli -- --json status
cargo run --bin spcli -- --json company list
cargo run --bin spcli -- --json finance accounts list
cargo run --bin spcli -- --json finance transactions list
cargo run --bin spcli -- --json cfdi list
```

Errors are written as structured JSON to stderr with a stable `code` and `message` field.

## Security Notes

The stored TOTP secret can generate valid login codes. If the credential file is stolen and decrypted, rotate the user's TOTP secret server-side and run:

```bash
cargo run --bin spcli -- reset-auth
```

Do not commit `credentials.bin`, generated TOTP codes, session cookies, or real TOTP secrets.

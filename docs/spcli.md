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

Read or update the current account profile. Profile output omits the TOTP secret; updates read a replacement secret from the environment:

```bash
cargo run --bin spcli -- account get
SPCLI_TOTP_SECRET=YOUR_BASE32_TOTP_SECRET cargo run --bin spcli -- account update --email operator@example.com --totp-secret-env SPCLI_TOTP_SECRET
```

List companies available to the user:

```bash
cargo run --bin spcli -- company list
```

Manage company metadata through admin JSON APIs:

```bash
cargo run --bin spcli -- admin companies list
cargo run --bin spcli -- admin companies get 64f000000000000000000000
cargo run --bin spcli -- admin companies create --name "Acme" --slug acme --default-currency MXN
cargo run --bin spcli -- admin companies update 64f000000000000000000000 --name "Acme" --slug acme --default-currency MXN
```

Company delete and company-wide maintenance deletes are intentionally unsupported in `spcli`.

Manage company users (admin only, scoped to companies you administer). On create the server generates a TOTP secret when `--secret-env` is omitted; the secret is never printed — read the new user's QR from the web app. Use `--input <file.json>` to send a full payload with multiple memberships:

```bash
cargo run --bin spcli -- admin users list
cargo run --bin spcli -- admin users get 64f000000000000000000000
cargo run --bin spcli -- admin users create --email operator@example.com --company-id 64f000000000000000000000 --role staff --permission view_projects --permission view_timeline
SPCLI_NEW_USER_SECRET=YOUR_BASE32_SECRET cargo run --bin spcli -- admin users create --email operator@example.com --company-id 64f000000000000000000000 --role admin --secret-env SPCLI_NEW_USER_SECRET
cargo run --bin spcli -- admin users create --input new-user.json
cargo run --bin spcli -- admin users update 64f000000000000000000000 --email operator@example.com --company-id 64f000000000000000000000 --role admin
cargo run --bin spcli -- admin users delete 64f000000000000000000000 --yes
```

The `--input` JSON mirrors the API body, e.g.:

```json
{ "email": "operator@example.com", "memberships": [ { "company_id": "64f000000000000000000000", "role": "admin", "permissions": ["view_projects"] } ] }
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
cargo run --bin spcli -- cfdi jobs list
cargo run --bin spcli -- sat configs list
cargo run --bin spcli -- admin users list
cargo run --bin spcli -- projects list
cargo run --bin spcli -- projects statuses list
cargo run --bin spcli -- projects concepts list --project-id 64f000000000000000000000
cargo run --bin spcli -- projects status-summary --project-id 64f000000000000000000000
cargo run --bin spcli -- orders list
cargo run --bin spcli -- resources list
cargo run --bin spcli -- resources logs list
cargo run --bin spcli -- resources usages list
cargo run --bin spcli -- resources usages allocations list 64f000000000000000000000
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
cargo run --bin spcli -- cfdi jobs status 12345678-1234-1234-1234-1234567890ab
cargo run --bin spcli -- sat configs get 64f000000000000000000000
cargo run --bin spcli -- projects get 64f000000000000000000000
cargo run --bin spcli -- orders get 64f000000000000000000000
cargo run --bin spcli -- resources get 64f000000000000000000000
cargo run --bin spcli -- resources logs get 64f000000000000000000000
cargo run --bin spcli -- resources usages get 64f000000000000000000000
```

Create finance master-data records:

```bash
cargo run --bin spcli -- finance accounts create --name "BBVA" --account-type bank --currency MXN
cargo run --bin spcli -- finance categories create --name "Services" --flow-type expense
cargo run --bin spcli -- finance contacts create --name "Customer SA" --contact-type customer --rfc XAXX010101000
cargo run --bin spcli -- finance forecasts create --generated-at 2026-01-01 --start-date 2026-01-01 --end-date 2026-12-31 --currency MXN --projected-income-total 1000 --projected-expense-total 500 --projected-net 500
cargo run --bin spcli -- finance recurring-plans create --name "Monthly Rent" --flow-type expense --category-id 64f000000000000000000000 --account-expected-id 64f000000000000000000001 --amount-estimated 1000 --frequency monthly --day-of-month 1 --start-date 2026-07-01T00:00:00Z
cargo run --bin spcli -- finance planned-entries create --name "Fuel" --flow-type expense --category-id 64f000000000000000000000 --account-expected-id 64f000000000000000000001 --amount-estimated 500 --due-date 2026-07-01T00:00:00Z
cargo run --bin spcli -- finance transactions create --date 2026-07-01T12:00:00Z --description "Fuel" --transaction-type expense --category-id 64f000000000000000000000 --account-from-id 64f000000000000000000001 --amount 500
cargo run --bin spcli -- sat configs create --rfc XAXX010101000 --cer-path /secure/sat/cert.cer --key-path /secure/sat/private.key --key-password-env SAT_KEY_PASSWORD --label "Main FIEL"
SAT_KEY_PASSWORD=secret cargo run --bin spcli -- sat configs upload --rfc XAXX010101000 --cer-file ./cert.cer --key-file ./private.key --key-password-env SAT_KEY_PASSWORD --label "Main FIEL"
cargo run --bin spcli -- projects create --title "Warehouse install" --category-id 64f000000000000000000000 --priority high --total-budget 50000 --scheduled-at 2026-07-01T00:00:00Z
cargo run --bin spcli -- orders create --title "Concrete pour" --category-id 64f000000000000000000000 --account-id 64f000000000000000000001 --status confirmed --amount 2500 --scheduled-at 2026-07-01T08:00:00Z --item "Labor:4:250" --item "Concrete:10:150"
cargo run --bin spcli -- projects statuses create --name "In Progress" --position 10 --color sky
cargo run --bin spcli -- projects concepts create --project-id 64f000000000000000000000 --name "Foundation" --quantity 1 --unit job --position 1
cargo run --bin spcli -- resources create --name "Excavator" --resource-type machinery --hourly-cost 350 --currency MXN --allowed-status-id 64f000000000000000000000
cargo run --bin spcli -- resources logs create --resource-id 64f000000000000000000000 --started-at 2026-06-01T10:00:00Z --operator-name "Operator"
cargo run --bin spcli -- resources usages create --resource-id 64f000000000000000000000 --started-at 2026-06-01T10:00:00Z --ended-at 2026-06-01T12:00:00Z --operator-name "Operator"
```

Update or delete finance master-data records:

```bash
cargo run --bin spcli -- finance accounts update 64f000000000000000000000 --name "BBVA" --account-type bank --currency MXN
cargo run --bin spcli -- finance categories update 64f000000000000000000000 --name "Services" --flow-type expense
cargo run --bin spcli -- finance contacts update 64f000000000000000000000 --name "Customer SA" --contact-type customer
cargo run --bin spcli -- finance forecasts update 64f000000000000000000000 --generated-at 2026-01-01 --start-date 2026-01-01 --end-date 2026-12-31 --currency MXN --projected-income-total 1000 --projected-expense-total 500 --projected-net 500
cargo run --bin spcli -- finance recurring-plans update 64f000000000000000000000 --name "Monthly Rent" --flow-type expense --category-id 64f000000000000000000001 --account-expected-id 64f000000000000000000002 --amount-estimated 1100 --frequency monthly --day-of-month 1 --start-date 2026-07-01T00:00:00Z
cargo run --bin spcli -- finance recurring-plans generate 64f000000000000000000000
cargo run --bin spcli -- finance recurring-plans delete 64f000000000000000000000 --yes
cargo run --bin spcli -- finance planned-entries update 64f000000000000000000000 --name "Fuel" --flow-type expense --category-id 64f000000000000000000001 --account-expected-id 64f000000000000000000002 --amount-estimated 550 --due-date 2026-07-02T00:00:00Z
cargo run --bin spcli -- finance planned-entries pay 64f000000000000000000000 --paid-at 2026-07-03T00:00:00Z --amount 550 --account-id 64f000000000000000000002
cargo run --bin spcli -- finance planned-entries bulk-pay --entry-id 64f000000000000000000000 --entry-id 64f000000000000000000001 --paid-at 2026-07-03T00:00:00Z --account-id 64f000000000000000000002
cargo run --bin spcli -- finance planned-entries delete 64f000000000000000000000 --yes
cargo run --bin spcli -- finance transactions update 64f000000000000000000000 --date 2026-07-01T12:00:00Z --description "Fuel" --transaction-type expense --category-id 64f000000000000000000001 --account-from-id 64f000000000000000000002 --amount 550
cargo run --bin spcli -- finance transactions delete 64f000000000000000000000 --yes
cargo run --bin spcli -- finance contacts delete 64f000000000000000000000 --yes
cargo run --bin spcli -- sat configs update 64f000000000000000000000 --rfc XAXX010101000 --cer-path /secure/sat/cert.cer --key-path /secure/sat/private.key --key-password-env SAT_KEY_PASSWORD --label "Updated FIEL"
cargo run --bin spcli -- sat configs delete 64f000000000000000000000 --yes
cargo run --bin spcli -- projects update 64f000000000000000000000 --title "Warehouse install" --category-id 64f000000000000000000001 --priority urgent --total-budget 55000
cargo run --bin spcli -- projects advance 64f000000000000000000000
cargo run --bin spcli -- projects delete 64f000000000000000000000 --yes
cargo run --bin spcli -- orders update 64f000000000000000000000 --title "Concrete pour" --category-id 64f000000000000000000001 --account-id 64f000000000000000000002 --status in_progress --amount 2750 --item "Labor:5:250" --item "Concrete:10:150"
cargo run --bin spcli -- orders complete 64f000000000000000000000
cargo run --bin spcli -- orders delete 64f000000000000000000000 --yes
cargo run --bin spcli -- resources update 64f000000000000000000000 --name "Excavator" --resource-type machinery --hourly-cost 375 --currency MXN --allowed-status-id 64f000000000000000000001
cargo run --bin spcli -- resources delete 64f000000000000000000000 --yes
cargo run --bin spcli -- resources logs update 64f000000000000000000000 --resource-id 64f000000000000000000001 --started-at 2026-06-01T10:00:00Z --ended-at 2026-06-01T12:00:00Z
cargo run --bin spcli -- resources logs end 64f000000000000000000000 --ended-at 2026-06-01T12:00:00Z
cargo run --bin spcli -- resources logs delete 64f000000000000000000000 --yes
cargo run --bin spcli -- resources usages update 64f000000000000000000000 --started-at 2026-06-01T10:00:00Z --ended-at 2026-06-01T12:00:00Z --hourly-cost-snapshot 250
cargo run --bin spcli -- resources usages delete 64f000000000000000000000 --yes
cargo run --bin spcli -- projects statuses update 64f000000000000000000000 --name "Done" --position 20 --terminal
cargo run --bin spcli -- projects statuses delete 64f000000000000000000000 --yes
cargo run --bin spcli -- projects concepts update 64f000000000000000000000 --name "Foundation" --quantity 2 --status-id 64f000000000000000000001 --position 1
cargo run --bin spcli -- projects concepts advance 64f000000000000000000000
cargo run --bin spcli -- projects concepts delete 64f000000000000000000000 --yes
cargo run --bin spcli -- resources usages allocations replace 64f000000000000000000000 --concept-id 64f000000000000000000001 --concept-id 64f000000000000000000002
cargo run --bin spcli -- resources usages allocations replace 64f000000000000000000000 --allocation 64f000000000000000000001:0.7 --allocation 64f000000000000000000002:0.3:extra-work
```

Bulk-save the hourly resource-usage grid for one day. Each `--cell` is `concept_id:hour:resource_id`; `--status-id` scopes to one concept status (or `all`). Staff need the `edit_resource_usage_today` permission and may only save within the allowed recent-day window:

```bash
cargo run --bin spcli -- resources usages grid --date 2026-06-17 --status-id all --cell 64f000000000000000000001:8:64f000000000000000000002 --cell 64f000000000000000000001:9:64f000000000000000000002
cargo run --bin spcli -- resources usages grid --input grid.json
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

CFDI download jobs are server in-memory records. `cfdi jobs list` and `cfdi jobs status <job-id>` only show jobs currently held by the running server process; status is lost when the app restarts unless persistent jobs are added later.

Initiating a SAT CFDI download is intentionally not yet exposed in `spcli`: the current server route accepts a browser form rather than a JSON body, and the operation has external SAT side effects and job-lifecycle concerns that need a dedicated JSON endpoint and design pass first. Until then, trigger downloads from the web app; `spcli` can still read and poll the resulting jobs.

## JSON Output

Use `--json` for automation:

```bash
cargo run --bin spcli -- --json status
cargo run --bin spcli -- --json company list
cargo run --bin spcli -- --json finance accounts list
cargo run --bin spcli -- --json finance transactions list
cargo run --bin spcli -- --json cfdi list
cargo run --bin spcli -- --json cfdi jobs list
```

Errors are written as structured JSON to stderr with a stable `code` and `message` field.

## Security Notes

The stored TOTP secret can generate valid login codes. If the credential file is stolen and decrypted, rotate the user's TOTP secret server-side and run:

```bash
cargo run --bin spcli -- reset-auth
```

Do not commit `credentials.bin`, generated TOTP codes, session cookies, or real TOTP secrets.

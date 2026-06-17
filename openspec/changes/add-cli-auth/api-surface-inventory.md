# spcli API Surface Inventory

This inventory maps the current web application route surface before expanding `spcli` beyond authentication and company selection. It intentionally documents the existing backend behavior first so CLI work can avoid scraping HTML pages or bypassing backend authorization.

## Current spcli Coverage

| CLI area | Current commands | Backend routes consumed | Status |
| --- | --- | --- | --- |
| Authentication | `login`, `status`, `logout`, `reset-auth` | `POST /login`, `GET /setup`, `POST /logout` | Covered |
| Account profile | `account get`, `account update` | `GET/POST /api/account` | Covered; profile reads redact TOTP setup data |
| Company selection | `company list`, `company use` | `GET /api/me/companies` | Covered |
| Company admin | `admin companies list/get/create/update` | `/api/admin/companies*` | Covered for metadata; delete and maintenance remain unsupported |
| Finance | `finance accounts/categories/contacts/forecasts/recurring-plans/planned-entries/transactions ...` | `/api/admin/*` finance routes | Covered for implemented CRUD and side-effect commands |
| Orders | `orders list/get/create/update/delete/complete` | `/api/admin/orders*` | Covered |
| Projects | `projects list/get/create/update/delete/advance/status-summary/statuses/concepts ...` | `/api/admin/projects*`, `/api/admin/concept_statuses*`, `/api/admin/project_concepts*` | Covered |
| Resources | `resources list/get/create/update/delete/logs/usages/allocations ...` | `/api/admin/resources*`, `/api/admin/resource_logs*`, `/api/admin/resource_usages*` | Covered |
| SAT configs | `sat configs list/get/create/update/delete` | `/api/admin/sat-configs*` | Covered with redacted output and env-based password input |
| CFDI | `cfdi list/get/jobs list/jobs status` | `/api/admin/cfdis*`, `/admin/companies/{id}/cfdi/jobs*` | Reads and job status covered; import/download still pending |
| Time/PDF | `time timeline`, `pdf preview` | `GET /api/tiempo`, `POST /pdf/preview` | Covered |
| Machine metadata | `manifest` | local manifest | Covered for implemented commands |

## Route Protection Boundary

| Boundary | Routes | Notes |
| --- | --- | --- |
| Public | `GET /`, `POST /login` | No stored session required. |
| Protected | All routes merged through `protected` in `src/main.rs` | Requires session middleware and active user context. |
| Tenant context | Protected business routes | Must use the selected company/tenant host context or active company from session. |
| Admin checks | Most `/admin/*` and `/api/admin/*` routes | Handlers must enforce admin/permission checks, not templates only. |

## JSON-Ready Routes

These routes already return JSON responses and can be consumed by `spcli` once command wrappers, request helpers, validation, documentation, and tests exist.

| Area | Method and route | Existing behavior | CLI candidate |
| --- | --- | --- | --- |
| Auth | `POST /login` | Login with email and TOTP code. | `spcli login` |
| Auth | `POST /logout` | Deletes server session and clears local CLI session state. | `spcli logout` |
| Setup/profile | `GET /setup` | Returns current user profile and setup data. Contains `otpauth_url`; CLI must keep sanitizing it. | `spcli status` |
| Setup/profile | `GET /api/me/companies` | Lists companies available to the current user. | `spcli company list/use` |
| Secret utility | `GET /secret` | Generates a new TOTP secret. Protected by current wiring. | Optional; likely not a normal CLI workflow because secrets are sensitive. |
| Time | `GET /api/tiempo` | Returns timeline buckets for the time view. | `spcli time timeline` |
| SAT direct download | `POST /api/sat/cfdi/download` | Runs a SAT CFDI download request directly from JSON payload. | Needs security review before CLI exposure. |
| PDF | `POST /pdf/preview` | Returns PDF preview/render result for submitted Typst content. | `spcli pdf preview` |
| CFDI | `GET /api/admin/cfdis/data` | Lists CFDI dashboard data for the active company. | `spcli cfdi list` |
| CFDI jobs | `POST /admin/companies/{id}/cfdi/download` | Starts monthly CFDI download jobs from form payload; returns JSON. | `spcli cfdi download start` |
| CFDI jobs | `GET /admin/companies/{id}/cfdi/jobs` | Lists in-memory CFDI jobs for a company. | `spcli cfdi jobs list` |
| CFDI jobs | `GET /admin/companies/{id}/cfdi/jobs/{job_id}` | Returns one in-memory CFDI job status. | `spcli cfdi jobs status` |
| Finance dashboard | `GET /api/admin/transactions/data` | Lists transaction dashboard rows for active company. | `spcli finance transactions list` read-only shortcut |
| Project statuses | `GET /api/admin/concept_statuses` | Lists concept statuses. | `spcli projects statuses list` |
| Project statuses | `POST /api/admin/concept_statuses` | Creates a concept status. | `spcli projects statuses create` |
| Project statuses | `POST /api/admin/concept_statuses/{id}/update` | Updates a concept status. | `spcli projects statuses update` |
| Project statuses | `POST /api/admin/concept_statuses/{id}/delete` | Deletes a concept status. | `spcli projects statuses delete --yes` |
| Project concepts | `GET /api/admin/projects/{project_id}/concepts` | Lists project concepts. | `spcli projects concepts list` |
| Project concepts | `POST /api/admin/projects/{project_id}/concepts` | Creates a project concept. | `spcli projects concepts create` |
| Project summary | `GET /api/admin/projects/{project_id}/status_summary` | Returns project status summary. | `spcli projects status-summary` |
| Project concepts | `POST /api/admin/project_concepts/{id}/update` | Updates a project concept. | `spcli projects concepts update` |
| Project concepts | `POST /api/admin/project_concepts/{id}/advance` | Advances concept status. | `spcli projects concepts advance` |
| Project concepts | `POST /api/admin/project_concepts/{id}/delete` | Deletes a project concept. | `spcli projects concepts delete --yes` |
| Resource usages | `GET /api/admin/resource_usages` | Lists resource usages. | `spcli resources usages list` |
| Resource usages | `POST /api/admin/resource_usages` | Creates resource usage. | `spcli resources usages create` |
| Resource usages | `POST /api/admin/resource_usages/{id}/update` | Updates resource usage. | `spcli resources usages update` |
| Resource usages | `POST /api/admin/resource_usages/{id}/delete` | Deletes resource usage. | `spcli resources usages delete --yes` |
| Resource usage allocations | `GET /api/admin/resource_usages/{id}/allocations` | Lists usage allocations. | `spcli resources usages allocations list` |
| Resource usage allocations | `POST /api/admin/resource_usages/{id}/allocations` | Replaces usage allocations. | `spcli resources usages allocations replace` |

## Original HTML/Form-Only Routes And Current Status

These routes originally rendered Askama templates or consumed browser form submissions with redirects. `spcli` should not scrape these pages. Completed areas now have explicit JSON endpoints; pending areas still need a CLI-grade JSON contract.

| Area | Existing routes | Current status |
| --- | --- | --- |
| Account | `GET/POST /account` | Covered by `GET/POST /api/account`; reads redact TOTP secret and setup URLs. |
| Users | `/admin/users`, `/admin/users/new`, `/admin/users/{id}/edit`, `/update`, `/delete`, `/qrcode` | Covered by `/api/admin/users*` (list/get/create/update/delete), admin-only and tenant-scoped. Secret never returned in JSON; new-user QR/secret read via the existing `/admin/users/{id}/qrcode`. `spcli admin users` CLI commands still pending. |
| Companies | `/admin/companies`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/cfdis/delete_all`, `/transactions/delete_all` | Metadata list/get/create/update covered by `/api/admin/companies*`; delete and delete-all maintenance remain intentionally unsupported. |
| SAT configs | `/admin/companies/{id}/sat_configs`, `/new`, `/{config_id}/delete` | Covered by `/api/admin/sat-configs*`; output redacts secret-bearing fields. File upload (`.cer`/`.key`) now covered by `POST /api/admin/sat-configs/upload` (multipart). |
| Finance accounts | `/admin/accounts`, `/new`, `/{id}/edit`, `/update`, `/delete` | Covered by `/api/admin/accounts*`. |
| Finance categories | `/admin/categories`, `/new`, `/{id}/edit`, `/update`, `/delete` | Covered by `/api/admin/categories*`. |
| Finance contacts | `/admin/contacts`, `/new`, `/{id}/edit`, `/update`, `/delete` | Covered by `/api/admin/contacts*`. |
| Recurring plans | `/admin/recurring_plans`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/{id}/generate` | Covered by `/api/admin/recurring-plans*` with side-effect summaries. |
| Planned entries | `/admin/planned_entries`, `/new`, `/bulk_pay`, `/{id}/edit`, `/update`, `/delete`, `/{id}/pay` | Covered by `/api/admin/planned-entries*` with payment side-effect summaries. |
| Transactions | `/admin/transactions`, `/new`, `/{id}/edit`, `/update`, `/delete` | Covered by `/api/admin/transactions*` with planned-entry recalculation summaries. |
| Forecasts | `/admin/forecasts`, `/new`, `/{id}/edit`, `/update`, `/delete` | Covered by `/api/admin/forecasts*`. |
| Service orders | `/admin/orders`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/{id}/complete` | Covered by `/api/admin/orders*` with transaction/planned-entry side-effect summaries. |
| Projects | `/admin/projects`, `/new`, `/{id}`, `/{id}/edit`, `/update`, `/delete`, `/{id}/advance` | Covered by `/api/admin/projects*`. |
| Project concept forms | `/admin/projects/{project_id}/concepts/new`, `/admin/project_concepts/{id}/edit`, form update/advance/delete routes | Covered through `/api/admin/projects/{project_id}/concepts` and `/api/admin/project_concepts*`. |
| Concept status forms | `/admin/concept_statuses`, `/new`, `/{id}/edit`, form update/delete routes | Covered through `/api/admin/concept_statuses*`. |
| Resources | `/admin/resources`, `/new`, `/{id}/edit`, `/update`, `/delete` | Covered by `/api/admin/resources*`. |
| Resource logs | `/admin/resource_logs`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/{id}/end` | Covered by `/api/admin/resource_logs*`. |
| Resource usage forms/grid | `/admin/resource_usages`, `/create`, `/new`, `/{id}/edit`, `/update`, `/delete` | Individual usages and allocations are covered; bulk grid save now covered by `POST /api/admin/resource_usages/grid` (honors the staff "today window" permission). |
| CFDI HTML list | `GET /admin/cfdis` | List/detail reads covered; CLI-safe import/download remains pending. |
| PDF editor | `GET /pdf` | Human editor only; CLI uses `POST /pdf/preview`. |
| Time page | `GET /tiempo` | Human page only; CLI uses `GET /api/tiempo`. |

## CLI Expansion Order

1. Done: shared JSON `GET`/`POST` helpers, tenant-aware host context, transparent re-login retry, response status mapping, JSON output, and destructive confirmation enforcement.
2. Done: read-only commands for existing JSON APIs including time, CFDI reads, transaction reads, project concepts/statuses, resource usages, and PDF preview.
3. Done: `spcli manifest` entries for implemented commands with arguments, auth requirements, company requirements, destructive flags, and output schemas.
4. Done: JSON backend endpoints and CLI commands for finance master data, forecasts, transactions, planned entries, recurring plans, service orders, projects, resources, resource logs, resource usages, SAT configs, company metadata, and account profile.
5. Done: representative harness coverage for JSON APIs, redaction, tenant isolation, forbidden cases, side effects, CLI manifest output, structured errors, and destructive confirmation checks.
6. Done (backend): users admin JSON APIs (`/api/admin/users*`), the resource usage grid bulk-save JSON API, and SAT config file upload. The full JSON API is now documented via OpenAPI/Swagger at `/docs` + `/api-docs/openapi.json`, gated behind the session middleware. Next: the `spcli admin users` CLI commands.
7. Next: add CLI-safe CFDI import/download commands after documenting secret handling, in-memory job limits, and operational side effects.
8. Later: add optional OS keyring support, stronger local key derivation, richer machine-readable API metadata, and any higher-risk maintenance operations only after a future spec adds audit/recovery requirements.

## Resolved Design Decisions

1. Command names use canonical English nouns only; no Spanish or shorthand aliases in the initial CLI coverage.
2. New backend APIs should be explicit JSON routes, preferably under `/api/admin/*` for administrative capabilities, instead of content negotiation on HTML routes.
3. `spcli` must not scrape HTML, submit browser forms as a stable contract, or depend on redirects as command behavior.
4. Dangerous company maintenance commands, including company delete, delete all CFDIs, and delete all transactions, stay out of the CLI for now.
5. SAT config reads return redacted metadata only. Do not print certificate paths, key paths, passwords, certificate/key contents, TOTP secrets, cookies, generated TOTP codes, or `otpauth_url`.
6. CFDI job commands may use current in-memory jobs, but documentation must state that job status is lost on server restart. Persistent jobs are a separate future improvement.
7. List commands use explicit pagination and filters. Default limit is 100 and maximum limit is at most 5000 unless a command-specific spec says otherwise.
8. MongoDB `ObjectId` is the canonical identifier for records. Name/slug lookup can be added later only where there is a stable model field or a future spec requires it.
9. Human output should be concise tables or summaries. `--json` output should return the complete stable structured response.
10. Mutations should support simple flags where practical and `--input <file.json>` for large or nested payloads.
11. Financial/admin dry-run, idempotency keys, and audit logging are future requirements for higher-risk mutations, not blockers for read-only JSON-ready commands.
12. Backend handlers remain the authority for permissions, tenant isolation, validation, and side effects. CLI validation is only a client-side convenience.

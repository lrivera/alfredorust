# spcli API Surface Inventory

This inventory maps the current web application route surface before expanding `spcli` beyond authentication and company selection. It intentionally documents the existing backend behavior first so CLI work can avoid scraping HTML pages or bypassing backend authorization.

## Current spcli Coverage

| CLI area | Current commands | Backend routes consumed | Status |
| --- | --- | --- | --- |
| Authentication | `login`, `logout`, `reset-auth` | `POST /login`, `POST /logout` | Covered |
| Profile/status | `status` | `GET /setup` | Covered, sanitized client-side |
| Company selection | `company list`, `company use` | `GET /api/me/companies` | Covered |
| Machine metadata | `manifest` | local manifest | Partial; needs full command metadata as commands are added |

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

## HTML/Form-Only Routes Needing API Work

These routes currently render Askama templates or consume browser form submissions with redirects. `spcli` should not scrape these pages. Add JSON endpoints or dual-mode handlers before implementing CLI commands for them.

| Area | Existing routes | Missing CLI-grade API capability |
| --- | --- | --- |
| Account | `GET/POST /account` | Profile/account read and update JSON contract. |
| Users | `/admin/users`, `/admin/users/new`, `/admin/users/{id}/edit`, `/update`, `/delete`, `/qrcode` | User list/get/create/update/delete and QR/setup API with admin checks. |
| Companies | `/admin/companies`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/cfdis/delete_all`, `/transactions/delete_all` | Company admin API and explicit destructive maintenance endpoints with confirmation semantics. |
| SAT configs | `/admin/companies/{id}/sat_configs`, `/new`, `/{config_id}/delete` | SAT config list/get/create/update/delete JSON API; must avoid leaking certificate password material. |
| Finance accounts | `/admin/accounts`, `/new`, `/{id}/edit`, `/update`, `/delete` | Account list/get/create/update/delete JSON API. |
| Finance categories | `/admin/categories`, `/new`, `/{id}/edit`, `/update`, `/delete` | Category list/get/create/update/delete JSON API. |
| Finance contacts | `/admin/contacts`, `/new`, `/{id}/edit`, `/update`, `/delete` | Contact list/get/create/update/delete JSON API. |
| Recurring plans | `/admin/recurring_plans`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/{id}/generate` | Recurring plan CRUD/generate JSON API with side-effect summary. |
| Planned entries | `/admin/planned_entries`, `/new`, `/bulk_pay`, `/{id}/edit`, `/update`, `/delete`, `/{id}/pay` | Planned entry CRUD/pay/bulk-pay JSON API with transaction side-effect summary. |
| Transactions | `/admin/transactions`, `/new`, `/{id}/edit`, `/update`, `/delete` | Full transaction CRUD JSON API; existing `/api/admin/transactions/data` is list-only dashboard data. |
| Forecasts | `/admin/forecasts`, `/new`, `/{id}/edit`, `/update`, `/delete` | Forecast CRUD JSON API. |
| Service orders | `/admin/orders`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/{id}/complete` | Order CRUD/complete JSON API with transaction/planned-entry side-effect summary. |
| Projects | `/admin/projects`, `/new`, `/{id}`, `/{id}/edit`, `/update`, `/delete`, `/{id}/advance` | Project list/get/create/update/delete/advance JSON API. Existing concept APIs do not cover project CRUD. |
| Project concept forms | `/admin/projects/{project_id}/concepts/new`, `/admin/project_concepts/{id}/edit`, form update/advance/delete routes | Already has JSON API equivalents for most concept actions; CLI should use `/api/admin/*` variants. |
| Concept status forms | `/admin/concept_statuses`, `/new`, `/{id}/edit`, form update/delete routes | Already has JSON API equivalents; CLI should use `/api/admin/concept_statuses*`. |
| Resources | `/admin/resources`, `/new`, `/{id}/edit`, `/update`, `/delete` | Resource list/get/create/update/delete JSON API. |
| Resource logs | `/admin/resource_logs`, `/new`, `/{id}/edit`, `/update`, `/delete`, `/{id}/end` | Resource log CRUD/end JSON API. Distinguish logs from resource usages. |
| Resource usage forms/grid | `/admin/resource_usages`, `/create`, `/new`, `/{id}/edit`, `/update`, `/delete` | Existing JSON API covers individual usages and allocations; grid save needs either explicit CLI support or stays web-only. |
| CFDI HTML list | `GET /admin/cfdis` | Existing JSON list exists at `/api/admin/cfdis/data`; detail/query/import APIs are still missing. |
| PDF editor | `GET /pdf` | Human editor only; CLI should use `POST /pdf/preview`. |
| Time page | `GET /tiempo` | Human page only; CLI should use `GET /api/tiempo`. |

## CLI Expansion Order

1. Add shared HTTP helpers first: JSON `GET`, JSON `POST`, form `POST` only where unavoidable, tenant-aware base URL, transparent re-login retry, response status mapping, and destructive confirmation enforcement.
2. Add read-only commands for existing JSON APIs: `time timeline`, `cfdi list`, `transactions list`, project concept/status reads, resource usage reads, PDF preview.
3. Expand `spcli manifest` with schema version, arguments, permissions, output schemas, destructive flags, and examples for every implemented command.
4. Add harness coverage for read-only commands, JSON output, structured errors, transparent re-login, tenant isolation, and forbidden cases.
5. Add JSON backend endpoints for finance master data: accounts, categories, contacts. These unlock most finance create/update flows.
6. Add JSON backend endpoints for transactions, planned entries, recurring plans, forecasts, and service orders, documenting finance side effects.
7. Add JSON backend endpoints for projects, resources, resource logs, users, companies, and SAT configs.
8. Add SAT/CFDI job commands after documenting secret handling, in-memory job limitations, and payment/planned-entry side effects.
9. Add higher-risk mutations after their APIs, side-effect responses, destructive confirmations, and harness tests exist.

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

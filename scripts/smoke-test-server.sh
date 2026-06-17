#!/usr/bin/env bash
#
# smoke-test-server.sh — end-to-end smoke test of the live server via spcli.
#
# Drives the real `spcli` client against a running deployment, exercising every
# command group (auth, finance, operations, resources, SAT/CFDI, admin users,
# time, PDF). It creates a dependency-ordered chain of test data, runs each
# create/get/update/list/action/delete, prints a coloured pass/fail report, and
# cleans up after itself so the tenant returns to its baseline.
#
# WARNING: this is DESTRUCTIVE on the target tenant — it creates and deletes
# records and sweeps all planned entries and transactions in the active company
# at the end. Run it ONLY against a dedicated test tenant.
#
# Usage:
#   SPCLI_TOTP_SECRET=YOUR_BASE32_SECRET ./scripts/smoke-test-server.sh
#
# Configuration (environment variables, with defaults):
#   SPCLI_BASE_URL   login/app host           (default https://app.alfredorivera.dev)
#   SPCLI_EMAIL      test user email          (default test@email.com)
#   SPCLI_TENANT     tenant slug              (default test)
#   SPCLI_TOTP_SECRET  base32 TOTP secret     (REQUIRED — never hard-coded here)
#   SPCLI_BIN        path to a prebuilt spcli (default: build debug binary)
#   SPCLI_RUN_SAT_UPLOAD=1  also test `sat configs upload` (writes cert files
#                           on the server; off by default)
#
# Exit code: 0 if every check passed, 1 otherwise.

set -uo pipefail

BASE_URL="${SPCLI_BASE_URL:-https://app.alfredorivera.dev}"
EMAIL="${SPCLI_EMAIL:-test@email.com}"
TENANT="${SPCLI_TENANT:-test}"
TOTP="${SPCLI_TOTP_SECRET:-}"
RUN_SAT_UPLOAD="${SPCLI_RUN_SAT_UPLOAD:-0}"
# Payments (pay / bulk-pay) run by default so the smoke really covers them. They
# create financial records that pin the account, but cleanup deactivates the
# account anyway, so the run stays green. Set SPCLI_RUN_PAYMENTS=0 to skip them
# (slightly less residue in the tenant).
RUN_PAYMENTS="${SPCLI_RUN_PAYMENTS:-1}"
# Structured reports (set to empty to disable). JSON is machine-readable; the
# HTML is a self-contained, colour-coded viewer grouped by category/subcategory.
REPORT_JSON="${SPCLI_REPORT_JSON:-smoke-report.json}"
REPORT_HTML="${SPCLI_REPORT_HTML:-smoke-report.html}"

# ---- colours (only when stdout is a TTY) -----------------------------------
if [ -t 1 ]; then
  BOLD=$'\033[1m'; DIM=$'\033[2m'; RESET=$'\033[0m'
  GREEN=$'\033[32m'; RED=$'\033[31m'; YELLOW=$'\033[33m'; BLUE=$'\033[34m'
else
  BOLD=''; DIM=''; RESET=''; GREEN=''; RED=''; YELLOW=''; BLUE=''
fi

PASS=0; FAIL=0
FAILED_NAMES=()
LAST_ID=""

# ---- locate the project root and spcli binary ------------------------------
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ -z "$TOTP" ]; then
  printf "${RED}error:${RESET} set SPCLI_TOTP_SECRET to the test user's base32 TOTP secret.\n" >&2
  exit 2
fi

SPCLI_BIN="${SPCLI_BIN:-}"
if [ -z "$SPCLI_BIN" ]; then
  printf "${DIM}building spcli (debug)…${RESET}\n"
  cargo build -q --bin spcli || { printf "${RED}error:${RESET} cargo build failed\n" >&2; exit 2; }
  SPCLI_BIN="$ROOT/target/debug/spcli"
fi

# ---- isolated credential store + cleanup -----------------------------------
CFG="$(mktemp -d)"
spc() { HOME="$CFG" XDG_CONFIG_HOME="$CFG" "$SPCLI_BIN" --json "$@"; }

# Structured result log: one US-separated record per check (category, status,
# name, detail). Assembled into JSON/HTML at the end.
REPORT_TMP="$CFG/records.tsv"
: > "$REPORT_TMP"
CURRENT_SECTION="General"
record() {  # status  name  detail
  local det
  det="$(printf '%s' "${3:-}" | tr '\n\r\t' '   ')"
  printf '%s\x1f%s\x1f%s\x1f%s\n' "$CURRENT_SECTION" "$1" "$2" "$det" >> "$REPORT_TMP"
}

# best-effort teardown if the script exits early
cleanup_trap() {
  for spec in \
    "admin users delete:$USER_ID" \
    "sat configs delete:$SAT_ID" \
    "finance transactions delete:$TX_ID" \
    "finance planned-entries delete:$PE_ID" \
    "finance planned-entries delete:$PE2_ID" \
    "finance recurring-plans delete:$PLAN_ID" \
    "resources usages delete:$USAGE_ID" \
    "resources logs delete:$LOG_ID" \
    "resources delete:$RES_ID" \
    "orders delete:$ORDER_ID" \
    "projects concepts delete:$CONCEPT_ID" \
    "projects statuses delete:$STATUS_ID" \
    "projects delete:$PROJECT_ID" \
    "finance forecasts delete:$FC_ID" \
    "finance contacts delete:$CONTACT_ID" \
    "finance accounts delete:$ACC_ID" \
    "finance categories delete:$CAT_ID" ; do
    local cmd="${spec%%:*}"; local id="${spec##*:}"
    [ -n "$id" ] && spc $cmd "$id" --yes >/dev/null 2>&1
  done
  rm -rf "$CFG"
}
trap cleanup_trap EXIT

# ---- reporting helpers -----------------------------------------------------
section() { CURRENT_SECTION="$1"; printf "\n${BOLD}${BLUE}══ %s ══${RESET}\n" "$1"; }
pass() { PASS=$((PASS + 1)); record pass "$1" ""; printf "  ${GREEN}✓${RESET} %s\n" "$1"; }
fail() {
  FAIL=$((FAIL + 1)); FAILED_NAMES+=("$1")
  record fail "$1" "${2:-}"
  printf "  ${RED}✗${RESET} %s\n" "$1"
  [ -n "${2:-}" ] && printf "      ${DIM}%s${RESET}\n" "$(printf '%s' "$2" | tr '\n' ' ' | cut -c1-280)"
}
skip() { record skip "$1" ""; printf "  ${YELLOW}∅${RESET} %s\n" "$1"; }

jid() {
  python3 -c 'import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get("id", "") if isinstance(d, dict) else "")
except Exception:
    print("")'
}

# ok NAME ARGS… — run a command, expect exit 0
ok() {
  local name="$1"; shift
  local out rc
  out="$(spc "$@" 2>&1)"; rc=$?
  if [ $rc -eq 0 ]; then pass "$name"; else fail "$name" "$out"; fi
}

# cap NAME ARGS… — run a create command, expect exit 0 and a JSON id (sets LAST_ID)
cap() {
  local name="$1"; shift
  local out rc
  out="$(spc "$@" 2>&1)"; rc=$?
  LAST_ID=""
  if [ $rc -eq 0 ]; then
    LAST_ID="$(printf '%s' "$out" | jid)"
    if [ -n "$LAST_ID" ]; then pass "$name → $LAST_ID"; else fail "$name" "no id in: $out"; fi
  else
    fail "$name" "$out"
  fi
}

# del NAME CMD… ID — delete by id (skips if id is empty)
del() {
  local name="$1"; shift
  local id="${!#}"            # last positional arg is the id
  if [ -z "$id" ]; then skip "$name (nothing created)"; return; fi
  ok "$name" "$@" --yes
}

# Extract ids from a JSON list, handling both flat `id` strings and raw BSON
# `_id: {$oid: ...}` (resource-usage endpoints serialize the raw model).
ids_of() {
  python3 -c 'import sys, json
def gid(x):
    if not isinstance(x, dict): return ""
    if isinstance(x.get("id"), str): return x["id"]
    i = x.get("_id")
    if isinstance(i, dict) and isinstance(i.get("$oid"), str): return i["$oid"]
    return ""
try:
    d = json.load(sys.stdin)
    print("\n".join(filter(None, (gid(x) for x in d))) if isinstance(d, list) else "")
except Exception:
    pass'
}

# Loop a list+delete until the collection is empty (handles pagination and
# untracked records such as grid-created usages or generated planned entries).
sweep_delete() {  # label  "list cmd"  "delete cmd"
  local label="$1" listcmd="$2" delbase="$3" total=0 round
  for round in 1 2 3 4 5 6 7 8 9 10; do
    local ids n=0
    ids="$(spc $listcmd 2>/dev/null | ids_of)"
    [ -z "$ids" ] && break
    while IFS= read -r id; do
      [ -z "$id" ] && continue
      spc $delbase "$id" --yes >/dev/null 2>&1 && { n=$((n + 1)); total=$((total + 1)); }
    done <<EOF
$ids
EOF
    [ "$n" -eq 0 ] && break
  done
  printf "  ${DIM}swept %s: %s removed${RESET}\n" "$label" "$total"
}

# IDs captured along the way
CAT_ID=""; ACC_ID=""; CONTACT_ID=""; FC_ID=""
STATUS_ID=""; PROJECT_ID=""; CONCEPT_ID=""; ORDER_ID=""
RES_ID=""; LOG_ID=""; USAGE_ID=""
PLAN_ID=""; PE_ID=""; PE2_ID=""; TX_ID=""
SAT_ID=""; USER_ID=""; COMPANY_ID=""; ACC_DEL=""

printf "${BOLD}spcli smoke test${RESET}  →  ${BASE_URL}  (tenant: ${TENANT}, user: ${EMAIL})\n"

# ============================================================================
section "Authentication & context"
# ============================================================================
ok  "login"               login --base-url "$BASE_URL" --email "$EMAIL" --totp-secret "$TOTP"
ok  "status"              status
ok  "account get"         account get
ok  "company list"        company list
ok  "company use $TENANT" company use "$TENANT"
ok  "manifest"            manifest
COMPANY_ID="$(spc company list 2>/dev/null | python3 -c 'import sys, json
d = json.load(sys.stdin)
print(next((c.get("id","") for c in d if c.get("slug")=="'"$TENANT"'"), ""))')"

# ============================================================================
section "Finance — master data"
# ============================================================================
cap "categories create" finance categories create --name "smoke-cat" --flow-type expense; CAT_ID="$LAST_ID"
ok  "categories list"    finance categories list
[ -n "$CAT_ID" ] && ok "categories get"    finance categories get "$CAT_ID"
[ -n "$CAT_ID" ] && ok "categories update" finance categories update "$CAT_ID" --name "smoke-cat-2" --flow-type expense

cap "accounts create" finance accounts create --name "smoke-acc" --account-type bank --currency MXN; ACC_ID="$LAST_ID"
ok  "accounts list"   finance accounts list
[ -n "$ACC_ID" ] && ok "accounts get"    finance accounts get "$ACC_ID"
[ -n "$ACC_ID" ] && ok "accounts update" finance accounts update "$ACC_ID" --name "smoke-acc-2" --account-type bank --currency MXN

cap "contacts create" finance contacts create --name "smoke-contact" --contact-type customer --rfc XAXX010101000; CONTACT_ID="$LAST_ID"
ok  "contacts list"   finance contacts list
[ -n "$CONTACT_ID" ] && ok "contacts get"    finance contacts get "$CONTACT_ID"
[ -n "$CONTACT_ID" ] && ok "contacts update" finance contacts update "$CONTACT_ID" --name "smoke-contact-2" --contact-type customer

cap "forecasts create" finance forecasts create --generated-at 2026-01-01 --start-date 2026-01-01 --end-date 2026-12-31 --currency MXN --projected-income-total 1000 --projected-expense-total 500 --projected-net 500; FC_ID="$LAST_ID"
ok  "forecasts list"   finance forecasts list
[ -n "$FC_ID" ] && ok "forecasts get"    finance forecasts get "$FC_ID"
[ -n "$FC_ID" ] && ok "forecasts update" finance forecasts update "$FC_ID" --generated-at 2026-01-01 --start-date 2026-01-01 --end-date 2026-12-31 --currency MXN --projected-income-total 1100 --projected-expense-total 500 --projected-net 600

# ============================================================================
section "Operations — projects, statuses, concepts, orders"
# ============================================================================
cap "projects statuses create" projects statuses create --name "smoke-status" --position 10 --color sky --initial; STATUS_ID="$LAST_ID"
ok  "projects statuses list"    projects statuses list
[ -n "$STATUS_ID" ] && ok "projects statuses update" projects statuses update "$STATUS_ID" --name "smoke-status-2" --position 11

cap "projects create" projects create --title "smoke-project" --category-id "$CAT_ID" --priority high --total-budget 50000 --scheduled-at 2026-07-01T00:00:00Z; PROJECT_ID="$LAST_ID"
ok  "projects list"   projects list
[ -n "$PROJECT_ID" ] && ok "projects get"            projects get "$PROJECT_ID"
[ -n "$PROJECT_ID" ] && ok "projects update"         projects update "$PROJECT_ID" --title "smoke-project-2" --category-id "$CAT_ID" --priority urgent --total-budget 55000
[ -n "$PROJECT_ID" ] && ok "projects status-summary" projects status-summary --project-id "$PROJECT_ID"

if [ -n "$PROJECT_ID" ] && [ -n "$STATUS_ID" ]; then
  cap "projects concepts create" projects concepts create --project-id "$PROJECT_ID" --status-id "$STATUS_ID" --name "smoke-concept" --quantity 1 --unit job --position 1; CONCEPT_ID="$LAST_ID"
  ok  "projects concepts list"   projects concepts list --project-id "$PROJECT_ID"
  [ -n "$CONCEPT_ID" ] && ok "projects concepts update"  projects concepts update "$CONCEPT_ID" --name "smoke-concept-2" --quantity 2 --status-id "$STATUS_ID" --position 1
fi

cap "orders create" orders create --title "smoke-order" --category-id "$CAT_ID" --account-id "$ACC_ID" --status confirmed --amount 2500 --scheduled-at 2026-07-01T08:00:00Z --item "Labor:4:250"; ORDER_ID="$LAST_ID"
ok  "orders list"   orders list
[ -n "$ORDER_ID" ] && ok "orders get"      orders get "$ORDER_ID"
[ -n "$ORDER_ID" ] && ok "orders update"   orders update "$ORDER_ID" --title "smoke-order-2" --category-id "$CAT_ID" --account-id "$ACC_ID" --status in_progress --amount 2750 --item "Labor:5:250"
[ -n "$ORDER_ID" ] && ok "orders complete" orders complete "$ORDER_ID"

# ============================================================================
section "Resources — resources, logs, usages, grid"
# ============================================================================
cap "resources create" resources create --name "smoke-resource" --resource-type machinery --hourly-cost 350 --currency MXN --allowed-status-id "$STATUS_ID"; RES_ID="$LAST_ID"
ok  "resources list"   resources list
[ -n "$RES_ID" ] && ok "resources get"    resources get "$RES_ID"
[ -n "$RES_ID" ] && ok "resources update" resources update "$RES_ID" --name "smoke-resource-2" --resource-type machinery --hourly-cost 375 --currency MXN --allowed-status-id "$STATUS_ID"

if [ -n "$RES_ID" ]; then
  cap "resources logs create" resources logs create --resource-id "$RES_ID" --started-at 2026-06-01T10:00:00Z --operator-name "smoke"; LOG_ID="$LAST_ID"
  ok  "resources logs list"   resources logs list
  [ -n "$LOG_ID" ] && ok "resources logs get"    resources logs get "$LOG_ID"
  [ -n "$LOG_ID" ] && ok "resources logs update" resources logs update "$LOG_ID" --resource-id "$RES_ID" --started-at 2026-06-01T10:00:00Z --ended-at 2026-06-01T11:00:00Z
  [ -n "$LOG_ID" ] && ok "resources logs end"    resources logs end "$LOG_ID" --ended-at 2026-06-01T12:00:00Z

  cap "resources usages create" resources usages create --resource-id "$RES_ID" --started-at 2026-06-01T10:00:00Z --ended-at 2026-06-01T12:00:00Z --operator-name "smoke"; USAGE_ID="$LAST_ID"
  ok  "resources usages list"   resources usages list
  [ -n "$USAGE_ID" ] && ok "resources usages get"    resources usages get "$USAGE_ID"
  [ -n "$USAGE_ID" ] && ok "resources usages update" resources usages update "$USAGE_ID" --started-at 2026-06-01T10:00:00Z --ended-at 2026-06-01T12:00:00Z --hourly-cost-snapshot 250
  if [ -n "$USAGE_ID" ] && [ -n "$CONCEPT_ID" ]; then
    ok "resources usages allocations replace" resources usages allocations replace "$USAGE_ID" --concept-id "$CONCEPT_ID"
    ok "resources usages allocations list"    resources usages allocations list "$USAGE_ID"
  fi
fi

# grid (NEW): saves an hourly cell; valid only when concept status matches the resource
if [ -n "$CONCEPT_ID" ] && [ -n "$RES_ID" ]; then
  ok "resources usages grid" resources usages grid --date 2026-06-01 --status-id all --cell "$CONCEPT_ID:8:$RES_ID"
fi

# ============================================================================
section "Finance — recurring plans, planned entries, payments, transactions"
# ============================================================================
if [ -n "$CAT_ID" ] && [ -n "$ACC_ID" ]; then
  cap "recurring-plans create" finance recurring-plans create --name "smoke-plan" --flow-type expense --category-id "$CAT_ID" --account-expected-id "$ACC_ID" --amount-estimated 1000 --frequency monthly --day-of-month 1 --start-date 2026-07-01T00:00:00Z; PLAN_ID="$LAST_ID"
  ok  "recurring-plans list"   finance recurring-plans list
  [ -n "$PLAN_ID" ] && ok "recurring-plans get"      finance recurring-plans get "$PLAN_ID"
  [ -n "$PLAN_ID" ] && ok "recurring-plans update"   finance recurring-plans update "$PLAN_ID" --name "smoke-plan-2" --flow-type expense --category-id "$CAT_ID" --account-expected-id "$ACC_ID" --amount-estimated 1100 --frequency monthly --day-of-month 1 --start-date 2026-07-01T00:00:00Z
  [ -n "$PLAN_ID" ] && ok "recurring-plans generate" finance recurring-plans generate "$PLAN_ID"

  cap "planned-entries create" finance planned-entries create --name "smoke-pe" --flow-type expense --category-id "$CAT_ID" --account-expected-id "$ACC_ID" --amount-estimated 500 --due-date 2026-07-01T00:00:00Z; PE_ID="$LAST_ID"
  ok  "planned-entries list"   finance planned-entries list
  [ -n "$PE_ID" ] && ok "planned-entries get"    finance planned-entries get "$PE_ID"
  [ -n "$PE_ID" ] && ok "planned-entries update" finance planned-entries update "$PE_ID" --name "smoke-pe-2" --flow-type expense --category-id "$CAT_ID" --account-expected-id "$ACC_ID" --amount-estimated 550 --due-date 2026-07-02T00:00:00Z

  if [ "$RUN_PAYMENTS" = "1" ]; then
    [ -n "$PE_ID" ] && ok "planned-entries pay" finance planned-entries pay "$PE_ID" --paid-at 2026-07-03T00:00:00Z --amount 550 --account-id "$ACC_ID"
    cap "planned-entries (for bulk-pay)" finance planned-entries create --name "smoke-pe-bulk" --flow-type expense --category-id "$CAT_ID" --account-expected-id "$ACC_ID" --amount-estimated 300 --due-date 2026-07-01T00:00:00Z; PE2_ID="$LAST_ID"
    [ -n "$PE2_ID" ] && ok "planned-entries bulk-pay" finance planned-entries bulk-pay --entry-id "$PE2_ID" --paid-at 2026-07-03T00:00:00Z --account-id "$ACC_ID"
  else
    skip "planned-entries pay / bulk-pay (disabled via SPCLI_RUN_PAYMENTS=0)"
  fi

  cap "transactions create" finance transactions create --date 2026-07-01T12:00:00Z --description "smoke-tx" --transaction-type expense --category-id "$CAT_ID" --account-from-id "$ACC_ID" --amount 500; TX_ID="$LAST_ID"
  ok  "transactions list"   finance transactions list
  [ -n "$TX_ID" ] && ok "transactions get"    finance transactions get "$TX_ID"
  [ -n "$TX_ID" ] && ok "transactions update" finance transactions update "$TX_ID" --date 2026-07-01T12:00:00Z --description "smoke-tx-2" --transaction-type expense --category-id "$CAT_ID" --account-from-id "$ACC_ID" --amount 550
fi

# ============================================================================
section "SAT & CFDI"
# ============================================================================
cap "sat configs create" sat configs create --rfc XAXX010101000 --cer-path /tmp/smoke.cer --key-path /tmp/smoke.key --key-password-env SPCLI_SMOKE_SAT_PW; SAT_ID="$LAST_ID"
ok  "sat configs list" sat configs list
[ -n "$SAT_ID" ] && ok "sat configs get"    sat configs get "$SAT_ID"
[ -n "$SAT_ID" ] && ok "sat configs update" sat configs update "$SAT_ID" --rfc XAXX010101000 --cer-path /tmp/smoke.cer --key-path /tmp/smoke.key --key-password-env SPCLI_SMOKE_SAT_PW --label "smoke FIEL"

if [ "$RUN_SAT_UPLOAD" = "1" ]; then
  printf '%s' "dummy-cert" > "$CFG/smoke.cer"; printf '%s' "dummy-key" > "$CFG/smoke.key"
  ok "sat configs upload" sat configs upload --rfc XAXX010101000 --cer-file "$CFG/smoke.cer" --key-file "$CFG/smoke.key" --key-password-env SPCLI_SMOKE_SAT_PW --label "smoke upload"
else
  skip "sat configs upload (set SPCLI_RUN_SAT_UPLOAD=1 to test; writes cert files on the server)"
fi

ok "cfdi list"      cfdi list
ok "cfdi jobs list" cfdi jobs list

# ============================================================================
section "Admin — companies, users"
# ============================================================================
ok "admin companies list" admin companies list
[ -n "$COMPANY_ID" ] && ok "admin companies get" admin companies get "$COMPANY_ID"

cap "admin users create" admin users create --email smoke-user@example.com --company-id "$COMPANY_ID" --role staff --permission view_projects; USER_ID="$LAST_ID"
ok  "admin users list"   admin users list
[ -n "$USER_ID" ] && ok "admin users get"    admin users get "$USER_ID"
[ -n "$USER_ID" ] && ok "admin users update" admin users update "$USER_ID" --email smoke-user@example.com --company-id "$COMPANY_ID" --role admin

# ============================================================================
section "Time & PDF"
# ============================================================================
ok "time timeline" time timeline --mode month --from 2026-01-01 --to 2026-12-31
ok "pdf preview"   pdf preview --source "= Smoke test"

# ============================================================================
section "Cleanup (delete commands + sweep)"
# ============================================================================
del "admin users delete"        admin users delete "$USER_ID"
del "sat configs delete"        sat configs delete "$SAT_ID"
del "transactions delete"       finance transactions delete "$TX_ID"
# Sweep accumulated/untracked records before deleting the things they reference:
# generated planned entries, any leftover transactions, and the grid-created
# resource usage (whose allocation would otherwise pin the project concept).
sweep_delete "transactions"     "finance transactions list"    "finance transactions delete"
sweep_delete "planned entries"  "finance planned-entries list" "finance planned-entries delete"
del "recurring-plans delete"    finance recurring-plans delete "$PLAN_ID"
del "resources usages delete"   resources usages delete "$USAGE_ID"
sweep_delete "resource usages"  "resources usages list"        "resources usages delete"
del "resources logs delete"     resources logs delete "$LOG_ID"
del "resources delete"          resources delete "$RES_ID"
del "orders delete"             orders delete "$ORDER_ID"
del "projects concepts delete"  projects concepts delete "$CONCEPT_ID"
del "projects statuses delete"  projects statuses delete "$STATUS_ID"
del "projects delete"           projects delete "$PROJECT_ID"
del "forecasts delete"          finance forecasts delete "$FC_ID"
del "contacts delete"           finance contacts delete "$CONTACT_ID"
# The account's in-tenant references (transactions, plans, planned entries) were
# all swept above, so the (now company-scoped) integrity check lets it be hard-
# deleted — the smoke leaves no inactive-account residue behind.
del "accounts delete"           finance accounts delete "$ACC_ID"
del "categories delete"         finance categories delete "$CAT_ID"

# clear captured ids so the EXIT trap does not re-attempt deletes
USER_ID=""; SAT_ID=""; PLAN_ID=""; USAGE_ID=""; LOG_ID=""; RES_ID=""
ORDER_ID=""; CONCEPT_ID=""; STATUS_ID=""; PROJECT_ID=""; FC_ID=""
CONTACT_ID=""; ACC_ID=""; CAT_ID=""

# Assemble the structured JSON report and a self-contained colour-coded HTML
# viewer (categories → subcategories → checks), like a Cypress/mochawesome report.
build_report() {
  [ -z "$REPORT_JSON$REPORT_HTML" ] && return 0
  R_TMP="$REPORT_TMP" R_JSON="$REPORT_JSON" R_HTML="$REPORT_HTML" \
  R_URL="$BASE_URL" R_TENANT="$TENANT" R_EMAIL="$EMAIL" python3 <<'PY'
import os, json, html, re, datetime

tmp = os.environ["R_TMP"]
out_json = os.environ.get("R_JSON", "")
out_html = os.environ.get("R_HTML", "")
url = os.environ.get("R_URL", ""); tenant = os.environ.get("R_TENANT", ""); email = os.environ.get("R_EMAIL", "")

ACTIONS = {"create", "list", "get", "update", "delete", "advance", "complete",
           "generate", "pay", "bulk-pay", "end", "replace", "grid",
           "status-summary", "deactivate", "use"}

def subcat(name):
    clean = re.sub(r"\s*\(.*?\)", "", name)        # drop "(for bulk-pay)" etc.
    clean = re.sub(r"\s*→.*$", "", clean).strip()  # drop "→ <created id>"
    toks = clean.split()
    for i, t in enumerate(toks):
        if t in ACTIONS:
            return " ".join(toks[:i]) or clean
    return clean

cats, order = {}, []
summary = {"passed": 0, "failed": 0, "skipped": 0}
with open(tmp, encoding="utf-8") as fh:
    for line in fh:
        line = line.rstrip("\n")
        if not line:
            continue
        p = line.split("\x1f")
        p += [""] * (4 - len(p))
        cat, status, name, detail = p[0], p[1], p[2], p[3]
        key = "passed" if status == "pass" else "failed" if status == "fail" else "skipped"
        summary[key] += 1
        if cat not in cats:
            cats[cat] = {}; order.append(cat)
        cats[cat].setdefault(subcat(name), []).append(
            {"name": name, "status": status, "detail": detail})

total = sum(summary.values())
report = {
    "generated_at": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
    "target": {"base_url": url, "tenant": tenant, "email": email},
    "summary": {**summary, "total": total, "ok": summary["failed"] == 0},
    "categories": [
        {"name": c, "subcategories": [
            {"name": sc, "checks": cats[c][sc]} for sc in cats[c]]}
        for c in order],
}
if out_json:
    with open(out_json, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, ensure_ascii=False)

if out_html:
    ICON = {"pass": "✓", "fail": "✗", "skip": "∅"}
    badge = lambda p, f, s: f'<span class="b ok">{p}</span><span class="b bad">{f}</span><span class="b skip">{s}</span>'
    cat_html = []
    for c in order:
        cp = cf = cs = 0; subs = []
        for sc in cats[c]:
            checks = cats[c][sc]
            sp = sum(x["status"] == "pass" for x in checks)
            sf = sum(x["status"] == "fail" for x in checks)
            ss = sum(x["status"] == "skip" for x in checks)
            cp += sp; cf += sf; cs += ss
            rows = []
            for x in checks:
                det = f'<div class="det">{html.escape(x["detail"])}</div>' if x["detail"] else ""
                rows.append(f'<li class="r {x["status"]}"><span class="i">{ICON[x["status"]]}</span><span class="n">{html.escape(x["name"])}</span>{det}</li>')
            subs.append(f'<details class="sub"{" open" if sf else ""}><summary>{html.escape(sc)} {badge(sp, sf, ss)}</summary><ul>{"".join(rows)}</ul></details>')
        cat_html.append(f'<details class="cat"{" open" if cf else ""}><summary>{html.escape(c)} {badge(cp, cf, cs)}</summary>{"".join(subs)}</details>')
    scls = "ok" if summary["failed"] == 0 else "bad"
    stxt = "ALL GREEN" if summary["failed"] == 0 else f'{summary["failed"]} FAILED'
    doc = f"""<!doctype html><html><head><meta charset="utf-8"><title>spcli smoke report</title><style>
:root{{--bg:#0f1115;--card:#171a21;--fg:#e6e6e6;--mut:#8b93a1;--ok:#3fb950;--bad:#f85149;--skip:#9aa0a6;--line:#2a2f3a}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 -apple-system,Segoe UI,Roboto,sans-serif;padding:24px}}
.h{{display:flex;align-items:center;gap:14px;flex-wrap:wrap;margin-bottom:8px}}.title{{font-size:20px;font-weight:700}}
.meta{{color:var(--mut);font-size:12px}}.status{{font-weight:700;padding:4px 12px;border-radius:999px}}
.status.ok{{background:rgba(63,185,80,.15);color:var(--ok)}}.status.bad{{background:rgba(248,81,73,.15);color:var(--bad)}}
.cards{{display:flex;gap:10px;margin:14px 0 20px}}.card{{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:10px 16px;min-width:88px}}
.card .v{{font-size:22px;font-weight:700}}.card .k{{color:var(--mut);font-size:11px;text-transform:uppercase;letter-spacing:.05em}}
.card.ok .v{{color:var(--ok)}}.card.bad .v{{color:var(--bad)}}.card.skip .v{{color:var(--skip)}}
details.cat{{background:var(--card);border:1px solid var(--line);border-radius:10px;margin:10px 0;overflow:hidden}}
details.cat>summary{{padding:12px 16px;font-weight:700;cursor:pointer;font-size:15px}}
details.sub{{margin:0 12px 10px}}details.sub>summary{{padding:8px 10px;cursor:pointer;border-radius:6px}}
details.sub>summary:hover{{background:rgba(255,255,255,.03)}}summary{{list-style:none}}summary::-webkit-details-marker{{display:none}}
ul{{list-style:none;margin:0 0 6px;padding:0 10px 0 14px}}
li.r{{display:grid;grid-template-columns:18px 1fr;gap:8px;padding:4px 6px;border-top:1px solid var(--line)}}
li.r .i{{font-weight:700}}li.r.pass .i{{color:var(--ok)}}li.r.fail .i{{color:var(--bad)}}li.r.skip .i{{color:var(--skip)}}li.r.skip .n{{color:var(--mut)}}
.det{{grid-column:2;color:var(--bad);font-family:ui-monospace,Menlo,monospace;font-size:12px;white-space:pre-wrap;background:rgba(248,81,73,.08);padding:6px 8px;border-radius:6px;margin-top:4px}}
.b{{display:inline-block;min-width:18px;text-align:center;font-size:11px;font-weight:700;padding:1px 6px;border-radius:6px;margin-left:4px}}
.b.ok{{background:rgba(63,185,80,.15);color:var(--ok)}}.b.bad{{background:rgba(248,81,73,.15);color:var(--bad)}}.b.skip{{background:rgba(154,160,166,.15);color:var(--skip)}}
</style></head><body>
<div class="h"><span class="title">spcli smoke report</span><span class="status {scls}">{stxt}</span></div>
<div class="meta">{html.escape(url)} &middot; tenant {html.escape(tenant)} &middot; {report["generated_at"]}</div>
<div class="cards"><div class="card"><div class="v">{total}</div><div class="k">total</div></div>
<div class="card ok"><div class="v">{summary["passed"]}</div><div class="k">passed</div></div>
<div class="card bad"><div class="v">{summary["failed"]}</div><div class="k">failed</div></div>
<div class="card skip"><div class="v">{summary["skipped"]}</div><div class="k">skipped</div></div></div>
{"".join(cat_html)}
</body></html>"""
    with open(out_html, "w", encoding="utf-8") as f:
        f.write(doc)
PY
}

# ============================================================================
section "Summary"
# ============================================================================
TOTAL=$((PASS + FAIL))
build_report
[ -n "$REPORT_JSON" ] && printf "${DIM}report: %s${RESET}\n" "$REPORT_JSON"
[ -n "$REPORT_HTML" ] && printf "${DIM}report: %s${RESET}\n" "$REPORT_HTML"
if [ "$FAIL" -eq 0 ]; then
  printf "${GREEN}${BOLD}ALL GREEN${RESET} — %s/%s checks passed against %s\n" "$PASS" "$TOTAL" "$BASE_URL"
  exit 0
else
  printf "${RED}${BOLD}%s/%s checks FAILED${RESET} against %s\n" "$FAIL" "$TOTAL" "$BASE_URL"
  for n in "${FAILED_NAMES[@]}"; do printf "  ${RED}✗${RESET} %s\n" "$n"; done
  exit 1
fi

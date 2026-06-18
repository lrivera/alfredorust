// test_dashboard.rs
// Landing page for the test-only tooling surface (Swagger + test reports).
// Mounted behind require_session + require_test_tenant, so it is only reachable
// when logged in on the test tenant.

use std::path::Path;

use axum::response::Html;

fn reports_dir() -> String {
    std::env::var("TEST_REPORTS_DIR").unwrap_or_else(|_| "test-reports".to_string())
}

pub async fn test_dashboard() -> Html<String> {
    let dir = reports_dir();
    let base = Path::new(&dir);

    // (href, label) — always offer Swagger; only link reports that exist.
    let mut items: Vec<(&str, &str)> = vec![("/docs", "Swagger UI — interactive API docs")];
    if base.join("smoke-report.html").is_file() {
        items.push((
            "/test/reports/smoke-report.html",
            "spcli smoke test — latest run",
        ));
    }
    if base.join("playwright/index.html").is_file() {
        items.push((
            "/test/reports/playwright/index.html",
            "Frontend Playwright report — latest run",
        ));
    }

    let rows = items
        .iter()
        .map(|(href, label)| {
            format!(r#"<li><a href="{href}">{label}</a></li>"#)
        })
        .collect::<String>();

    let missing = if items.len() == 1 {
        r#"<p class="note">No test reports published yet. They appear here once
           the smoke test / Playwright run uploads them to the server's reports
           directory.</p>"#
    } else {
        ""
    };

    Html(format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Test tooling</title>
<style>
body{{font:15px/1.6 -apple-system,Segoe UI,Roboto,sans-serif;max-width:640px;margin:48px auto;padding:0 20px;color:#e6e6e6;background:#0f1115}}
h1{{font-size:22px}} .note{{color:#8b93a1;font-size:13px}}
ul{{list-style:none;padding:0}} li{{margin:10px 0}}
a{{display:block;padding:14px 16px;background:#171a21;border:1px solid #2a2f3a;border-radius:10px;color:#58a6ff;text-decoration:none}}
a:hover{{border-color:#3b82f6}}
.tag{{display:inline-block;font-size:11px;color:#3fb950;background:rgba(63,185,80,.12);padding:2px 8px;border-radius:999px;margin-left:8px}}
</style></head><body>
<h1>Test tooling <span class="tag">test tenant</span></h1>
<p class="note">Internal — only visible when logged in on the test tenant.</p>
<ul>{rows}</ul>
{missing}
</body></html>"#
    ))
}

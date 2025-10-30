use std::{
    fs,
    process::{Command, Stdio},
    sync::Arc,
};

use askama::Template;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};

use crate::{session::SessionUser, state::AppState};

fn render<T: Template>(tpl: T) -> Result<Html<String>, StatusCode> {
    tpl.render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Template)]
#[template(path = "pdf/editor.html")]
struct PdfEditorTemplate {
    session_email: String,
}

#[derive(Deserialize)]
pub(crate) struct PdfPreviewRequest {
    source: String,
}

#[derive(Serialize)]
pub(crate) struct PdfPreviewResponse {
    ok: bool,
    pdf_base64: Option<String>,
    error: Option<String>,
}

pub async fn pdf_editor(SessionUser(session): SessionUser) -> Result<Html<String>, StatusCode> {
    render(PdfEditorTemplate {
        session_email: session.user.email,
    })
}

pub async fn pdf_preview(
    SessionUser(_session): SessionUser,
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<PdfPreviewRequest>,
) -> impl IntoResponse {
    match compile_typst(&payload.source).await {
        Ok(bytes) => {
            let encoded = data_encoding::BASE64.encode(&bytes);
            Json(PdfPreviewResponse {
                ok: true,
                pdf_base64: Some(encoded),
                error: None,
            })
        }
        Err(err) => Json(PdfPreviewResponse {
            ok: false,
            pdf_base64: None,
            error: Some(err),
        }),
    }
}

async fn compile_typst(source: &str) -> Result<Vec<u8>, String> {
    let mut rng = rand::rng();
    let suffix: String = (&mut rng)
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    let tmp_dir = std::env::temp_dir().join(format!("typst-{}", suffix));
    fs::create_dir(&tmp_dir).map_err(|e| format!("No se pudo crear directorio temporal: {e}"))?;

    let input_path = tmp_dir.join("input.typ");
    let output_path = tmp_dir.join("output.pdf");

    let write_result = fs::write(&input_path, source);
    if let Err(err) = write_result {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(format!("No se pudo escribir archivo temporal: {err}"));
    }

    let typst_bin = std::env::var("TYPST_BIN").unwrap_or_else(|_| "typst".to_string());

    let output = Command::new(&typst_bin)
        .arg("compile")
        .arg(&input_path)
        .arg(&output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| {
            let _ = fs::remove_dir_all(&tmp_dir);
            if err.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "No se encontró el binario `{}`. Instálalo y/o define la variable TYPST_BIN",
                    typst_bin
                )
            } else {
                format!("Error ejecutando typst: {err}")
            }
        })?;

    if !output.status.success() {
        let _ = fs::remove_dir_all(&tmp_dir);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(if stderr.trim().is_empty() {
            "Fallo al ejecutar typst".to_string()
        } else {
            stderr
        });
    }

    let pdf_bytes =
        fs::read(&output_path).map_err(|err| format!("No se pudo leer el PDF generado: {err}"))?;

    let _ = fs::remove_dir_all(&tmp_dir);
    Ok(pdf_bytes)
}

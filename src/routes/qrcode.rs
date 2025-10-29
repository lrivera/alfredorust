// routes/qrcode.rs
// GET /qrcode?email=... -> returns a PNG QR code of the otpauth URL.

use axum::{
    body::Body,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use image::{ImageFormat, Luma};
use qrcode::QrCode;
use serde::Deserialize;
use std::io::Cursor;
use crate::session::SessionUser;
use crate::totp::build_totp;

#[derive(Deserialize)]
pub struct EmailQuery {
    pub email: String,
}

/// Builds and returns a PNG QR code so clients can scan and enroll.
pub async fn qrcode(session: SessionUser, Query(q): Query<EmailQuery>) -> Response {
    if let Err(resp) = session.ensure_email(&q.email) {
        return resp;
    }

    let current = session.user();

    match build_totp(&current.company_name, &current.email, &current.secret) {
        Ok(totp) => {
            let url = totp.get_url();
            if let Ok(code) = QrCode::new(url.as_bytes()) {
                let img = code.render::<Luma<u8>>().min_dimensions(200, 200).build();

                // image 0.25: write_to requires Write + Seek -> Cursor<Vec<u8>>
                let mut cursor = Cursor::new(Vec::<u8>::new());
                if image::DynamicImage::ImageLuma8(img)
                    .write_to(&mut cursor, ImageFormat::Png)
                    .is_ok()
                {
                    let png = cursor.into_inner();
                    return Response::builder()
                        .header("Content-Type", "image/png")
                        .body(Body::from(png))
                        .unwrap();
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to build qr",
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid secret",
        )
            .into_response(),
    }
}

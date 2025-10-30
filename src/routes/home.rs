// routes/home.rs
// GET / -> renders the login page using Askama templates.

use askama::Template;
use axum::{http::StatusCode, response::Html};

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate;

pub async fn home() -> Result<Html<String>, StatusCode> {
    HomeTemplate
        .render()
        .map(Html)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

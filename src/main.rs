use dotenvy::dotenv;
use std::env;
use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
    // Carga el archivo .env al inicio
    dotenv().ok();

    // Lee variables del entorno
    let app_name = env::var("APP_NAME").unwrap_or_else(|_| "MyAxumApp".into());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".into());
    println!("🚀 Starting {app_name} on port {port}");
    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    // run our app with hyper, listening globally on port 8080
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

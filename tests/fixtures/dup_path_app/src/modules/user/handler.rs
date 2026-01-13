use axum::{routing::get, Router};

pub fn router() -> Router {
    Router::new().route("/login", get(login))
}

pub async fn login() -> &'static str {
    "login handler"
}

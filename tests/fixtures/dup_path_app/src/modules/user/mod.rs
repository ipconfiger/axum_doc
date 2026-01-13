use axum::{routing::get, Router};

pub mod handler;

pub fn router() -> Router {
    // This creates the duplicate path issue
    Router::new().nest("/api/v1/user", handler::router())
}

// Auth module handlers
pub mod auth_handler;

use axum::{routing::post, Router};
use auth_handler::login;

pub fn router() -> Router {
    Router::new().route("/login", post(login))
}

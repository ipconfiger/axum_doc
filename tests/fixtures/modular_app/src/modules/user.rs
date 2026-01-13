// User module handlers
pub mod user_handler;

use axum::{routing::get, Router};
use user_handler::get_user_info;

pub fn router() -> Router {
    Router::new().route("/info", get(get_user_info))
}

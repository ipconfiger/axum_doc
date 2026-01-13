pub mod user;

use axum::Router;

pub fn router() -> Router {
    Router::new()
        .nest("/api/v1/user", user::router())
}

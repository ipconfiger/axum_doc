// Main module router that combines sub-modules
mod auth;
mod user;

pub fn router() -> Router {
    Router::new()
        .merge(auth::router())
        .nest("/api/v1/user", user::router())
}

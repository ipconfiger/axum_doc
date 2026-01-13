use axum::{routing::get, Router};

mod modules;

fn main() {
    let app = router();
    // For testing purposes
}

fn router() -> Router {
    Router::new()
        .route("/", get(root))
        .merge(modules::router())
}

async fn root() -> &'static str {
    "Welcome"
}

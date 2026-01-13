use axum::{routing::get, Router};

mod modules;

/// Root health check
async fn root() -> &'static str {
    "Modular app example"
}

fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .merge(modules::router())
}

fn main() {
    println!("Modular app fixture for axum_doc testing");
}

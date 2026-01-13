use axum::{extract::Path, routing::{get, post}, Json, Router};
use form::LoginForm;
use response::LoginResponse;
use types::User;

mod form;
mod response;
mod types;

/// User login endpoint
///
/// This endpoint handles user authentication and returns a JWT token.
/// The token can be used for subsequent authenticated requests.
async fn login(Json(form): Json<LoginForm>) -> Json<LoginResponse> {
    Json(LoginResponse {
        token: "sample_jwt_token".to_string(),
        user_id: uuid::Uuid::new_v4(),
        username: form.username,
    })
}

/// Get user by ID
///
/// Retrieves user information by their unique identifier.
/// Returns 404 if the user doesn't exist.
async fn get_user(Path(user_id): Path<String>) -> Json<User> {
    Json(User {
        id: uuid::Uuid::new_v4(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
        created_at: chrono::Utc::now(),
    })
}

/// Root health check
async fn root() -> &'static str {
    "Service is running"
}

fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/login", post(login))
        .route("/user/:id", get(get_user))
}

fn main() {
    println!("Simple app fixture for axum_doc testing");
}

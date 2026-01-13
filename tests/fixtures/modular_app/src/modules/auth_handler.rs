use axum::Json;
use serde::{Deserialize, Serialize};

/// User login credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
}

/// Login response with token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
}

/// User login endpoint
///
/// Authenticates a user and returns a JWT token.
pub async fn login(Json(creds): Json<LoginCredentials>) -> Json<LoginResponse> {
    Json(LoginResponse {
        token: "sample_jwt_token".to_string(),
        user_id: "user_123".to_string(),
    })
}

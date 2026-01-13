use axum::Json;
use serde::{Deserialize, Serialize};

/// User profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
}

/// Get user information
///
/// Returns the current user's profile information.
pub async fn get_user_info() -> Json<UserInfo> {
    Json(UserInfo {
        id: "user_123".to_string(),
        username: "testuser".to_string(),
        email: "test@example.com".to_string(),
    })
}

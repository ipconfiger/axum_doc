use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User login response with token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: Uuid,
    pub username: String,
}

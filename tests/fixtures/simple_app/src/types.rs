use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::DateTime;

/// User profile data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: DateTime<chrono::Utc>,
}

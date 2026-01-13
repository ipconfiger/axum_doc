use serde::{Deserialize, Serialize};

/// User login form
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

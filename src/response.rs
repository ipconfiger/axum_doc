// src/response.rs

use serde::Serialize;

#[derive(Serialize)]
pub struct UserProfile {
    pub id: u64,
    pub username: String,
    pub email: String,
}
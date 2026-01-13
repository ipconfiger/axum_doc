// src/response.rs

#![allow(dead_code)]

use serde::Serialize;

#[derive(Serialize)]
pub struct UserProfile {
    pub id: u64,
    pub username: String,
    pub email: String,
}
// src/form.rs

use serde::Deserialize;

#[derive(Deserialize)]
pub struct UserLogin {
    pub username: String,
    pub pass: String,
}
use axum::{
    Json,
    extract::Path as AxumPath,
};
use crate::types::User;
use crate::response::UserProfile;

pub async fn get_user_profile(AxumPath(id): AxumPath<u64>) -> Json<UserProfile> {
    Json(UserProfile {
        id,
        username: format!("user_{}", id),
        email: format!("user{}@example.com", id),
    })
}

pub async fn update_user(AxumPath(id): AxumPath<u64>, Json(user): Json<User>) -> Json<User> {
    Json(User { id, name: user.name })
} 
//! Supabase Auth REST client. Stub; M3 implements the calls.
//!
//! Endpoints: `/auth/v1/signup`, `/auth/v1/token?grant_type=password`,
//! `/auth/v1/token?grant_type=refresh_token`, `/auth/v1/logout`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub user_id: String,
}

#[allow(dead_code)]
pub struct AuthClient {
    pub base_url: String,
    pub anon_key: String,
    http: reqwest::Client,
}

impl AuthClient {
    pub fn new(base_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            anon_key: anon_key.into(),
            http: reqwest::Client::new(),
        }
    }
}

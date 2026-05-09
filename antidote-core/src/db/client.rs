//! PostgREST client over `reqwest`. Same code path on native and wasm.
//! Stub; M1 finishes the round-trip.

use crate::db::models::Game;

pub struct PostgrestClient {
    pub base_url: String,
    pub anon_key: String,
    pub access_token: Option<String>,
    http: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server returned {status}: {body}")]
    Server { status: u16, body: String },
}

impl PostgrestClient {
    pub fn new(base_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            anon_key: anon_key.into(),
            access_token: None,
            http: reqwest::Client::new(),
        }
    }

    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    pub async fn list_games(&self) -> Result<Vec<Game>, DbError> {
        let url = format!(
            "{}/rest/v1/games?select=*&order=sort_order.asc",
            self.base_url
        );
        let mut req = self
            .http
            .get(&url)
            .header("apikey", &self.anon_key)
            .header("accept", "application/json");
        if let Some(tok) = &self.access_token {
            req = req.bearer_auth(tok);
        } else {
            req = req.bearer_auth(&self.anon_key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(DbError::Server {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp.json::<Vec<Game>>().await?)
    }
}

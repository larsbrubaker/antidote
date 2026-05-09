//! PostgREST client. Same code path on native and wasm via `ehttp::fetch`.
//!
//! Each method captures the inbox sender, kicks off the request, and
//! deserializes the response into a typed [`DbInboxEvent`] variant. Errors
//! come through as `Err(String)` so the UI can show them verbatim.

use crate::db::inbox::{DbInbox, DbInboxEvent};
use crate::db::models::{Game, UserScore};

pub struct PostgrestClient {
    pub base_url: String,
    pub anon_key: String,
    pub access_token: Option<String>,
}

impl PostgrestClient {
    pub fn new(base_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            anon_key: anon_key.into(),
            access_token: None,
        }
    }

    pub fn set_access_token(&mut self, token: Option<String>) {
        self.access_token = token;
    }

    /// `GET /rest/v1/games?select=*&order=sort_order.asc`. Result lands on
    /// `inbox.rx` as `DbInboxEvent::GamesList`.
    pub fn list_games_async(&self, inbox: &DbInbox) {
        let url = format!(
            "{}/rest/v1/games?select=*&order=sort_order.asc",
            self.base_url
        );
        let req = self.get_request(url);
        let tx = inbox.tx.clone();
        ehttp::fetch(req, move |result| {
            let event = DbInboxEvent::GamesList(parse_json_array::<Game>(result));
            let _ = tx.send(event);
        });
    }

    /// `GET /rest/v1/user_scores?game_id=eq.<id>&order=high_score.desc&limit=<n>`.
    /// Result lands on `inbox.rx` as `DbInboxEvent::TopScoresList`.
    pub fn top_scores_for_game_async(&self, game_id: &str, limit: u32, inbox: &DbInbox) {
        let url = format!(
            "{}/rest/v1/user_scores?select=*&game_id=eq.{}&order=high_score.desc&limit={}",
            self.base_url, game_id, limit
        );
        let req = self.get_request(url);
        let tx = inbox.tx.clone();
        ehttp::fetch(req, move |result| {
            let event = DbInboxEvent::TopScoresList(parse_json_array::<UserScore>(result));
            let _ = tx.send(event);
        });
    }

    /// Fire-and-forget upsert of the running player's score row. We don't
    /// surface a UI confirmation right now, just log on failure.
    /// `PATCH-with-Prefer: resolution=merge-duplicates` semantics; we pass
    /// the full row on POST + `Prefer: resolution=merge-duplicates`.
    pub fn upsert_user_score_async(&self, score: &UserScore) {
        let url = format!("{}/rest/v1/user_scores", self.base_url);
        let body = serde_json::to_vec(score).unwrap_or_default();
        let mut req = ehttp::Request::post(url, body);
        self.add_auth_headers(&mut req);
        req.headers
            .insert("Content-Type".to_owned(), "application/json".to_owned());
        req.headers.insert(
            "Prefer".to_owned(),
            "resolution=merge-duplicates,return=minimal".to_owned(),
        );
        ehttp::fetch(req, |_| {});
    }

    fn get_request(&self, url: String) -> ehttp::Request {
        let mut req = ehttp::Request::get(url);
        self.add_auth_headers(&mut req);
        req.headers
            .insert("Accept".to_owned(), "application/json".to_owned());
        req
    }

    fn add_auth_headers(&self, req: &mut ehttp::Request) {
        req.headers
            .insert("apikey".to_owned(), self.anon_key.clone());
        let bearer = self.access_token.as_deref().unwrap_or(&self.anon_key);
        req.headers
            .insert("Authorization".to_owned(), format!("Bearer {bearer}"));
    }
}

fn parse_json_array<T: serde::de::DeserializeOwned>(
    result: Result<ehttp::Response, String>,
) -> Result<Vec<T>, String> {
    let resp = result.map_err(|e| format!("network: {e}"))?;
    if !resp.ok {
        let body = std::str::from_utf8(&resp.bytes).unwrap_or("");
        return Err(format!("{}: {}", resp.status, body));
    }
    serde_json::from_slice::<Vec<T>>(&resp.bytes).map_err(|e| format!("parse: {e}"))
}

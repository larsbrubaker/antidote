//! Supabase Auth REST client.
//!
//! Endpoints we hit (all under `<base_url>/auth/v1/`):
//! - `POST /signup` (body `{email, password}`)
//! - `POST /token?grant_type=password` (body `{email, password}`)
//! - `POST /token?grant_type=refresh_token` (body `{refresh_token}`)
//!
//! Every call goes through `ehttp::fetch` with a callback that posts a
//! [`DbInboxEvent`] back to the UI. See `db/inbox.rs` for the event shape
//! and the bridge rationale.

use serde::Deserialize;

use crate::db::inbox::{DbInbox, DbInboxEvent, Session};

pub struct AuthClient {
    pub base_url: String,
    pub anon_key: String,
}

impl AuthClient {
    pub fn new(base_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            anon_key: anon_key.into(),
        }
    }

    /// Fire-and-forget sign-in. The result lands on `inbox.rx` as
    /// `DbInboxEvent::SignInResult` once the response arrives.
    pub fn sign_in_async(&self, email: &str, password: &str, inbox: &DbInbox) {
        self.post_token(
            "/auth/v1/token?grant_type=password",
            email,
            password,
            inbox,
            DbInboxEvent::SignInResult,
        );
    }

    /// Fire-and-forget sign-up. Result is `DbInboxEvent::SignUpResult`.
    /// On the default Supabase config, sign-up returns the user with no
    /// session until they confirm via email; in that case `bytes` will
    /// contain a user object without `access_token`. We surface that as an
    /// error string so the UI can prompt the user to check their email.
    pub fn sign_up_async(&self, email: &str, password: &str, inbox: &DbInbox) {
        self.post_token(
            "/auth/v1/signup",
            email,
            password,
            inbox,
            DbInboxEvent::SignUpResult,
        );
    }

    fn post_token(
        &self,
        path: &str,
        email: &str,
        password: &str,
        inbox: &DbInbox,
        wrap: fn(Result<Session, String>) -> DbInboxEvent,
    ) {
        let url = format!("{}{}", self.base_url, path);
        let body = serde_json::json!({
            "email": email,
            "password": password,
        });
        let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
        let mut req = ehttp::Request::post(url, body_bytes);
        req.headers
            .insert("Content-Type".to_owned(), "application/json".to_owned());
        req.headers
            .insert("apikey".to_owned(), self.anon_key.clone());

        let tx = inbox.tx.clone();
        let email_owned = email.to_owned();
        ehttp::fetch(req, move |result| {
            let event = wrap(parse_token_response(result, email_owned));
            let _ = tx.send(event);
        });
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    /// Seconds until the access token expires. Supabase returns this
    /// alongside the tokens; we convert to absolute epoch in `Session`.
    expires_in: Option<i64>,
    /// Present on `/signup` even if `access_token` is missing (because the
    /// project requires email confirmation).
    user: Option<TokenUser>,
}

#[derive(Debug, Deserialize)]
struct TokenUser {
    id: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    /// Modern Supabase Auth puts the human-readable string here.
    msg: Option<String>,
    /// PostgREST + GoTrue both fall back to `error_description` for
    /// OAuth-style failures.
    error_description: Option<String>,
    /// Generic fall-through.
    message: Option<String>,
}

fn parse_token_response(
    result: Result<ehttp::Response, String>,
    email: String,
) -> Result<Session, String> {
    let resp = result.map_err(|e| format!("network: {e}"))?;
    if !resp.ok {
        let body = std::str::from_utf8(&resp.bytes).unwrap_or("");
        let msg = serde_json::from_str::<ErrorResponse>(body)
            .ok()
            .and_then(|e| e.msg.or(e.error_description).or(e.message))
            .unwrap_or_else(|| body.to_owned());
        return Err(format!("{}: {}", resp.status, msg));
    }
    let parsed: TokenResponse =
        serde_json::from_slice(&resp.bytes).map_err(|e| format!("parse: {e}"))?;
    let access_token = parsed
        .access_token
        .ok_or("no access_token returned (email confirmation may be required)")?;
    let refresh_token = parsed.refresh_token.unwrap_or_default();
    let expires_in = parsed.expires_in.unwrap_or(3600);
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let user_id = parsed
        .user
        .as_ref()
        .and_then(|u| u.id.clone())
        .unwrap_or_default();
    let session_email = parsed.user.and_then(|u| u.email).or(Some(email));
    Ok(Session {
        access_token,
        refresh_token,
        expires_at: now + expires_in,
        user_id,
        email: session_email,
    })
}

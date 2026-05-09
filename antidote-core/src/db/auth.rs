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

/// OAuth providers we expose buttons for. Maps to the `provider=` query
/// parameter of `/auth/v1/authorize`. Each one has to be enabled + given a
/// Client ID + Secret in the Supabase Dashboard before its button does
/// anything useful — see `db/README.md` for the per-provider setup steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    Facebook,
    Apple,
}

impl OAuthProvider {
    pub fn slug(self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Facebook => "facebook",
            OAuthProvider::Apple => "apple",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            OAuthProvider::Google => "Google",
            OAuthProvider::Facebook => "Facebook",
            OAuthProvider::Apple => "Apple",
        }
    }
}

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

    /// Build the URL the platform shell should navigate to (web) or open in
    /// the system browser (native) to start an OAuth flow. Supabase will
    /// redirect to `redirect_to` after the provider completes the round
    /// trip, with `#access_token=...&refresh_token=...&expires_in=...`
    /// appended to the URL hash.
    ///
    /// `redirect_to` must be in the project's "Allowed redirect URLs" list
    /// (Supabase Dashboard → Authentication → URL Configuration). For the
    /// production deploy that's `https://larsbrubaker.github.io/antidote/`.
    pub fn oauth_url(&self, provider: OAuthProvider, redirect_to: &str) -> String {
        format!(
            "{}/auth/v1/authorize?provider={}&redirect_to={}",
            self.base_url,
            provider.slug(),
            url_encode(redirect_to),
        )
    }

    /// Construct a [`Session`] from raw OAuth-redirect tokens. Decodes the
    /// JWT payload (no signature check — we trust the channel) to fill in
    /// `user_id` and `email`. Does NOT push to the inbox; the caller (the
    /// platform shell, after parsing the URL hash) writes directly to the
    /// model.
    pub fn session_from_oauth_tokens(
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    ) -> Session {
        let (user_id, email) = parse_jwt_payload(&access_token).unwrap_or_default();
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Session {
            access_token,
            refresh_token,
            expires_at: now + expires_in,
            user_id,
            email,
        }
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

/// Minimal URL-encoder for the OAuth `redirect_to` query parameter. We
/// only need to escape characters that break the query string — full
/// percent-encoding tables aren't worth a dependency for this single call.
fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Decode a JWT's payload (the middle base64url-encoded JSON section) and
/// pull out `sub` (user_id) and `email`. Returns `None` if the token isn't
/// a well-formed JWT or the payload doesn't parse. We do *not* verify the
/// signature — the token came directly from a Supabase OAuth round trip
/// and we're going to send it back to Supabase as a Bearer; the server
/// will reject anything tampered with.
fn parse_jwt_payload(jwt: &str) -> Option<(String, Option<String>)> {
    let mut parts = jwt.split('.');
    let _header = parts.next()?;
    let payload_b64 = parts.next()?;
    let bytes = {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload_b64.as_bytes())
            .ok()?
    };
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let sub = json.get("sub")?.as_str()?.to_owned();
    let email = json
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    Some((sub, email))
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

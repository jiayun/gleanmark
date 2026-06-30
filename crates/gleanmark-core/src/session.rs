//! Supabase auth session for cloud mode.
//!
//! Holds the user's Supabase session (email/password login), hands the gateway
//! a fresh `access_token` on demand, and transparently refreshes it via the
//! `refresh_token` when it expires. Supabase **rotates** refresh tokens on every
//! refresh, so each refresh is persisted back to `session.json` (chmod 600) —
//! otherwise the stored token would be stale after a restart.
//!
//! `cloud.toml` holds static config (`gateway_url`, `supabase_url`,
//! `supabase_anon_key`); `session.json` holds the rotating, machine-managed
//! tokens. The two are kept separate on purpose.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{Error, Result};

/// Refresh this many seconds before the access token actually expires, so a
/// request never races the expiry boundary.
const EXPIRY_SKEW_SECS: i64 = 60;

#[derive(Clone, Serialize, Deserialize)]
struct StoredSession {
    refresh_token: String,
    access_token: String,
    /// Unix seconds at which `access_token` expires.
    expires_at: i64,
    #[serde(default)]
    email: Option<String>,
}

/// Public, redacted view of the current session for the Settings UI.
#[derive(Serialize)]
pub struct SessionStatus {
    pub signed_in: bool,
    pub email: Option<String>,
}

pub struct SessionManager {
    http: reqwest::Client,
    supabase_url: String,
    anon_key: String,
    session_path: PathBuf,
    state: RwLock<Option<StoredSession>>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    expires_at: Option<i64>,
    #[serde(default)]
    user: Option<UserObject>,
}

#[derive(Deserialize)]
struct UserObject {
    email: Option<String>,
}

impl SessionManager {
    /// Build a manager, loading any persisted session from `session_path`.
    pub fn new(supabase_url: String, anon_key: String, session_path: PathBuf) -> Self {
        let state = std::fs::read_to_string(&session_path)
            .ok()
            .and_then(|s| serde_json::from_str::<StoredSession>(&s).ok());

        Self {
            http: reqwest::Client::new(),
            supabase_url: supabase_url.trim_end_matches('/').to_string(),
            anon_key,
            session_path,
            state: RwLock::new(state),
        }
    }

    fn token_url(&self, grant_type: &str) -> String {
        format!("{}/auth/v1/token?grant_type={grant_type}", self.supabase_url)
    }

    /// Sign in with email/password. Replaces any existing session.
    pub async fn login(&self, email: &str, password: &str) -> Result<()> {
        let resp = self
            .http
            .post(self.token_url("password"))
            .header("apikey", &self.anon_key)
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send()
            .await?;
        let session = self.parse_token_response(resp).await?;
        self.store(session).await;
        Ok(())
    }

    /// Sign out: drop the in-memory session and remove `session.json`.
    pub async fn logout(&self) {
        *self.state.write().await = None;
        let _ = std::fs::remove_file(&self.session_path);
    }

    pub async fn status(&self) -> SessionStatus {
        match self.state.read().await.as_ref() {
            Some(s) => SessionStatus {
                signed_in: true,
                email: s.email.clone(),
            },
            None => SessionStatus {
                signed_in: false,
                email: None,
            },
        }
    }

    /// Return a usable `access_token`, refreshing first if it is missing or
    /// (near) expired.
    pub async fn bearer(&self) -> Result<String> {
        {
            let guard = self.state.read().await;
            match guard.as_ref() {
                None => return Err(Error::Auth("not signed in".into())),
                Some(s) if now() < s.expires_at - EXPIRY_SKEW_SECS => {
                    return Ok(s.access_token.clone());
                }
                Some(_) => {}
            }
        }
        self.refresh().await
    }

    /// Force a refresh regardless of the cached expiry (used after a 401).
    pub async fn force_refresh(&self) -> Result<String> {
        self.refresh().await
    }

    async fn refresh(&self) -> Result<String> {
        let refresh_token = {
            let guard = self.state.read().await;
            match guard.as_ref() {
                Some(s) => s.refresh_token.clone(),
                None => return Err(Error::Auth("not signed in".into())),
            }
        };

        let resp = self
            .http
            .post(self.token_url("refresh_token"))
            .header("apikey", &self.anon_key)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::BAD_REQUEST
            || resp.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            // Refresh token is invalid/revoked — force a fresh sign-in.
            self.logout().await;
            return Err(Error::Auth("session expired, please sign in again".into()));
        }

        let session = self.parse_token_response(resp).await?;
        let access = session.access_token.clone();
        self.store(session).await;
        Ok(access)
    }

    async fn parse_token_response(&self, resp: reqwest::Response) -> Result<StoredSession> {
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = extract_error_message(&body).unwrap_or_else(|| format!("HTTP {status}"));
            return Err(Error::Auth(msg));
        }

        let t: TokenResponse = resp.json().await?;
        let expires_at = t.expires_at.unwrap_or_else(|| now() + t.expires_in);
        Ok(StoredSession {
            refresh_token: t.refresh_token,
            access_token: t.access_token,
            expires_at,
            email: t.user.and_then(|u| u.email),
        })
    }

    async fn store(&self, session: StoredSession) {
        if let Ok(json) = serde_json::to_string(&session) {
            if std::fs::write(&self.session_path, json).is_ok() {
                restrict_permissions(&self.session_path);
            }
        }
        *self.state.write().await = Some(session);
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Supabase error bodies look like `{"error_description": "..."}` or
/// `{"msg": "..."}` or `{"error": "..."}`. Pull whichever is present.
fn extract_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    for key in ["error_description", "msg", "error", "message"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}

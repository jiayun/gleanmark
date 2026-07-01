//! Cloud-interest waitlist submission.
//!
//! Cloud is invite-only for now, so instead of self-serve signup the app offers
//! a small form to register interest (and willingness to pay). Submissions go
//! straight to a Supabase `waitlist` table via the baked publishable anon key
//! (INSERT-only under RLS — the key can add rows but never read the list).

use crate::error::{Error, Result};
use crate::models::{DEFAULT_SUPABASE_ANON_KEY, DEFAULT_SUPABASE_URL};

/// Insert one waitlist entry. A duplicate email (unique constraint) is treated
/// as success — the person is already on the list.
pub async fn submit_waitlist(email: &str, pay_interest: &str, note: Option<&str>) -> Result<()> {
    let url = format!(
        "{}/rest/v1/waitlist",
        DEFAULT_SUPABASE_URL.trim_end_matches('/')
    );
    let resp = reqwest::Client::new()
        .post(url)
        .header("apikey", DEFAULT_SUPABASE_ANON_KEY)
        .bearer_auth(DEFAULT_SUPABASE_ANON_KEY)
        .header("Prefer", "return=minimal")
        .json(&serde_json::json!({
            "email": email,
            "pay_interest": pay_interest,
            "note": note,
        }))
        .send()
        .await?;

    if resp.status().is_success() || resp.status() == reqwest::StatusCode::CONFLICT {
        Ok(())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(Error::Gateway(format!(
            "waitlist submit failed: HTTP {status}: {body}"
        )))
    }
}

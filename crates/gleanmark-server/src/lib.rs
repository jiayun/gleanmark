//! Shared Axum router for the local HTTP API + web UI.
//!
//! Used by both the standalone `gleanmark-server` binary and the Tauri desktop
//! app, so the two never drift apart (they previously duplicated the routes).

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gleanmark_core::models::{BackendMode, BookmarkInput, Config, SearchQuery};
use gleanmark_core::GleanMark;
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

pub type AppState = Arc<GleanMark>;

/// Build the full application router. CORS is permissive (the server binds to
/// localhost only); the `/api/config` write path is additionally Origin-guarded
/// below because permissive CORS does not stop cross-site mutation.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/bookmarks", post(create_bookmark).get(list_bookmarks))
        .route("/api/bookmarks/{id}", axum::routing::delete(delete_bookmark))
        .route("/api/search", post(search_bookmarks))
        .route("/api/export", post(export_bookmarks))
        .route("/api/import", post(import_bookmarks))
        .route("/api/open", post(open_url))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/auth/status", get(auth_status))
        .route("/api/auth/login", post(auth_login))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/usage", get(get_usage_summary))
        .route("/api/waitlist", post(join_waitlist))
        .fallback(static_handler)
        .layer(cors)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn create_bookmark(
    State(gm): State<AppState>,
    Json(input): Json<BookmarkInput>,
) -> Result<impl IntoResponse, AppError> {
    let bookmark = gm.save_bookmark(input).await?;
    Ok((StatusCode::CREATED, Json(bookmark)))
}

#[derive(Deserialize)]
struct ListParams {
    #[serde(default = "default_limit")]
    limit: usize,
    offset: Option<String>,
}

fn default_limit() -> usize {
    50
}

async fn list_bookmarks(
    State(gm): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<gleanmark_core::models::Bookmark>>, AppError> {
    let bookmarks = gm.list(params.limit, params.offset).await?;
    Ok(Json(bookmarks))
}

async fn delete_bookmark(
    State(gm): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    gm.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn search_bookmarks(
    State(gm): State<AppState>,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Vec<gleanmark_core::models::SearchResult>>, AppError> {
    let results = gm.search(query).await?;
    Ok(Json(results))
}

#[derive(Deserialize)]
struct ExportRequest {
    path: String,
}

async fn export_bookmarks(
    State(gm): State<AppState>,
    Json(req): Json<ExportRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let count = gm.export_json(std::path::Path::new(&req.path)).await?;
    Ok(Json(serde_json::json!({ "exported": count })))
}

#[derive(Deserialize)]
struct ImportRequest {
    path: String,
}

async fn import_bookmarks(
    State(gm): State<AppState>,
    Json(req): Json<ImportRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let count = gm.import_json(std::path::Path::new(&req.path)).await?;
    Ok(Json(serde_json::json!({ "imported": count })))
}

#[derive(Deserialize)]
struct OpenRequest {
    url: String,
}

async fn open_url(Json(req): Json<OpenRequest>) -> StatusCode {
    match open::that(&req.url) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// ---------------------------------------------------------------------------
// Cloud config (Settings UI)
// ---------------------------------------------------------------------------

/// The UI's only legitimate origin. `PUT /api/config` accepts requests from this
/// exact origin and rejects `null`/missing — a sandboxed iframe on a malicious
/// site serializes its origin to `null` and could otherwise rewrite the gateway
/// URL. CORS alone does not stop the cross-site request from executing.
const UI_ORIGIN: &str = "http://127.0.0.1:21580";

/// Reject any mutation whose `Origin` is not exactly the UI's. A sandboxed
/// iframe on a malicious site serializes its origin to `null`, so missing/`null`
/// is rejected too. CORS alone does not stop the cross-site request executing.
fn origin_ok(headers: &HeaderMap) -> bool {
    headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) == Some(UI_ORIGIN)
}

fn forbidden_origin() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": "forbidden origin" })),
    )
        .into_response()
}

#[derive(Serialize)]
struct ConfigView {
    mode: &'static str,
    /// The effective gateway URL (baked hosted default in cloud mode). Public;
    /// shown only as "Current: Cloud — …". The user never types it.
    gateway_url: Option<String>,
}

/// GET /api/config — current backend mode. The hosted cloud connection details
/// are baked in, so there is nothing for the user to enter or for us to redact.
async fn get_config() -> Json<ConfigView> {
    let c = Config::load();
    Json(ConfigView {
        mode: match c.mode {
            BackendMode::Cloud => "cloud",
            BackendMode::Local => "local",
        },
        gateway_url: c.gateway_url,
    })
}

#[derive(Deserialize)]
struct ConfigUpdate {
    mode: String,
}

/// PUT /api/config — write `{data_dir}/cloud.toml`. Origin-guarded. Cloud mode
/// just records `mode = "cloud"`; `Config::load()` supplies the hosted defaults.
async fn put_config(headers: HeaderMap, Json(body): Json<ConfigUpdate>) -> Response {
    if !origin_ok(&headers) {
        return forbidden_origin();
    }

    let path = Config::load().cloud_config_path();

    let contents = match body.mode.as_str() {
        "local" => "mode = \"local\"\n",
        "cloud" => "mode = \"cloud\"\n",
        _ => return bad_request("mode must be \"local\" or \"cloud\""),
    };

    if let Err(e) = std::fs::write(&path, contents) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }
    restrict_permissions(&path);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "restart_required": true })),
    )
        .into_response()
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Auth (Supabase sign-in) — cloud mode only
// ---------------------------------------------------------------------------

/// GET /api/auth/status — whether cloud mode is active and a user is signed in.
async fn auth_status(State(gm): State<AppState>) -> Json<serde_json::Value> {
    match gm.session_manager() {
        Some(session) => {
            let s = session.status().await;
            Json(serde_json::json!({
                "cloud": true,
                "signed_in": s.signed_in,
                "email": s.email,
            }))
        }
        None => Json(serde_json::json!({ "cloud": false, "signed_in": false })),
    }
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

/// POST /api/auth/login — sign in to Supabase. Origin-guarded (carries
/// credentials and mutates session state). Takes effect immediately — no restart.
async fn auth_login(
    State(gm): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Response {
    if !origin_ok(&headers) {
        return forbidden_origin();
    }
    let Some(session) = gm.session_manager() else {
        return bad_request("not in cloud mode — save cloud settings and restart first");
    };
    match session.login(&body.email, &body.password).await {
        Ok(()) => {
            let s = session.status().await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "signed_in": true, "email": s.email })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/auth/logout — clear the local session. Origin-guarded.
async fn auth_logout(State(gm): State<AppState>, headers: HeaderMap) -> Response {
    if !origin_ok(&headers) {
        return forbidden_origin();
    }
    if let Some(session) = gm.session_manager() {
        session.logout().await;
    }
    (StatusCode::OK, Json(serde_json::json!({ "signed_in": false }))).into_response()
}

/// GET /api/usage — cloud usage summary, or `{}` in local mode.
async fn get_usage_summary(State(gm): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(gm.usage().await?.unwrap_or_else(|| serde_json::json!({}))))
}

// ---------------------------------------------------------------------------
// Cloud-interest waitlist (invite-only phase)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WaitlistRequest {
    email: String,
    #[serde(default)]
    pay_interest: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

/// POST /api/waitlist — register interest (and willingness to pay) in the hosted
/// cloud. Origin-guarded. Submits to the Supabase `waitlist` table via the baked
/// anon key; works in local mode too (that's who we're collecting).
async fn join_waitlist(headers: HeaderMap, Json(body): Json<WaitlistRequest>) -> Response {
    if !origin_ok(&headers) {
        return forbidden_origin();
    }
    let email = body.email.trim();
    if !email.contains('@') || email.len() < 3 {
        return bad_request("a valid email is required");
    }
    let pay_interest = body.pay_interest.as_deref().unwrap_or("unknown");
    let note = body.note.as_deref().map(str::trim).filter(|s| !s.is_empty());

    match gleanmark_core::waitlist::submit_waitlist(email, pay_interest, note).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}

// ---------------------------------------------------------------------------
// Errors & static assets
// ---------------------------------------------------------------------------

struct AppError(gleanmark_core::error::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            gleanmark_core::error::Error::NotFound(_) => StatusCode::NOT_FOUND,
            gleanmark_core::error::Error::QuotaExceeded { .. } => StatusCode::PAYMENT_REQUIRED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut body = serde_json::json!({ "error": self.0.to_string() });
        if matches!(&self.0, gleanmark_core::error::Error::QuotaExceeded { .. }) {
            body["code"] = "quota_exceeded".into();
        }
        (status, Json(body)).into_response()
    }
}

impl From<gleanmark_core::error::Error> for AppError {
    fn from(err: gleanmark_core::error::Error) -> Self {
        AppError(err)
    }
}

#[derive(rust_embed::Embed)]
#[folder = "static/"]
struct Assets;

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => match Assets::get("index.html") {
            Some(file) => (
                [(header::CONTENT_TYPE, "text/html")],
                file.data.to_vec(),
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn quota_exceeded_maps_to_402_with_code() {
        let err = AppError(gleanmark_core::error::Error::QuotaExceeded {
            message: "Monthly bookmark limit reached (30/30). Upgrade your plan to save more."
                .into(),
            used: Some(30),
            limit: Some(30),
        });
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        let body = body_json(resp).await;
        assert_eq!(body["code"], "quota_exceeded");
        assert!(body["error"].as_str().unwrap().contains("Monthly"));
    }

    #[tokio::test]
    async fn other_errors_stay_500_without_code() {
        let err = AppError(gleanmark_core::error::Error::Gateway("boom".into()));
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = body_json(resp).await;
        assert!(body.get("code").is_none());
    }
}

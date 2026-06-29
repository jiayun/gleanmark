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

#[derive(Serialize)]
struct ConfigView {
    mode: &'static str,
    gateway_url: Option<String>,
    gateway_token_set: bool,
    /// Masked preview only — never the plaintext token.
    gateway_token_masked: Option<String>,
}

/// GET /api/config — current backend config with the token redacted.
async fn get_config() -> Json<ConfigView> {
    let c = Config::load();
    Json(ConfigView {
        mode: match c.mode {
            BackendMode::Cloud => "cloud",
            BackendMode::Local => "local",
        },
        gateway_token_masked: c.gateway_token.as_deref().map(mask_token),
        gateway_token_set: c.gateway_token.is_some(),
        gateway_url: c.gateway_url,
    })
}

fn mask_token(t: &str) -> String {
    let n = t.chars().count();
    if n <= 8 {
        return "•".repeat(n);
    }
    let first: String = t.chars().take(4).collect();
    let last: String = t.chars().skip(n - 4).collect();
    format!("{first}…{last}")
}

#[derive(Deserialize)]
struct ConfigUpdate {
    mode: String,
    #[serde(default)]
    gateway_url: Option<String>,
    /// Empty/absent → keep the existing token (the UI never sees the plaintext).
    #[serde(default)]
    gateway_token: Option<String>,
}

/// PUT /api/config — write `{data_dir}/cloud.toml`. Origin-guarded.
async fn put_config(headers: HeaderMap, Json(body): Json<ConfigUpdate>) -> Response {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if origin != Some(UI_ORIGIN) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "forbidden origin" })),
        )
            .into_response();
    }

    let existing = Config::load();
    let path = existing.cloud_config_path();

    let contents = match body.mode.as_str() {
        "local" => "mode = \"local\"\n".to_string(),
        "cloud" => {
            let url = match body.gateway_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(u) => u.to_string(),
                None => return bad_request("gateway_url is required for cloud mode"),
            };
            // Preserve the existing token when the field is left blank.
            let token = match body
                .gateway_token
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(t) => t.to_string(),
                None => match existing.gateway_token {
                    Some(t) => t,
                    None => return bad_request("gateway_token is required for cloud mode"),
                },
            };
            format_cloud_toml(&url, &token)
        }
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

fn format_cloud_toml(url: &str, token: &str) -> String {
    format!(
        "mode = \"cloud\"\ngateway_url = \"{}\"\ngateway_token = \"{}\"\n",
        toml_escape(url),
        toml_escape(token)
    )
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = serde_json::json!({ "error": self.0.to_string() });
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

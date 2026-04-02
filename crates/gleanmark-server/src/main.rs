use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use gleanmark_core::models::{BookmarkInput, Config, SearchQuery};
use gleanmark_core::GleanMark;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

type AppState = Arc<GleanMark>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::default();
    let gleanmark = GleanMark::new(config).await?;
    let state: AppState = Arc::new(gleanmark);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/bookmarks", post(create_bookmark))
        .route("/api/bookmarks", get(list_bookmarks))
        .route("/api/bookmarks/{id}", delete(delete_bookmark))
        .route("/api/search", post(search_bookmarks))
        .route("/api/export", post(export_bookmarks))
        .route("/api/import", post(import_bookmarks))
        .route("/api/open", post(open_url))
        .fallback(static_handler)
        .layer(cors)
        .with_state(state);

    let addr = "127.0.0.1:21580";
    info!("Starting server at http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
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
    axum::extract::Query(params): axum::extract::Query<ListParams>,
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

// Error handling
struct AppError(gleanmark_core::error::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
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
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => match Assets::get("index.html") {
            Some(file) => (
                [(axum::http::header::CONTENT_TYPE, "text/html")],
                file.data.to_vec(),
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}

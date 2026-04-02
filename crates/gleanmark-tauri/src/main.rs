#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use std::sync::Arc;

use tauri::Manager;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_app(&handle).await {
                    eprintln!("Failed to start: {e}");
                    std::process::exit(1);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window on close instead of quitting (stay in tray)
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn start_app(handle: &tauri::AppHandle) -> anyhow::Result<()> {
    // Update splash screen status
    update_splash(handle, "Starting Qdrant...");

    // Prepare Qdrant sidecar symlink
    prepare_sidecar();

    // Initialize backend (starts Qdrant, loads models)
    update_splash(handle, "Loading embedding models...");
    let config = gleanmark_core::models::Config::default();
    let gm = Arc::new(gleanmark_core::GleanMark::new(config).await?);

    // Start Axum server in background
    update_splash(handle, "Starting server...");
    let gm_for_server = Arc::clone(&gm);
    tauri::async_runtime::spawn(async move {
        run_axum_server(gm_for_server).await;
    });

    // Wait for server to be ready
    wait_for_server_ready().await;

    // Setup tray and global shortcut
    tray::create_tray(handle)?;
    tray::register_global_shortcut(handle)
        .map_err(|e| anyhow::anyhow!("Failed to register shortcut: {e}"))?;

    // Transition: close splash, navigate main window, then show it
    if let Some(main_win) = handle.get_webview_window("main") {
        let _ = main_win
            .navigate("http://127.0.0.1:21580".parse().unwrap());
        // Give WebView a moment to start loading
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = main_win.show();
        let _ = main_win.set_focus();
    }
    if let Some(splash) = handle.get_webview_window("splashscreen") {
        let _ = splash.close();
    }

    // Keep GleanMark alive for the app lifetime
    // QdrantManager::drop will kill Qdrant on process exit
    std::mem::forget(gm);

    Ok(())
}

fn update_splash(handle: &tauri::AppHandle, msg: &str) {
    if let Some(splash) = handle.get_webview_window("splashscreen") {
        let js = format!(
            "document.getElementById('status').textContent = '{}'",
            msg.replace('\'', "\\'")
        );
        let _ = splash.eval(&js);
    }
}

fn prepare_sidecar() {
    let Some(data_dir) = dirs::data_local_dir() else { return };
    let bin_dir = data_dir.join("gleanmark").join("bin");
    let target = bin_dir.join("qdrant");

    if target.exists() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else { return };
    let Some(exe_dir) = exe.parent() else { return };

    let sidecar = exe_dir.join(format!(
        "qdrant-{}-apple-darwin",
        std::env::consts::ARCH
    ));

    if sidecar.exists() {
        let _ = std::fs::create_dir_all(&bin_dir);
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&sidecar, &target);
        }
    }
}

async fn run_axum_server(gm: Arc<gleanmark_core::GleanMark>) {
    use axum::extract::{Path, Query};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::{delete, get, post};
    use axum::{Json, Router};
    use serde::Deserialize;
    use tower_http::cors::{Any, CorsLayer};

    #[derive(Deserialize)]
    struct OpenRequest {
        url: String,
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

    #[derive(Deserialize)]
    struct ExportRequest {
        path: String,
    }

    #[derive(Deserialize)]
    struct ImportRequest {
        path: String,
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route(
            "/api/bookmarks",
            post({
                let gm = Arc::clone(&gm);
                move |Json(input): Json<gleanmark_core::models::BookmarkInput>| {
                    let gm = Arc::clone(&gm);
                    async move {
                        match gm.save_bookmark(input).await {
                            Ok(b) => (StatusCode::CREATED, Json(serde_json::to_value(b).unwrap()))
                                .into_response(),
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                                .into_response(),
                        }
                    }
                }
            }),
        )
        .route(
            "/api/bookmarks",
            get({
                let gm = Arc::clone(&gm);
                move |Query(params): Query<ListParams>| {
                    let gm = Arc::clone(&gm);
                    async move {
                        match gm.list(params.limit, params.offset).await {
                            Ok(b) => Json(serde_json::to_value(b).unwrap()).into_response(),
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                                .into_response(),
                        }
                    }
                }
            }),
        )
        .route(
            "/api/bookmarks/{id}",
            delete({
                let gm = Arc::clone(&gm);
                move |Path(id): Path<String>| {
                    let gm = Arc::clone(&gm);
                    async move {
                        match gm.delete(&id).await {
                            Ok(()) => StatusCode::NO_CONTENT.into_response(),
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                                .into_response(),
                        }
                    }
                }
            }),
        )
        .route(
            "/api/search",
            post({
                let gm = Arc::clone(&gm);
                move |Json(query): Json<gleanmark_core::models::SearchQuery>| {
                    let gm = Arc::clone(&gm);
                    async move {
                        match gm.search(query).await {
                            Ok(r) => Json(serde_json::to_value(r).unwrap()).into_response(),
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                                .into_response(),
                        }
                    }
                }
            }),
        )
        .route(
            "/api/export",
            post({
                let gm = Arc::clone(&gm);
                move |Json(req): Json<ExportRequest>| {
                    let gm = Arc::clone(&gm);
                    async move {
                        match gm.export_json(std::path::Path::new(&req.path)).await {
                            Ok(count) => {
                                Json(serde_json::json!({"exported": count})).into_response()
                            }
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                                .into_response(),
                        }
                    }
                }
            }),
        )
        .route(
            "/api/import",
            post({
                let gm = Arc::clone(&gm);
                move |Json(req): Json<ImportRequest>| {
                    let gm = Arc::clone(&gm);
                    async move {
                        match gm.import_json(std::path::Path::new(&req.path)).await {
                            Ok(count) => {
                                Json(serde_json::json!({"imported": count})).into_response()
                            }
                            Err(e) => (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"error": e.to_string()})),
                            )
                                .into_response(),
                        }
                    }
                }
            }),
        )
        .route(
            "/api/open",
            post(|Json(req): Json<OpenRequest>| async move {
                match open::that(&req.url) {
                    Ok(()) => StatusCode::NO_CONTENT.into_response(),
                    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                }
            }),
        )
        .fallback(static_handler)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:21580")
        .await
        .expect("Failed to bind port 21580");
    axum::serve(listener, app)
        .await
        .expect("Axum server failed");
}

#[derive(rust_embed::Embed)]
#[folder = "../gleanmark-server/static/"]
struct Assets;

async fn static_handler(uri: axum::http::Uri) -> axum::response::Response {
    use axum::response::IntoResponse;

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
            None => axum::http::StatusCode::NOT_FOUND.into_response(),
        },
    }
}

async fn wait_for_server_ready() {
    for _ in 0..60 {
        if tokio::net::TcpStream::connect("127.0.0.1:21580")
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

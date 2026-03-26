use std::path::PathBuf;

use clap::{Parser, Subcommand};
use gleanmark_core::models::{BookmarkInput, Config, SearchQuery};
use gleanmark_core::GleanMark;

#[derive(Parser)]
#[command(name = "gleanmark", about = "Bookmark semantic search tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a bookmark
    Add {
        /// URL of the bookmark
        url: String,
        /// Title
        #[arg(short, long)]
        title: Option<String>,
        /// Content / description
        #[arg(short, long)]
        content: Option<String>,
        /// Tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
    },
    /// Search bookmarks
    Search {
        /// Search query
        query: String,
        /// Max results
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Filter by tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
    },
    /// List bookmarks
    List {
        /// Max results
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },
    /// Delete a bookmark by ID
    Delete {
        /// Bookmark ID
        id: String,
    },
    /// Export bookmarks to JSON file
    Export {
        /// Output file path
        path: PathBuf,
    },
    /// Import bookmarks from JSON file
    Import {
        /// Input file path
        path: PathBuf,
    },
    /// Start the HTTP API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "21580")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = Config::default();
    let gm = GleanMark::new(config).await?;

    match cli.command {
        Commands::Add {
            url,
            title,
            content,
            tags,
        } => {
            let input = BookmarkInput {
                url: url.clone(),
                title: title.unwrap_or_else(|| url.clone()),
                content: content.unwrap_or_default(),
                tags,
            };
            let bookmark = gm.save_bookmark(input).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&bookmark)?);
            } else {
                println!("Saved: {} ({})", bookmark.title, bookmark.id);
            }
        }

        Commands::Search { query, limit, tags } => {
            let sq = SearchQuery {
                query,
                limit: Some(limit),
                tags,
            };
            let results = gm.search(sq).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else if results.is_empty() {
                println!("No results found.");
            } else {
                for r in &results {
                    println!(
                        "[{:.3}] {} — {}",
                        r.score, r.bookmark.title, r.bookmark.url
                    );
                    if !r.bookmark.tags.is_empty() {
                        println!("       tags: {}", r.bookmark.tags.join(", "));
                    }
                }
            }
        }

        Commands::List { limit } => {
            let bookmarks = gm.list(limit).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&bookmarks)?);
            } else if bookmarks.is_empty() {
                println!("No bookmarks.");
            } else {
                for b in &bookmarks {
                    println!("{} — {} ({})", b.id, b.title, b.url);
                }
            }
        }

        Commands::Delete { id } => {
            gm.delete(&id).await?;
            println!("Deleted: {id}");
        }

        Commands::Export { path } => {
            let count = gm.export_json(&path).await?;
            println!("Exported {count} bookmarks to {}", path.display());
        }

        Commands::Import { path } => {
            let count = gm.import_json(&path).await?;
            println!("Imported {count} bookmarks from {}", path.display());
        }

        Commands::Serve { port } => {
            // Drop the GleanMark instance - the server creates its own
            drop(gm);
            println!("Starting server at http://127.0.0.1:{port}");
            println!("Note: Use gleanmark-server binary for the HTTP server.");
            println!("This subcommand is a convenience alias.");
            // Re-use the server logic by invoking the same flow
            serve(port).await?;
        }
    }

    Ok(())
}

async fn serve(port: u16) -> anyhow::Result<()> {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use tower_http::cors::{Any, CorsLayer};

    let config = Config::default();
    let gm = Arc::new(GleanMark::new(config).await?);

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
                move |Json(input): Json<BookmarkInput>| {
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
            "/api/search",
            post({
                let gm = Arc::clone(&gm);
                move |Json(query): Json<SearchQuery>| {
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
        .layer(cors);

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

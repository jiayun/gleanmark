use std::sync::Arc;

use gleanmark_core::models::{BookmarkInput, Config, SearchQuery};
use gleanmark_core::GleanMark;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router, ServerHandler,
};
use tokio::sync::OnceCell;

use crate::types::{AddBookmarkParams, DeleteParams, ListParams, SearchParams};

pub struct GleanMarkMcp {
    gm: Arc<OnceCell<GleanMark>>,
    tool_router: ToolRouter<Self>,
}

impl GleanMarkMcp {
    pub fn new() -> Self {
        let tool_router = Self::bookmark_router();
        Self {
            gm: Arc::new(OnceCell::new()),
            tool_router,
        }
    }

    async fn gm(&self) -> Result<&GleanMark, String> {
        self.gm
            .get_or_try_init(|| async {
                let mut config = Config::default();
                config.show_download_progress = false;
                GleanMark::new(config)
                    .await
                    .map_err(|e| format!("Failed to initialize: {e}"))
            })
            .await
            .map_err(|e| e.to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GleanMarkMcp {
    fn get_info(&self) -> ServerInfo {
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability::default());
        ServerInfo::new(caps)
            .with_server_info(Implementation::new(
                "gleanmark-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "GleanMark bookmark search MCP server. Search, list, add, and delete bookmarks with semantic search support.",
            )
    }
}

#[tool_router(router = bookmark_router)]
impl GleanMarkMcp {
    #[tool(
        name = "search_bookmarks",
        description = "Search bookmarks using semantic and keyword matching. Returns ranked results with relevance scores."
    )]
    async fn search(&self, Parameters(params): Parameters<SearchParams>) -> String {
        let gm = match self.gm().await {
            Ok(gm) => gm,
            Err(e) => return e,
        };
        let query = SearchQuery {
            query: params.query,
            limit: params.limit,
            tags: params.tags,
        };
        match gm.search(query).await {
            Ok(results) => {
                if results.is_empty() {
                    "No results found.".to_string()
                } else {
                    serde_json::to_string_pretty(&results).unwrap_or_else(|e| format!("Error: {e}"))
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "list_bookmarks",
        description = "List all saved bookmarks, ordered by most recent."
    )]
    async fn list(&self, Parameters(params): Parameters<ListParams>) -> String {
        let gm = match self.gm().await {
            Ok(gm) => gm,
            Err(e) => return e,
        };
        let limit = params.limit.unwrap_or(20);
        match gm.list(limit, params.offset).await {
            Ok(bookmarks) => {
                if bookmarks.is_empty() {
                    "No bookmarks.".to_string()
                } else {
                    serde_json::to_string_pretty(&bookmarks)
                        .unwrap_or_else(|e| format!("Error: {e}"))
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "add_bookmark",
        description = "Save a new bookmark with URL, title, optional content, and tags."
    )]
    async fn add(&self, Parameters(params): Parameters<AddBookmarkParams>) -> String {
        let gm = match self.gm().await {
            Ok(gm) => gm,
            Err(e) => return e,
        };
        let input = BookmarkInput {
            url: params.url,
            title: params.title,
            content: params.content.unwrap_or_default(),
            tags: params.tags,
        };
        match gm.save_bookmark(input).await {
            Ok(bookmark) => format!("Saved: {} ({})", bookmark.title, bookmark.id),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(
        name = "delete_bookmark",
        description = "Delete a bookmark by its ID."
    )]
    async fn delete(&self, Parameters(params): Parameters<DeleteParams>) -> String {
        let gm = match self.gm().await {
            Ok(gm) => gm,
            Err(e) => return e,
        };
        match gm.delete(&params.id).await {
            Ok(()) => format!("Deleted: {}", params.id),
            Err(e) => format!("Error: {e}"),
        }
    }
}

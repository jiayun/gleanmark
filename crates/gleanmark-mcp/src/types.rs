use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Search query (supports semantic and keyword matching).
    pub query: String,

    /// Maximum number of results to return (default: 10).
    #[serde(default)]
    pub limit: Option<usize>,

    /// Filter results by tags.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListParams {
    /// Maximum number of bookmarks to return (default: 20).
    #[serde(default)]
    pub limit: Option<usize>,

    /// Bookmark ID to start after (for pagination). Use the last bookmark's ID from the previous page.
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddBookmarkParams {
    /// URL of the bookmark.
    pub url: String,

    /// Title of the bookmark.
    pub title: String,

    /// Page content or description.
    #[serde(default)]
    pub content: Option<String>,

    /// Tags for categorization.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteParams {
    /// Bookmark ID to delete.
    pub id: String,
}

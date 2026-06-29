use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use qdrant_client::qdrant::Value;
use qdrant_client::Payload;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkInput {
    pub url: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub bookmark: Bookmark,
    pub score: f32,
}

/// Where the Qdrant backend lives.
///
/// `Local` (default) starts and manages a bundled Qdrant subprocess.
/// `Cloud` connects to a remote Qdrant (e.g. Railway) and skips the subprocess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendMode {
    #[default]
    Local,
    Cloud,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: BackendMode,
    /// Local Qdrant address (used in local mode only).
    pub qdrant_url: String,
    /// Cloud gateway base URL, e.g. `https://...railway.app` (cloud mode only).
    pub gateway_url: Option<String>,
    /// Bearer token sent to the gateway (cloud mode only). Personal: a static
    /// token; Phase 2: a Supabase JWT.
    pub gateway_token: Option<String>,
    pub data_dir: PathBuf,
    pub collection_name: String,
    /// Show download progress for embedding models (disable for MCP/stdio)
    pub show_download_progress: bool,
}

/// On-disk overlay read from `{data_dir}/cloud.toml`. Every field is optional so
/// a partial file only overrides what it specifies; absent fields keep defaults.
#[derive(Debug, Deserialize)]
struct CloudFile {
    mode: Option<BackendMode>,
    qdrant_url: Option<String>,
    gateway_url: Option<String>,
    gateway_token: Option<String>,
    collection_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gleanmark");

        Self {
            mode: BackendMode::Local,
            qdrant_url: "http://localhost:6334".to_string(),
            gateway_url: None,
            gateway_token: None,
            data_dir,
            collection_name: "bookmarks".to_string(),
            show_download_progress: true,
        }
    }
}

impl Config {
    pub fn is_cloud(&self) -> bool {
        self.mode == BackendMode::Cloud
    }

    /// Path of the optional cloud-config overlay file.
    pub fn cloud_config_path(&self) -> PathBuf {
        self.data_dir.join("cloud.toml")
    }

    /// Start from defaults (local mode), then overlay `{data_dir}/cloud.toml`
    /// if present. A missing file leaves local mode untouched; a malformed file
    /// is logged and ignored so the app still starts.
    pub fn load() -> Self {
        let mut config = Self::default();
        let path = config.cloud_config_path();

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return config, // no overlay → local mode
        };

        match toml::from_str::<CloudFile>(&contents) {
            Ok(file) => {
                if let Some(mode) = file.mode {
                    config.mode = mode;
                }
                if let Some(url) = file.qdrant_url {
                    config.qdrant_url = url;
                }
                if file.gateway_url.is_some() {
                    config.gateway_url = file.gateway_url;
                }
                if file.gateway_token.is_some() {
                    config.gateway_token = file.gateway_token;
                }
                if let Some(name) = file.collection_name {
                    config.collection_name = name;
                }
            }
            Err(e) => {
                tracing::warn!("Ignoring malformed {}: {e}", path.display());
            }
        }

        config
    }
}

impl Bookmark {
    pub fn to_payload(&self) -> Payload {
        let mut map = HashMap::new();
        map.insert("url".to_string(), Value::from(self.url.as_str()));
        map.insert("title".to_string(), Value::from(self.title.as_str()));
        map.insert("content".to_string(), Value::from(self.content.as_str()));
        map.insert(
            "tags".to_string(),
            Value::from(
                self.tags
                    .iter()
                    .map(|t| Value::from(t.as_str()))
                    .collect::<Vec<_>>(),
            ),
        );
        map.insert(
            "created_at".to_string(),
            Value::from(self.created_at.to_rfc3339().as_str()),
        );
        map.insert(
            "updated_at".to_string(),
            Value::from(self.updated_at.to_rfc3339().as_str()),
        );
        Payload::from(map)
    }

    pub fn from_payload(id: &str, payload: &HashMap<String, Value>) -> Option<Self> {
        let url = payload.get("url")?.as_str()?.to_string();
        let title = payload.get("title")?.as_str()?.to_string();
        let content = payload.get("content")?.as_str()?.to_string();

        let tags = payload
            .get("tags")
            .and_then(|v| v.as_list())
            .map(|list| {
                list.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let created_at = payload
            .get("created_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let updated_at = payload
            .get("updated_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        Some(Bookmark {
            id: id.to_string(),
            url,
            title,
            content,
            tags,
            created_at,
            updated_at,
        })
    }
}

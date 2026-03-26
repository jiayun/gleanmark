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

#[derive(Debug, Clone)]
pub struct Config {
    pub qdrant_url: String,
    pub data_dir: PathBuf,
    pub collection_name: String,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gleanmark");

        Self {
            qdrant_url: "http://localhost:6334".to_string(),
            data_dir,
            collection_name: "bookmarks".to_string(),
        }
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

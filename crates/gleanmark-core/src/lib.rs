pub mod embedding;
pub mod error;
pub mod models;
pub mod qdrant_manager;
pub mod search;
pub mod storage;

use std::path::Path;

use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use crate::embedding::EmbeddingService;
use crate::error::{Error, Result};
use crate::models::{Bookmark, BookmarkInput, Config, SearchQuery, SearchResult};
use crate::qdrant_manager::QdrantManager;
use crate::storage::Storage;

pub struct GleanMark {
    embedding: EmbeddingService,
    storage: Storage,
    _qdrant_manager: QdrantManager,
}

impl GleanMark {
    pub async fn new(config: Config) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir)?;

        let qdrant_manager = QdrantManager::start(&config).await?;
        let storage = Storage::new(qdrant_manager.url(), &config.collection_name).await?;
        let embedding = EmbeddingService::new()?;

        Ok(Self {
            embedding,
            storage,
            _qdrant_manager: qdrant_manager,
        })
    }

    pub async fn save_bookmark(&self, input: BookmarkInput) -> Result<Bookmark> {
        let now = Utc::now();
        let bookmark = Bookmark {
            id: Uuid::new_v4().to_string(),
            url: input.url,
            title: input.title.clone(),
            content: input.content.clone(),
            tags: input.tags.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };

        let embed_text = format!("{} {}", input.title, input.content);
        let result = self.embedding.embed_passage(&embed_text).await?;

        self.storage
            .upsert(&bookmark, result.dense, result.sparse)
            .await?;

        info!("Saved bookmark: {} ({})", bookmark.title, bookmark.id);
        Ok(bookmark)
    }

    pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        let limit = query.limit.unwrap_or(10) as u64;
        let result = self.embedding.embed_query(&query.query).await?;

        let tag_filter = query.tags.as_deref();
        search::hybrid_search(&self.storage, &result, limit, tag_filter).await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.storage.delete(id).await?;
        info!("Deleted bookmark: {id}");
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Bookmark> {
        self.storage
            .get(id)
            .await?
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }

    pub async fn list(&self, limit: usize, offset: Option<String>) -> Result<Vec<Bookmark>> {
        self.storage.list(limit as u32, offset).await
    }

    pub async fn export_json(&self, path: &Path) -> Result<usize> {
        let mut all = Vec::new();
        let batch_size = 100u32;
        let mut offset: Option<String> = None;

        loop {
            let batch = self.storage.list(batch_size, offset).await?;
            if batch.is_empty() {
                break;
            }
            offset = batch.last().map(|b| b.id.clone());
            all.extend(batch);
        }

        let count = all.len();
        let json = serde_json::to_string_pretty(&all)?;
        std::fs::write(path, json)?;

        info!("Exported {count} bookmarks to {}", path.display());
        Ok(count)
    }

    pub async fn reindex(&self) -> Result<usize> {
        // Collect all bookmarks first to avoid cursor invalidation during upsert
        let mut all = Vec::new();
        let batch_size = 100u32;
        let mut offset: Option<String> = None;

        loop {
            let batch = self.storage.list(batch_size, offset).await?;
            if batch.is_empty() {
                break;
            }
            offset = batch.last().map(|b| b.id.clone());
            all.extend(batch);
        }

        let total = all.len();
        for (i, bookmark) in all.iter().enumerate() {
            let embed_text = format!("{} {}", bookmark.title, bookmark.content);
            let result = self.embedding.embed_passage(&embed_text).await?;
            self.storage
                .upsert(bookmark, result.dense, result.sparse)
                .await?;
            info!("Re-indexed {}/{total}: {}", i + 1, bookmark.title);
        }

        info!("Re-indexed {total} bookmarks total");
        Ok(total)
    }

    pub async fn import_json(&self, path: &Path) -> Result<usize> {
        let data = std::fs::read_to_string(path)?;
        let bookmarks: Vec<Bookmark> = serde_json::from_str(&data)?;
        let count = bookmarks.len();

        for bookmark in &bookmarks {
            let embed_text = format!("{} {}", bookmark.title, bookmark.content);
            let result = self.embedding.embed_passage(&embed_text).await?;
            self.storage
                .upsert(bookmark, result.dense, result.sparse)
                .await?;
        }

        info!("Imported {count} bookmarks from {}", path.display());
        Ok(count)
    }
}

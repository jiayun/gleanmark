pub mod backend;
pub mod embedding;
pub mod error;
pub mod models;
pub mod qdrant_manager;
pub mod search;
pub mod session;
pub mod storage;

// The `GleanMark` facade does client-side embedding, so it (and only it) needs
// the `embed` feature. The cloud gateway uses `storage`/`search`/`backend`
// directly and builds this crate with `--no-default-features`.
#[cfg(feature = "embed")]
use std::path::Path;

#[cfg(feature = "embed")]
use chrono::Utc;
#[cfg(feature = "embed")]
use tracing::info;
#[cfg(feature = "embed")]
use uuid::Uuid;

#[cfg(feature = "embed")]
use crate::backend::{Backend, GatewayBackend, QdrantBackend};
#[cfg(feature = "embed")]
use crate::embedding::EmbeddingService;
#[cfg(feature = "embed")]
use crate::error::{Error, Result};
#[cfg(feature = "embed")]
use crate::models::{Bookmark, BookmarkInput, Config, SearchQuery, SearchResult};
#[cfg(feature = "embed")]
use crate::qdrant_manager::QdrantManager;
#[cfg(feature = "embed")]
use crate::session::SessionManager;
#[cfg(feature = "embed")]
use crate::storage::Storage;
#[cfg(feature = "embed")]
use std::sync::Arc;

#[cfg(feature = "embed")]
pub struct GleanMark {
    embedding: EmbeddingService,
    backend: Box<dyn Backend>,
    /// Local Qdrant subprocess, kept alive for the app lifetime. `None` in
    /// cloud mode (the gateway owns Qdrant).
    _qdrant_manager: Option<QdrantManager>,
    /// Supabase session, shared with the cloud backend. `None` in local mode.
    /// Exposed so the HTTP layer can drive login/logout without a restart.
    session: Option<Arc<SessionManager>>,
}

#[cfg(feature = "embed")]
impl GleanMark {
    pub async fn new(config: Config) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir)?;

        // Embeddings are computed client-side in every mode.
        let cache_dir = config.data_dir.join("models");
        let embedding =
            EmbeddingService::with_full_options(config.show_download_progress, Some(cache_dir))?;

        #[allow(clippy::type_complexity)]
        let (backend, qdrant_manager, session): (
            Box<dyn Backend>,
            Option<QdrantManager>,
            Option<Arc<SessionManager>>,
        ) = if config.is_cloud() {
            let url = config.gateway_url.clone().ok_or_else(|| {
                Error::Other("cloud mode requires gateway_url in cloud.toml".into())
            })?;
            let supabase_url = config.supabase_url.clone().ok_or_else(|| {
                Error::Other("cloud mode requires supabase_url in cloud.toml".into())
            })?;
            let anon_key = config.supabase_anon_key.clone().ok_or_else(|| {
                Error::Other("cloud mode requires supabase_anon_key in cloud.toml".into())
            })?;
            info!("Cloud mode: using gateway at {url}");
            let session = Arc::new(SessionManager::new(
                supabase_url,
                anon_key,
                config.session_path(),
            ));
            let backend = Box::new(GatewayBackend::new(url, session.clone()));
            (backend, None, Some(session))
        } else {
            let manager = QdrantManager::start(&config).await?;
            let storage = Storage::new(manager.url(), &config.collection_name, None).await?;
            (Box::new(QdrantBackend::new(storage)), Some(manager), None)
        };

        Ok(Self {
            embedding,
            backend,
            _qdrant_manager: qdrant_manager,
            session,
        })
    }

    /// The Supabase session manager (cloud mode only). The HTTP layer uses this
    /// to sign in/out and report status without restarting the app.
    pub fn session_manager(&self) -> Option<Arc<SessionManager>> {
        self.session.clone()
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

        self.backend.upsert(&bookmark, result).await?;

        info!("Saved bookmark: {} ({})", bookmark.title, bookmark.id);
        Ok(bookmark)
    }

    pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        let limit = query.limit.unwrap_or(10) as u64;
        let result = self.embedding.embed_query(&query.query).await?;

        let tag_filter = query.tags.as_deref();
        self.backend.search(&result, limit, tag_filter).await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.backend.delete(id).await?;
        info!("Deleted bookmark: {id}");
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Bookmark> {
        self.backend
            .get(id)
            .await?
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }

    pub async fn list(&self, limit: usize, offset: Option<String>) -> Result<Vec<Bookmark>> {
        let (bookmarks, _next) = self.backend.list(limit as u32, offset).await?;
        Ok(bookmarks)
    }

    pub async fn export_json(&self, path: &Path) -> Result<usize> {
        let mut all = Vec::new();
        let batch_size = 100u32;
        let mut offset: Option<String> = None;

        loop {
            let (batch, next_offset) = self.backend.list(batch_size, offset).await?;
            all.extend(batch);
            match next_offset {
                Some(next) => offset = Some(next),
                None => break,
            }
        }

        let count = all.len();
        let json = serde_json::to_string_pretty(&all)?;
        std::fs::write(path, json)?;

        info!("Exported {count} bookmarks to {}", path.display());
        Ok(count)
    }

    pub async fn reindex(&self) -> Result<usize> {
        // Collect all bookmarks first to avoid cursor invalidation during upsert
        println!("Reading all bookmarks from storage...");
        let mut all = Vec::new();
        let batch_size = 100u32;
        let mut offset: Option<String> = None;

        loop {
            let (batch, next_offset) = self.backend.list(batch_size, offset).await?;
            all.extend(batch);
            match next_offset {
                Some(next) => offset = Some(next),
                None => break,
            }
        }

        let total = all.len();
        println!("Total: {total} bookmarks to re-embed");
        for (i, bookmark) in all.iter().enumerate() {
            let embed_text = format!("{} {}", bookmark.title, bookmark.content);
            let result = self.embedding.embed_passage(&embed_text).await?;
            self.backend.upsert(bookmark, result).await?;
            println!("  [{}/{}] {}", i + 1, total, bookmark.title);
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
            self.backend.upsert(bookmark, result).await?;
        }

        info!("Imported {count} bookmarks from {}", path.display());
        Ok(count)
    }
}

//! Storage/IO backend behind the `GleanMark` facade.
//!
//! Embeddings are always computed client-side; a backend only persists and
//! queries *pre-computed* vectors. Two implementations:
//! - [`QdrantBackend`] — talks gRPC to a Qdrant instance via [`Storage`]. Used
//!   in local mode (and reused inside the cloud gateway service).
//! - [`GatewayBackend`] — HTTP/JSON to the cloud gateway. Used in cloud mode.
//!
//! The wire DTOs ([`UpsertBody`], [`SearchBody`], [`ListResponse`]) are public
//! so the gateway service (separate crate) can deserialize the same shapes.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::embedding::EmbeddingResult;
use crate::error::{Error, Result};
use crate::models::{Bookmark, SearchResult};
use crate::search;
use crate::session::SessionManager;
use crate::storage::Storage;

#[async_trait]
pub trait Backend: Send + Sync {
    async fn upsert(&self, bookmark: &Bookmark, embedding: EmbeddingResult) -> Result<()>;
    async fn search(
        &self,
        embedding: &EmbeddingResult,
        limit: u64,
        tags: Option<&[String]>,
    ) -> Result<Vec<SearchResult>>;
    async fn get(&self, id: &str) -> Result<Option<Bookmark>>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn list(
        &self,
        limit: u32,
        offset: Option<String>,
    ) -> Result<(Vec<Bookmark>, Option<String>)>;

    /// Usage summary (cloud mode). `None` when the backend has no notion of
    /// usage (local mode).
    async fn usage(&self) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Wire DTOs (shared with the gateway service)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertBody {
    #[serde(flatten)]
    pub bookmark: Bookmark,
    pub dense: Vec<f32>,
    pub sparse_indices: Vec<u32>,
    pub sparse_values: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchBody {
    pub dense: Vec<f32>,
    pub sparse_indices: Vec<u32>,
    pub sparse_values: Vec<f32>,
    pub limit: u64,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub bookmarks: Vec<Bookmark>,
    pub next: Option<String>,
}

// ---------------------------------------------------------------------------
// Local backend: direct Qdrant via the existing Storage layer
// ---------------------------------------------------------------------------

pub struct QdrantBackend {
    storage: Storage,
}

impl QdrantBackend {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl Backend for QdrantBackend {
    async fn upsert(&self, bookmark: &Bookmark, embedding: EmbeddingResult) -> Result<()> {
        self.storage
            .upsert(bookmark, embedding.dense, embedding.sparse)
            .await
    }

    async fn search(
        &self,
        embedding: &EmbeddingResult,
        limit: u64,
        tags: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        search::hybrid_search(&self.storage, embedding, limit, tags).await
    }

    async fn get(&self, id: &str) -> Result<Option<Bookmark>> {
        self.storage.get(id).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.storage.delete(id).await
    }

    async fn list(
        &self,
        limit: u32,
        offset: Option<String>,
    ) -> Result<(Vec<Bookmark>, Option<String>)> {
        self.storage.list(limit, offset).await
    }
}

// ---------------------------------------------------------------------------
// Cloud backend: HTTP/JSON to the gateway
// ---------------------------------------------------------------------------

pub struct GatewayBackend {
    http: reqwest::Client,
    base_url: String,
    session: Arc<SessionManager>,
}

impl GatewayBackend {
    pub fn new(base_url: String, session: Arc<SessionManager>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            session,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Send a request with the current `access_token`; on a 401, refresh the
    /// session once and retry. `build` is called per attempt with a fresh
    /// bearer token so the rebuilt request carries the new credential.
    async fn send(
        &self,
        build: impl Fn(&str) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        let token = self.session.bearer().await?;
        let resp = build(&token).send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let token = self.session.force_refresh().await?;
            return Ok(build(&token).send().await?);
        }
        Ok(resp)
    }
}

/// Treat any non-2xx response as a gateway error, surfacing the body.
async fn ensure_ok(resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(Error::Gateway(format!("HTTP {status}: {body}")))
    }
}

#[async_trait]
impl Backend for GatewayBackend {
    async fn upsert(&self, bookmark: &Bookmark, embedding: EmbeddingResult) -> Result<()> {
        let body = UpsertBody {
            bookmark: bookmark.clone(),
            dense: embedding.dense,
            sparse_indices: embedding.sparse.indices,
            sparse_values: embedding.sparse.values,
        };
        let resp = self
            .send(|tok| self.http.post(self.url("/v1/bookmarks")).bearer_auth(tok).json(&body))
            .await?;
        ensure_ok(resp).await?;
        Ok(())
    }

    async fn search(
        &self,
        embedding: &EmbeddingResult,
        limit: u64,
        tags: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        let body = SearchBody {
            dense: embedding.dense.clone(),
            sparse_indices: embedding.sparse.indices.clone(),
            sparse_values: embedding.sparse.values.clone(),
            limit,
            tags: tags.map(|t| t.to_vec()),
        };
        let resp = self
            .send(|tok| self.http.post(self.url("/v1/search")).bearer_auth(tok).json(&body))
            .await?;
        let resp = ensure_ok(resp).await?;
        Ok(resp.json().await?)
    }

    async fn get(&self, id: &str) -> Result<Option<Bookmark>> {
        let path = format!("/v1/bookmarks/{id}");
        let resp = self
            .send(|tok| self.http.get(self.url(&path)).bearer_auth(tok))
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let resp = ensure_ok(resp).await?;
        Ok(Some(resp.json().await?))
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let path = format!("/v1/bookmarks/{id}");
        let resp = self
            .send(|tok| self.http.delete(self.url(&path)).bearer_auth(tok))
            .await?;
        ensure_ok(resp).await?;
        Ok(())
    }

    async fn list(
        &self,
        limit: u32,
        offset: Option<String>,
    ) -> Result<(Vec<Bookmark>, Option<String>)> {
        let resp = self
            .send(|tok| {
                let mut req = self
                    .http
                    .get(self.url("/v1/bookmarks"))
                    .bearer_auth(tok)
                    .query(&[("limit", limit.to_string())]);
                if let Some(ref off) = offset {
                    req = req.query(&[("offset", off)]);
                }
                req
            })
            .await?;
        let resp = ensure_ok(resp).await?;
        let body: ListResponse = resp.json().await?;
        Ok((body.bookmarks, body.next))
    }

    async fn usage(&self) -> Result<Option<serde_json::Value>> {
        let resp = self
            .send(|tok| self.http.get(self.url("/v1/usage")).bearer_auth(tok))
            .await?;
        let resp = ensure_ok(resp).await?;
        Ok(Some(resp.json().await?))
    }
}

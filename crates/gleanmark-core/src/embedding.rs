use std::sync::{Arc, LazyLock, Mutex};

use fastembed::{
    EmbeddingModel, InitOptions, SparseInitOptions, SparseModel, SparseTextEmbedding,
    TextEmbedding,
};
use opencc_jieba_rs::OpenCC;

use crate::error::Result;

static OPENCC: LazyLock<OpenCC> = LazyLock::new(OpenCC::new);

pub struct EmbeddingService {
    dense: Arc<Mutex<TextEmbedding>>,
    sparse: Arc<Mutex<SparseTextEmbedding>>,
}

pub struct EmbeddingResult {
    pub dense: Vec<f32>,
    pub sparse: SparseVec,
}

pub struct SparseVec {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

/// Segment Chinese text with jieba, rejoin with spaces.
/// Non-Chinese text passes through with minimal impact.
fn segment_for_sparse(text: &str) -> String {
    OPENCC
        .jieba_cut(text, false)
        .into_iter()
        .filter(|w| !w.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl EmbeddingService {
    pub fn new() -> Result<Self> {
        Self::with_options(true)
    }

    pub fn with_options(show_download_progress: bool) -> Result<Self> {
        Self::with_full_options(show_download_progress, None)
    }

    pub fn with_full_options(
        show_download_progress: bool,
        cache_dir: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        let mut dense_opts = InitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_show_download_progress(show_download_progress);
        let mut sparse_opts = SparseInitOptions::new(SparseModel::SPLADEPPV1)
            .with_show_download_progress(show_download_progress);

        if let Some(ref dir) = cache_dir {
            dense_opts = dense_opts.with_cache_dir(dir.clone());
            sparse_opts = sparse_opts.with_cache_dir(dir.clone());
        }

        let dense = TextEmbedding::try_new(dense_opts)?;
        let sparse = SparseTextEmbedding::try_new(sparse_opts)?;

        Ok(Self {
            dense: Arc::new(Mutex::new(dense)),
            sparse: Arc::new(Mutex::new(sparse)),
        })
    }

    /// Embed text for indexing (storing a bookmark).
    /// Uses "passage: " prefix for E5 model.
    pub async fn embed_passage(&self, text: &str) -> Result<EmbeddingResult> {
        let dense_text = format!("passage: {text}");
        let sparse_text = segment_for_sparse(text);
        self.embed_inner(dense_text, sparse_text).await
    }

    /// Embed text for searching (query).
    /// Uses "query: " prefix for E5 model.
    pub async fn embed_query(&self, text: &str) -> Result<EmbeddingResult> {
        let dense_text = format!("query: {text}");
        let sparse_text = segment_for_sparse(text);
        self.embed_inner(dense_text, sparse_text).await
    }

    async fn embed_inner(
        &self,
        dense_text: String,
        sparse_text: String,
    ) -> Result<EmbeddingResult> {
        let dense_model = Arc::clone(&self.dense);
        let dense = tokio::task::spawn_blocking(move || {
            let mut model = dense_model.lock().unwrap();
            model.embed(vec![dense_text], None)
        })
        .await??;

        let sparse_model = Arc::clone(&self.sparse);
        let sparse = tokio::task::spawn_blocking(move || {
            let mut model = sparse_model.lock().unwrap();
            model.embed(vec![sparse_text], None)
        })
        .await??;

        let dense_vec = dense.into_iter().next().unwrap_or_default();
        let sparse_emb = sparse.into_iter().next().ok_or_else(|| {
            crate::error::Error::Embedding("No sparse embedding returned".to_string())
        })?;

        Ok(EmbeddingResult {
            dense: dense_vec,
            sparse: SparseVec {
                indices: sparse_emb
                    .indices
                    .into_iter()
                    .map(|i| i as u32)
                    .collect(),
                values: sparse_emb.values,
            },
        })
    }
}

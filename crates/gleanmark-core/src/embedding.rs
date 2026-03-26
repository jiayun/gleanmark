use std::sync::{Arc, Mutex};

use fastembed::{
    EmbeddingModel, InitOptions, SparseInitOptions, SparseModel, SparseTextEmbedding,
    TextEmbedding,
};

use crate::error::Result;

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

impl EmbeddingService {
    pub fn new() -> Result<Self> {
        let dense = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;

        let sparse = SparseTextEmbedding::try_new(
            SparseInitOptions::new(SparseModel::SPLADEPPV1).with_show_download_progress(true),
        )?;

        Ok(Self {
            dense: Arc::new(Mutex::new(dense)),
            sparse: Arc::new(Mutex::new(sparse)),
        })
    }

    pub async fn embed(&self, text: &str) -> Result<EmbeddingResult> {
        let text = text.to_string();

        let dense_model = Arc::clone(&self.dense);
        let text_for_dense = text.clone();
        let dense = tokio::task::spawn_blocking(move || {
            let mut model = dense_model.lock().unwrap();
            model.embed(vec![text_for_dense], None)
        })
        .await??;

        let sparse_model = Arc::clone(&self.sparse);
        let text_for_sparse = text;
        let sparse = tokio::task::spawn_blocking(move || {
            let mut model = sparse_model.lock().unwrap();
            model.embed(vec![text_for_sparse], None)
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

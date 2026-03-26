use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::{
    Condition, Filter, Fusion, PrefetchQueryBuilder, Query, QueryPointsBuilder,
};

use crate::embedding::EmbeddingResult;
use crate::error::Result;
use crate::models::{Bookmark, SearchResult};
use crate::storage::Storage;

pub async fn hybrid_search(
    storage: &Storage,
    embedding: &EmbeddingResult,
    limit: u64,
    tag_filter: Option<&[String]>,
) -> Result<Vec<SearchResult>> {
    // Dense query from Vec<f32>
    let dense_query = Query::from(embedding.dense.clone());

    // Sparse query from Vec<(u32, f32)>
    let sparse_pairs: Vec<(u32, f32)> = embedding
        .sparse
        .indices
        .iter()
        .copied()
        .zip(embedding.sparse.values.iter().copied())
        .collect();
    let sparse_query = Query::from(sparse_pairs);

    let dense_prefetch = PrefetchQueryBuilder::default()
        .query(dense_query)
        .using("dense")
        .limit(20u64)
        .build();

    let sparse_prefetch = PrefetchQueryBuilder::default()
        .query(sparse_query)
        .using("sparse")
        .limit(20u64)
        .build();

    let mut builder = QueryPointsBuilder::new(storage.collection())
        .add_prefetch(dense_prefetch)
        .add_prefetch(sparse_prefetch)
        .query(Query::new_fusion(Fusion::Rrf))
        .limit(limit)
        .with_payload(true);

    if let Some(tags) = tag_filter {
        let tag_strings: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
        let filter = Filter::must([Condition::matches("tags", tag_strings)]);
        builder = builder.filter(filter);
    }

    let response = storage.client().query(builder).await?;

    let results = response
        .result
        .iter()
        .filter_map(|p| {
            let id = p.id.as_ref().and_then(|pid| match &pid.point_id_options {
                Some(PointIdOptions::Uuid(s)) => Some(s.clone()),
                Some(PointIdOptions::Num(n)) => Some(n.to_string()),
                None => None,
            })?;
            let bookmark = Bookmark::from_payload(&id, &p.payload)?;
            Some(SearchResult {
                bookmark,
                score: p.score,
            })
        })
        .collect();

    Ok(results)
}

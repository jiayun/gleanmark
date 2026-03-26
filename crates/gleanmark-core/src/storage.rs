use std::collections::HashMap;

use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::vectors::VectorsOptions;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, DeletePointsBuilder, Distance, GetPointsBuilder, Modifier,
    NamedVectors, PointStruct, PointsIdsList, ScrollPointsBuilder, SparseVectorParamsBuilder,
    UpsertPointsBuilder, Vector, VectorParamsBuilder, Vectors,
};
use qdrant_client::Qdrant;
use tracing::info;

use crate::embedding::SparseVec;
use crate::error::Result;
use crate::models::Bookmark;

pub struct Storage {
    client: Qdrant,
    collection: String,
}

fn point_id_to_string(id: &qdrant_client::qdrant::PointId) -> Option<String> {
    match &id.point_id_options {
        Some(PointIdOptions::Uuid(s)) => Some(s.clone()),
        Some(PointIdOptions::Num(n)) => Some(n.to_string()),
        None => None,
    }
}

impl Storage {
    pub async fn new(url: &str, collection: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;

        let storage = Self {
            client,
            collection: collection.to_string(),
        };

        storage.ensure_collection().await?;
        Ok(storage)
    }

    pub fn client(&self) -> &Qdrant {
        &self.client
    }

    pub fn collection(&self) -> &str {
        &self.collection
    }

    async fn ensure_collection(&self) -> Result<()> {
        if self.client.collection_exists(&self.collection).await? {
            info!("Collection '{}' already exists", self.collection);
            return Ok(());
        }

        info!("Creating collection '{}'", self.collection);

        let mut vectors_config = qdrant_client::qdrant::VectorsConfigBuilder::default();
        vectors_config.add_named_vector_params(
            "dense",
            VectorParamsBuilder::new(384, Distance::Cosine),
        );

        let mut sparse_vectors: HashMap<String, qdrant_client::qdrant::SparseVectorParams> =
            HashMap::new();
        sparse_vectors.insert(
            "sparse".to_string(),
            SparseVectorParamsBuilder::default()
                .modifier(Modifier::Idf)
                .build(),
        );

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(vectors_config)
                    .sparse_vectors_config(sparse_vectors),
            )
            .await?;

        info!("Collection '{}' created", self.collection);
        Ok(())
    }

    pub async fn upsert(
        &self,
        bookmark: &Bookmark,
        dense: Vec<f32>,
        sparse: SparseVec,
    ) -> Result<()> {
        let payload = bookmark.to_payload();

        let mut named = NamedVectors::default();
        named
            .vectors
            .insert("dense".to_string(), Vector::new_dense(dense));
        named.vectors.insert(
            "sparse".to_string(),
            Vector::new_sparse(sparse.indices, sparse.values),
        );

        let vectors = Vectors {
            vectors_options: Some(VectorsOptions::Vectors(named)),
        };

        let point = PointStruct::new(bookmark.id.clone(), vectors, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<Bookmark>> {
        let response = self
            .client
            .get_points(
                GetPointsBuilder::new(&self.collection, vec![id.to_string().into()])
                    .with_payload(true),
            )
            .await?;

        Ok(response.result.first().and_then(|p| {
            let pid = p.id.as_ref().and_then(point_id_to_string)?;
            Bookmark::from_payload(&pid, &p.payload)
        }))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection)
                    .points(PointsIdsList {
                        ids: vec![id.to_string().into()],
                    })
                    .wait(true),
            )
            .await?;

        Ok(())
    }

    pub async fn list(&self, limit: u32, offset: Option<String>) -> Result<Vec<Bookmark>> {
        let mut builder = ScrollPointsBuilder::new(&self.collection)
            .limit(limit)
            .with_payload(true);

        if let Some(ref offset_id) = offset {
            builder =
                builder.offset(qdrant_client::qdrant::PointId::from(offset_id.clone()));
        }

        let response = self.client.scroll(builder).await?;

        let bookmarks = response
            .result
            .iter()
            .filter_map(|p| {
                let id = p.id.as_ref().and_then(point_id_to_string)?;
                Bookmark::from_payload(&id, &p.payload)
            })
            .collect();

        Ok(bookmarks)
    }
}

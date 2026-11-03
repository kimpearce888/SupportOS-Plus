use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub(crate) type PointId = String;

pub(crate) type DenseVector = Vec<f32>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Filter {
    pub must: HashMap<String, String>,
}

impl Filter {
    #[must_use]
    pub fn matches(&self, payload: &Payload) -> bool {
        for (key, value) in &self.must {
            match payload.get(key) {
                Some(v) => {
                    if v.as_str() != Some(value.as_str()) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

pub(crate) type Payload = serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub id: PointId,
    pub dense: Option<DenseVector>,
    pub sparse: Option<SparseVector>,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ScoredPoint {
    pub id: PointId,
    pub score: f32,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CollectionInfo {
    pub name: String,
    pub dense_dim: Option<usize>,
    pub point_count: usize,
}

pub trait VectorStore: Send + Sync {
    fn create_collection(&self, name: &str, dense_dim: Option<usize>) -> Result<()>;

    fn drop_collection(&self, name: &str) -> Result<()>;

    fn upsert(&self, collection: &str, point: Point) -> Result<()>;

    fn search_dense(
        &self,
        collection: &str,
        query: &[f32],
        filter: Option<&Filter>,
        top_k: usize,
    ) -> Result<Vec<ScoredPoint>>;

    fn collection_info(&self, name: &str) -> Result<Option<CollectionInfo>>;
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub type PointId = String;

pub type DenseVector = Vec<f32>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

impl SparseVector {
    #[must_use]
    pub fn new(mut indices: Vec<u32>, mut values: Vec<f32>) -> Self {
        assert_eq!(
            indices.len(),
            values.len(),
            "sparse vector: indices and values must have the same length"
        );
        let mut pairs: Vec<(u32, f32)> = indices
            .iter()
            .zip(values.iter())
            .map(|(&i, &v)| (i, v))
            .collect();
        pairs.sort_by_key(|(i, _)| *i);
        indices = pairs.iter().map(|(i, _)| *i).collect();
        values = pairs.iter().map(|(_, v)| *v).collect();
        Self { indices, values }
    }

    #[must_use]
    pub fn dot_product(&self, other: &Self) -> f32 {
        let mut sum = 0.0_f32;
        let mut i = 0;
        let mut j = 0;
        while i < self.indices.len() && j < other.indices.len() {
            if self.indices[i] == other.indices[j] {
                sum += self.values[i] * other.values[j];
                i += 1;
                j += 1;
            } else if self.indices[i] < other.indices[j] {
                i += 1;
            } else {
                j += 1;
            }
        }
        sum
    }

    #[must_use]
    pub fn l2_norm(&self) -> f32 {
        self.values.iter().map(|v| v * v).sum::<f32>().sqrt()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Filter {
    pub must: HashMap<String, String>,
}

impl Filter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn must_eq(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.must.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.must.is_empty()
    }

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

pub type Payload = serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub id: PointId,
    pub dense: Option<DenseVector>,
    pub sparse: Option<SparseVector>,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredPoint {
    pub id: PointId,
    pub score: f32,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub dense_dim: Option<usize>,
    pub point_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSnapshot {
    pub name: String,
    pub dense_dim: Option<usize>,
    pub points: Vec<Point>,
}

pub trait VectorStore: Send + Sync {
    fn create_collection(&self, name: &str, dense_dim: Option<usize>) -> Result<()>;

    fn drop_collection(&self, name: &str) -> Result<()>;

    fn upsert(&self, collection: &str, point: Point) -> Result<()>;

    fn delete(&self, collection: &str, id: &PointId) -> Result<()>;

    fn search_dense(
        &self,
        collection: &str,
        query: &[f32],
        filter: Option<&Filter>,
        top_k: usize,
    ) -> Result<Vec<ScoredPoint>>;

    fn search_sparse(
        &self,
        collection: &str,
        query: &SparseVector,
        filter: Option<&Filter>,
        top_k: usize,
    ) -> Result<Vec<ScoredPoint>>;

    fn count(&self, collection: &str, filter: Option<&Filter>) -> Result<usize>;

    fn collection_info(&self, name: &str) -> Result<Option<CollectionInfo>>;

    fn snapshot(&self, name: &str) -> Result<Vec<u8>>;

    fn restore(&self, bytes: &[u8]) -> Result<()>;
}

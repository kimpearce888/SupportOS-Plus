use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use qdrant_edge::{
    Condition, Distance, EdgeConfig, EdgeShard, EdgeSparseVectorParams, EdgeVectorParams,
    FieldCondition, Filter as QdrantFilter, JsonPath, Match as QdrantMatch, MatchValue,
    PointInsertOperations, PointOperations, PointStructPersisted as PointStruct, QueryEnum,
    QueryRequestBuilder, ScoringQuery, ScrollRequestBuilder, UpdateOperation, ValueVariants,
    VectorInternal, VectorPersisted, VectorStructInternal, VectorStructPersisted,
    WithPayloadInterface, WithVector,
};

use crate::error::{Error, Result};
use crate::vectorstore::{
    CollectionInfo, CollectionSnapshot, DenseVector, Filter, Point, PointId, ScoredPoint,
    SparseVector, VectorStore,
};

const SPARSE_VECTOR_NAME: &str = "text";

const EDGE_CONFIG_FILE: &str = "edge_config.json";

pub struct QdrantEdgeVectorStore {
    data_dir: PathBuf,
    shards: Mutex<HashMap<String, EdgeShard>>,
    dims: Mutex<HashMap<String, Option<usize>>>,
}

impl QdrantEdgeVectorStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir).map_err(Error::Io)?;
        Ok(Self {
            data_dir,
            shards: Mutex::new(HashMap::new()),
            dims: Mutex::new(HashMap::new()),
        })
    }

    fn collection_path(&self, name: &str) -> PathBuf {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.data_dir.join(safe)
    }

    fn open_shard(&self, name: &str, dense_dim: Option<usize>) -> Result<()> {
        let mut shards = self.shards.lock().expect("mutex poisoned");
        if shards.contains_key(name) {
            return Ok(());
        }

        let path = self.collection_path(name);
        std::fs::create_dir_all(&path).map_err(Error::Io)?;

        let mut config_builder = EdgeConfig::builder();
        if let Some(dim) = dense_dim {
            config_builder = config_builder.vector(
                qdrant_edge::DEFAULT_VECTOR_NAME.to_string(),
                EdgeVectorParams::builder(dim, Distance::Cosine).build(),
            );
        }
        config_builder = config_builder.sparse_vector(
            SPARSE_VECTOR_NAME.to_string(),
            EdgeSparseVectorParams::default(),
        );
        let config = config_builder.build();

        let shard = if path.join(EDGE_CONFIG_FILE).exists() {
            let config_opt = dense_dim.map(|_| config);
            EdgeShard::load(&path, config_opt).map_err(map_qdrant_err)?
        } else {
            EdgeShard::new(&path, config).map_err(map_qdrant_err)?
        };

        shards.insert(name.to_string(), shard);
        self.dims
            .lock()
            .expect("mutex poisoned")
            .insert(name.to_string(), dense_dim);
        Ok(())
    }

    fn ensure_shard_open(&self, name: &str) -> Result<()> {
        {
            let shards = self.shards.lock().expect("mutex poisoned");
            if shards.contains_key(name) {
                return Ok(());
            }
        }
        if !self.collection_path(name).join(EDGE_CONFIG_FILE).exists() {
            return Err(Error::Config(format!("collection '{name}' does not exist")));
        }
        self.open_shard(name, None)
    }

    fn dim_for(&self, name: &str) -> Option<usize> {
        self.dims
            .lock()
            .expect("mutex poisoned")
            .get(name)
            .copied()
            .flatten()
    }

    fn translate_filter(filter: &Filter) -> Option<QdrantFilter> {
        if filter.must.is_empty() {
            return None;
        }
        let conditions: Vec<Condition> = filter
            .must
            .iter()
            .map(|(key, value)| {
                let condition = FieldCondition {
                    key: std::convert::TryFrom::try_from(key.as_str()).unwrap_or_else(|_| {
                        JsonPath {
                            first_key: String::new(),
                            rest: Vec::new(),
                        }
                    }),
                    r#match: Some(QdrantMatch::Value(MatchValue {
                        value: ValueVariants::String(value.clone()),
                    })),
                    range: None,
                    geo_bounding_box: None,
                    geo_radius: None,
                    geo_polygon: None,
                    values_count: None,
                    is_empty: None,
                    is_null: None,
                };
                Condition::Field(condition)
            })
            .collect();
        Some(QdrantFilter {
            should: None,
            min_should: None,
            must: Some(conditions),
            must_not: None,
        })
    }
}

impl VectorStore for QdrantEdgeVectorStore {
    fn create_collection(&self, name: &str, dense_dim: Option<usize>) -> Result<()> {
        self.open_shard(name, dense_dim)
    }

    fn drop_collection(&self, name: &str) -> Result<()> {
        let mut shards = self.shards.lock().expect("mutex poisoned");
        shards.remove(name);
        self.dims.lock().expect("mutex poisoned").remove(name);
        let path = self.collection_path(name);
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(Error::Io)?;
        }
        Ok(())
    }

    fn upsert(&self, collection: &str, point: Point) -> Result<()> {
        if !self
            .shards
            .lock()
            .expect("mutex poisoned")
            .contains_key(collection)
        {
            return Err(Error::Config(format!(
                "collection '{collection}' does not exist; call create_collection first"
            )));
        }

        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(collection)
            .ok_or_else(|| Error::Config(format!("collection '{collection}' does not exist")))?;

        let id_u64 = point_id_to_u64(&point.id)?;
        let vector = match (&point.dense, &point.sparse) {
            (Some(dense), Some(sparse)) => {
                let mut named: HashMap<String, VectorPersisted> = HashMap::new();
                named.insert(
                    qdrant_edge::DEFAULT_VECTOR_NAME.to_string(),
                    VectorPersisted::Dense(dense.clone()),
                );
                named.insert(
                    SPARSE_VECTOR_NAME.to_string(),
                    VectorPersisted::Sparse(qdrant_sparse_vector(sparse)),
                );
                VectorStructPersisted::Named(named)
            }
            (Some(dense), None) => VectorStructPersisted::Single(dense.clone()),
            (None, Some(sparse)) => {
                let mut named: HashMap<String, VectorPersisted> = HashMap::new();
                named.insert(
                    SPARSE_VECTOR_NAME.to_string(),
                    VectorPersisted::Sparse(qdrant_sparse_vector(sparse)),
                );
                VectorStructPersisted::Named(named)
            }
            (None, None) => {
                return Err(Error::Config(
                    "Point must have at least a dense or sparse vector".to_string(),
                ));
            }
        };

        let point_struct = PointStruct {
            id: id_u64.into(),
            vector,
            payload: value_to_qdrant_payload(point.payload.clone()),
        };

        shard
            .update(UpdateOperation::PointOperation(
                PointOperations::UpsertPoints(PointInsertOperations::PointsList(vec![
                    point_struct,
                ])),
            ))
            .map_err(map_qdrant_err)?;

        shard.flush().map_err(map_qdrant_err)?;

        Ok(())
    }

    fn delete(&self, collection: &str, id: &PointId) -> Result<()> {
        self.ensure_shard_open(collection)?;
        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(collection)
            .ok_or_else(|| Error::Config(format!("collection '{collection}' does not exist")))?;
        let id_u64 = point_id_to_u64(id)?;
        shard
            .update(UpdateOperation::PointOperation(
                PointOperations::DeletePoints {
                    ids: vec![id_u64.into()],
                },
            ))
            .map_err(map_qdrant_err)?;
        Ok(())
    }

    fn search_dense(
        &self,
        collection: &str,
        query: &[f32],
        filter: Option<&Filter>,
        top_k: usize,
    ) -> Result<Vec<ScoredPoint>> {
        self.ensure_shard_open(collection)?;
        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(collection)
            .ok_or_else(|| Error::Config(format!("collection '{collection}' does not exist")))?;

        let query_vec: Vec<f32> = query.to_vec();
        let qdrant_filter = filter.and_then(Self::translate_filter);
        let mut request = QueryRequestBuilder::new(top_k)
            .query(ScoringQuery::Vector(QueryEnum::Nearest(
                qdrant_edge::NamedQuery {
                    query: qdrant_edge::VectorInternal::from(query_vec),
                    using: Some(qdrant_edge::DEFAULT_VECTOR_NAME.to_string()),
                },
            )))
            .with_payload(WithPayloadInterface::Bool(true));
        if let Some(f) = qdrant_filter {
            request = request.filter(f);
        }
        let request = request.build();

        let results = shard.query(request).map_err(map_qdrant_err)?;
        let scored: Vec<ScoredPoint> = results
            .into_iter()
            .map(|r| ScoredPoint {
                id: u64_to_point_id(r.id),
                score: r.score,
                payload: qdrant_payload_to_value(r.payload),
            })
            .collect();
        Ok(scored)
    }

    fn search_sparse(
        &self,
        collection: &str,
        query: &SparseVector,
        filter: Option<&Filter>,
        top_k: usize,
    ) -> Result<Vec<ScoredPoint>> {
        self.ensure_shard_open(collection)?;
        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(collection)
            .ok_or_else(|| Error::Config(format!("collection '{collection}' does not exist")))?;

        let qdrant_sparse = qdrant_sparse_vector(query);
        let qdrant_filter = filter.and_then(Self::translate_filter);
        let mut request = QueryRequestBuilder::new(top_k)
            .query(ScoringQuery::Vector(QueryEnum::Nearest(
                qdrant_edge::NamedQuery {
                    query: VectorInternal::Sparse(qdrant_sparse),
                    using: Some(SPARSE_VECTOR_NAME.to_string()),
                },
            )))
            .with_payload(WithPayloadInterface::Bool(true));
        if let Some(f) = qdrant_filter {
            request = request.filter(f);
        }
        let request = request.build();

        let results = shard.query(request).map_err(map_qdrant_err)?;
        let scored: Vec<ScoredPoint> = results
            .into_iter()
            .map(|r| ScoredPoint {
                id: u64_to_point_id(r.id),
                score: r.score,
                payload: qdrant_payload_to_value(r.payload),
            })
            .collect();
        Ok(scored)
    }

    fn count(&self, collection: &str, filter: Option<&Filter>) -> Result<usize> {
        self.ensure_shard_open(collection)?;
        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(collection)
            .ok_or_else(|| Error::Config(format!("collection '{collection}' does not exist")))?;
        let qdrant_filter = filter.and_then(Self::translate_filter);

        let count_request = qdrant_edge::CountRequest {
            filter: qdrant_filter,
            exact: true,
        };
        let result = shard.count(count_request).map_err(map_qdrant_err)?;
        Ok(result)
    }

    fn collection_info(&self, name: &str) -> Result<Option<CollectionInfo>> {
        {
            let shards = self.shards.lock().expect("mutex poisoned");
            if !shards.contains_key(name)
                && !self.collection_path(name).join(EDGE_CONFIG_FILE).exists()
            {
                return Ok(None);
            }
        }
        self.ensure_shard_open(name)?;
        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(name)
            .ok_or_else(|| Error::Config(format!("collection '{name}' does not exist")))?;
        let info = shard.info().map_err(map_qdrant_err)?;
        Ok(Some(CollectionInfo {
            name: name.to_string(),
            dense_dim: self.dim_for(name),
            point_count: info.points_count,
        }))
    }

    fn snapshot(&self, name: &str) -> Result<Vec<u8>> {
        self.ensure_shard_open(name)?;
        let shards = self.shards.lock().expect("mutex poisoned");
        let shard = shards
            .get(name)
            .ok_or_else(|| Error::Config(format!("collection '{name}' does not exist")))?;

        let mut all_points = Vec::new();
        let mut offset: Option<qdrant_edge::PointId> = None;
        loop {
            let mut req = ScrollRequestBuilder::new()
                .with_payload(WithPayloadInterface::Bool(true))
                .with_vector(WithVector::Bool(true));
            if let Some(off) = offset {
                req = req.offset(off);
            }
            let (records, next_offset) = shard.scroll(req.build()).map_err(map_qdrant_err)?;
            for record in records {
                let id = u64_to_point_id(record.id);
                let payload = qdrant_payload_to_value(record.payload);

                let (dense, sparse) = extract_vectors(record.vector);
                all_points.push(Point {
                    id,
                    dense,
                    sparse,
                    payload,
                });
            }
            if next_offset.is_none() {
                break;
            }
            offset = next_offset;
        }

        let snapshot = CollectionSnapshot {
            name: name.to_string(),
            dense_dim: self.dim_for(name),
            points: all_points,
        };

        serde_json::to_vec(&snapshot)
            .map_err(|e| Error::Config(format!("snapshot serialization failed: {e}")))
    }

    fn restore(&self, bytes: &[u8]) -> Result<()> {
        let snapshot: CollectionSnapshot = serde_json::from_slice(bytes)
            .map_err(|e| Error::Config(format!("snapshot deserialization failed: {e}")))?;

        self.create_collection(&snapshot.name, snapshot.dense_dim)?;

        for point in snapshot.points {
            self.upsert(&snapshot.name, point)?;
        }

        Ok(())
    }
}

fn point_id_to_u64(id: &PointId) -> Result<u64> {
    id.parse::<u64>().map_err(|_| {
        Error::Config(format!(
            "QdrantEdge adapter requires numeric point ids; got '{id}'"
        ))
    })
}

fn u64_to_point_id(id: qdrant_edge::PointId) -> PointId {
    match id {
        qdrant_edge::PointId::NumId(n) => n.to_string(),
        qdrant_edge::PointId::Uuid(u) => u.to_string(),
    }
}

fn value_to_qdrant_payload(value: serde_json::Value) -> Option<qdrant_edge::Payload> {
    match value {
        serde_json::Value::Object(map) => Some(qdrant_edge::Payload(map)),
        serde_json::Value::Null => None,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("value".to_string(), other);
            Some(qdrant_edge::Payload(map))
        }
    }
}

fn qdrant_payload_to_value(payload: Option<qdrant_edge::Payload>) -> serde_json::Value {
    match payload {
        Some(p) => serde_json::Value::Object(p.0),
        None => serde_json::Value::Null,
    }
}

fn qdrant_sparse_vector(sparse: &SparseVector) -> qdrant_edge::SparseVector {
    qdrant_edge::SparseVector {
        indices: sparse.indices.clone(),
        values: sparse.values.clone(),
    }
}

fn extract_vectors(
    vec: Option<VectorStructInternal>,
) -> (Option<DenseVector>, Option<SparseVector>) {
    let mut dense = None;
    let mut sparse = None;

    if let Some(v) = vec {
        match v {
            VectorStructInternal::Single(d) => dense = Some(d),
            VectorStructInternal::Named(named) => {
                for (name, vi) in named {
                    match vi {
                        VectorInternal::Dense(d) => dense = Some(d),
                        VectorInternal::Sparse(s) => {
                            sparse = Some(SparseVector {
                                indices: s.indices,
                                values: s.values,
                            });
                        }
                        VectorInternal::MultiDense(_) => {}
                    }
                    let _ = name;
                }
            }
            VectorStructInternal::MultiDense(_) => {}
        }
    }

    (dense, sparse)
}

fn map_qdrant_err(e: qdrant_edge::OperationError) -> Error {
    Error::Config(format!("qdrant-edge error: {e}"))
}

pub const QDRANT_COLLECTION: &str = "supportos_vectors";

pub const DEFAULT_QDRANT_URL: &str = "http://127.0.0.1:6333";

#[derive(Debug, Clone)]
pub struct VectorPoint {
    pub id: i64,
    pub vector: Vec<f32>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: i64,
    pub score: f32,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Health {
    pub connected: bool,
    pub url: String,
    pub collections: Vec<String>,
    pub error: Option<String>,
}

pub struct EmbeddedQdrant {
    data_dir: PathBuf,
    inner: Mutex<EmbeddedInner>,
}

struct EmbeddedInner {
    enabled: bool,
    url: String,
    store: Option<QdrantEdgeVectorStore>,
    last_error: Option<String>,
}

impl EmbeddedQdrant {
    pub fn new(data_dir: impl Into<PathBuf>, url: &str, enabled: bool) -> Self {
        Self {
            data_dir: data_dir.into(),
            inner: Mutex::new(EmbeddedInner {
                enabled,
                url: url.trim_end_matches('/').to_string(),
                store: None,
                last_error: None,
            }),
        }
    }

    pub fn reconfigure(&self, url: Option<String>, enabled: Option<bool>) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        if let Some(u) = url {
            let trimmed = u.trim_end_matches('/').to_string();
            if trimmed != inner.url {
                inner.url = trimmed;
                inner.store = None;
            }
        }
        if let Some(e) = enabled {
            inner.enabled = e;
        }
    }

    fn dir_for(data_dir: &std::path::Path, url: &str) -> PathBuf {
        let safe: String = url
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        data_dir
            .join("qdrant")
            .join(format!("{safe}-{:08x}", fnv1a(url)))
    }

    fn store<'a>(
        inner: &'a mut EmbeddedInner,
        data_dir: &std::path::Path,
    ) -> Option<&'a QdrantEdgeVectorStore> {
        if !inner.enabled {
            return None;
        }
        if inner.store.is_none() {
            match QdrantEdgeVectorStore::new(Self::dir_for(data_dir, &inner.url)) {
                Ok(s) => {
                    inner.store = Some(s);
                    inner.last_error = None;
                }
                Err(e) => {
                    inner.last_error = Some(format!("Qdrant storage open failed: {e}"));
                    return None;
                }
            }
        }
        inner.store.as_ref()
    }

    pub fn health(&self) -> Health {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        if !inner.enabled {
            return Health {
                connected: false,
                url: inner.url.clone(),
                collections: Vec::new(),
                error: Some("Qdrant disabled in settings".to_string()),
            };
        }
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return Health {
                connected: false,
                url: inner.url.clone(),
                collections: Vec::new(),
                error: Some(
                    inner
                        .last_error
                        .clone()
                        .unwrap_or_else(|| "Qdrant storage unavailable".to_string()),
                ),
            };
        };
        let collections = if store
            .collection_path(QDRANT_COLLECTION)
            .join(EDGE_CONFIG_FILE)
            .exists()
        {
            vec![QDRANT_COLLECTION.to_string()]
        } else {
            Vec::new()
        };
        Health {
            connected: true,
            url: inner.url.clone(),
            collections,
            error: None,
        }
    }

    pub fn ensure_collection(&self, dimension: usize) -> bool {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return false;
        };
        store
            .create_collection(QDRANT_COLLECTION, Some(dimension))
            .is_ok()
    }

    pub fn upsert(&self, points: &[VectorPoint]) -> bool {
        if points.is_empty() {
            return false;
        }
        let mut inner = self.inner.lock().expect("mutex poisoned");
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return false;
        };
        if store
            .collection_info(QDRANT_COLLECTION)
            .ok()
            .flatten()
            .is_none()
        {
            let dim = points[0].vector.len();
            if store
                .create_collection(QDRANT_COLLECTION, Some(dim))
                .is_err()
            {
                return false;
            }
        }
        for p in points {
            let point = Point {
                id: p.id.to_string(),
                dense: Some(p.vector.clone()),
                sparse: None,
                payload: p.payload.clone(),
            };
            if store.upsert(QDRANT_COLLECTION, point).is_err() {
                return false;
            }
        }
        true
    }

    pub fn search(&self, vector: &[f32], limit: usize) -> Vec<SearchHit> {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return Vec::new();
        };
        match store.search_dense(QDRANT_COLLECTION, vector, None, limit) {
            Ok(hits) => hits
                .into_iter()
                .filter_map(|h| {
                    let id = h.id.parse::<i64>().ok()?;
                    Some(SearchHit {
                        id,
                        score: h.score,
                        payload: h.payload,
                    })
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn delete_by_entity(&self, entity_type: &str, entity_ids: &[i64]) -> bool {
        if entity_ids.is_empty() {
            return false;
        }
        let mut inner = self.inner.lock().expect("mutex poisoned");
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return false;
        };
        let Ok(bytes) = store.snapshot(QDRANT_COLLECTION) else {
            return false;
        };
        let Ok(snapshot) = serde_json::from_slice::<CollectionSnapshot>(&bytes) else {
            return false;
        };
        for point in snapshot.points {
            let Some(payload) = point.payload.as_object() else {
                continue;
            };
            let matches_type =
                payload.get("entity_type").and_then(|v| v.as_str()) == Some(entity_type);
            let matches_id = payload
                .get("entity_id")
                .and_then(|v| v.as_i64())
                .is_some_and(|id| entity_ids.contains(&id));
            if matches_type && matches_id && store.delete(QDRANT_COLLECTION, &point.id).is_err() {
                return false;
            }
        }
        true
    }

    pub fn count_by_entity(&self, entity_type: &str) -> usize {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return 0;
        };
        let filter = Filter::new().must_eq("entity_type", entity_type);
        store.count(QDRANT_COLLECTION, Some(&filter)).unwrap_or(0)
    }

    pub fn drop_collection(&self) -> bool {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        let Some(store) = Self::store(&mut inner, &self.data_dir) else {
            return false;
        };
        store.drop_collection(QDRANT_COLLECTION).is_ok()
    }

    pub fn url(&self) -> String {
        self.inner.lock().expect("mutex poisoned").url.clone()
    }

    pub fn enabled(&self) -> bool {
        self.inner.lock().expect("mutex poisoned").enabled
    }
}

fn fnv1a(s: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in s.as_bytes() {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

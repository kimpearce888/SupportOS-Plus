//! sqlite-vec vector store (decision D4: gate passed at 0 chunks < 100 000,
//! so the embedded qdrant engine is out and the dense index lives beside
//! the rest of the data in SQLite).
//!
//! Two layers, mirroring the old `vectorstore_qdrant` shape:
//!
//! * [`SqliteVecStore`] implements the [`VectorStore`] trait seam — the
//!   exact create/drop/upsert/delete/search/count/snapshot/restore
//!   contract the qdrant-edge adapter offered, so the store stays
//!   swappable (D4: "the `VectorStore` trait keeps it reversible").
//! * [`VectorIndex`] is the app-facing facade (lazy init, enable flag,
//!   boolean return codes for the worker pipeline) with the same method
//!   surface the old `EmbeddedQdrant` facade had.
//!
//! Storage layout — one extra SQLite database, `<data_dir>/vectors/
//! vectors.db`, holding:
//!
//! * `vec_meta` — collection name → dense dimension (vec0 tables need the
//!   dimension fixed at `CREATE VIRTUAL TABLE` time).
//! * `vec_payload` — the JSON payload plus the two columns the worker
//!   pipeline filters and deletes by (`entity_type`, `entity_id`).
//! * one `vec0` virtual table per collection
//!   (`point_id INTEGER PRIMARY KEY, embedding FLOAT[dim]
//!   distance_metric=cosine`).
//!
//! Sparse vectors: the hybrid search stack already runs its sparse lane
//! through FTS5 (`search::universal_search`) — the qdrant upserts never
//! carried sparse vectors either. [`VectorStore::search_sparse`] therefore
//! returns an empty result and is pinned by a test.

// The only unsafe code in `spp_core`: registering the sqlite-vec extension
// entry point via rusqlite's FFI (`sqlite3_auto_extension`) before the
// first connection opens. The crate-level lint is `deny` (not `forbid`)
// solely for this module.
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};

use rusqlite::{Connection, params};

use crate::error::{Error, Result};
use crate::vectorstore::{
    CollectionInfo, CollectionSnapshot, DenseVector, Filter, Point, PointId, ScoredPoint,
    SparseVector, VectorStore,
};

/// The single collection the app pipeline uses (name carried over from the
/// qdrant era so settings/xtask output stay recognizable).
pub const VECTOR_COLLECTION: &str = "supportos_vectors";

/// Point-id namespacing (clean-audit item 21): all three chunk families
/// upsert into the single [`VECTOR_COLLECTION`] collection, and their
/// per-table row ids are independent sequences — knowledge chunk 7 and
/// conversation chunk 7 used to land on the same `point_id`, so the
/// upsert's DELETE+INSERT silently overwrote whichever family had written
/// first. Production writes now derive the point id from the family band
/// below, making ids unique across families by construction. The stride
/// bounds each family at one billion chunks — far beyond any real corpus
/// — and a chunk id that would cross into the next band fails loudly
/// instead of colliding.
const POINT_ID_STRIDE: i64 = 1_000_000_000;

#[must_use]
fn chunk_family_band(entity_type: &str) -> Option<i64> {
    match entity_type {
        "knowledge_chunk" => Some(0),
        "docs_chunk" => Some(1),
        "conversation_chunk" => Some(2),
        _ => None,
    }
}

/// Collision-free vector point id for `(entity_type, chunk_id)` — the
/// storage key every chunk-family writer must use for the shared
/// collection (audit item 21).
pub fn namespaced_chunk_point_id(entity_type: &str, chunk_id: i64) -> Result<i64> {
    let band = chunk_family_band(entity_type).ok_or_else(|| {
        Error::Config(format!(
            "vector point ids must be namespaced by a known chunk family \
             (knowledge_chunk/docs_chunk/conversation_chunk); got '{entity_type}'"
        ))
    })?;
    if !(0..POINT_ID_STRIDE).contains(&chunk_id) {
        return Err(Error::Config(format!(
            "chunk id {chunk_id} for entity family '{entity_type}' exceeds the \
             point-id band stride ({POINT_ID_STRIDE}); refusing to map it into a \
             neighbouring family's id space"
        )));
    }
    Ok(band * POINT_ID_STRIDE + chunk_id)
}

/// Directory the sqlite-vec database lives in, for a given data dir.
#[must_use]
pub fn store_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("vectors")
}

/// Register the sqlite-vec extension entry point on this process exactly
/// once; every connection opened afterwards has `vec0` available.
///
/// Mirrors the pattern from the `sqlite-vec` crate's own rusqlite test.
fn register_extension_once() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| unsafe {
        // The extension entry point has the signature SQLite passes to
        // `sqlite3_auto_extension`'s init slot; the transmute is the
        // pattern from the sqlite-vec crate's own rusqlite test, with the
        // target type spelled out.
        let init: unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *mut std::ffi::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::ffi::c_int = std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
        rusqlite::ffi::sqlite3_auto_extension(Some(init));
    });
}

fn open_store_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    register_extension_once();
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS vec_meta (
            collection TEXT PRIMARY KEY,
            dim        INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vec_payload (
            collection   TEXT    NOT NULL,
            point_id     INTEGER NOT NULL,
            entity_type  TEXT,
            entity_id    INTEGER,
            payload_json TEXT    NOT NULL,
            PRIMARY KEY (collection, point_id)
        );
        CREATE INDEX IF NOT EXISTS idx_vec_payload_entity
            ON vec_payload (collection, entity_type, entity_id);
        ",
    )?;
    Ok(conn)
}

fn safe_table_suffix(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "collection".to_string()
    } else {
        safe
    }
}

fn vec_table_for(collection: &str) -> String {
    format!("vec_{}", safe_table_suffix(collection))
}

fn vec_to_blob(vector: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vector.len() * 4);
    for f in vector {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn blob_to_vec(bytes: &[u8]) -> DenseVector {
    let (chunks, _) = bytes.as_chunks::<4>();
    chunks
        .iter()
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// sqlite-vec-backed [`VectorStore`].
pub struct SqliteVecStore {
    conn: Mutex<Connection>,
}

impl SqliteVecStore {
    /// Open (creating if needed) the vector database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(open_store_db(path)?),
        })
    }

    /// Delete every point of `entity_type` whose `entity_id` is listed.
    ///
    /// Facade-level bulk delete (the delete pipeline's shape); a direct
    /// method on the store so it can run both halves in one place.
    pub fn delete_by_entity(
        &self,
        collection: &str,
        entity_type: &str,
        entity_ids: &[i64],
    ) -> Result<()> {
        if entity_ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let table = vec_table_for(collection);
        let tx = conn.transaction()?;
        let placeholders: Vec<String> =
            (3..3 + entity_ids.len()).map(|i| format!("?{i}")).collect();
        let in_list = placeholders.join(", ");
        let mut bound: Vec<String> = vec![collection.to_string(), entity_type.to_string()];
        bound.extend(entity_ids.iter().map(ToString::to_string));
        tx.execute(
            &format!(
                "DELETE FROM {table} WHERE point_id IN (
                     SELECT point_id FROM vec_payload
                      WHERE collection = ?1 AND entity_type = ?2
                        AND entity_id IN ({in_list})
                 )"
            ),
            rusqlite::params_from_iter(bound.iter()),
        )?;
        tx.execute(
            &format!(
                "DELETE FROM vec_payload
                  WHERE collection = ?1 AND entity_type = ?2
                    AND entity_id IN ({in_list})"
            ),
            rusqlite::params_from_iter(bound.iter()),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Open the default store database inside `data_dir`.
    pub fn open_default(data_dir: &Path) -> Result<Self> {
        Self::open(&store_dir(data_dir).join("vectors.db"))
    }

    /// Clean-audit item 60: remove every point of `entity_type` whose point
    /// id is NOT in `live_point_ids` — the orphaned vectors of chunks that
    /// were deleted or re-chunked (a re-chunk hands every chunk a new id, so
    /// the old points keep matching with stale text snapshots forever). The
    /// re-chunk lanes replace chunk rows in bulk and none of them could reach
    /// a store handle, so the sweep runs from the embed pipeline, which
    /// holds both the main database (the live chunk ids) and the store.
    ///
    /// Deletes run in bounded batches (SQLite parameter limits) inside one
    /// transaction. Returns how many points were removed.
    pub fn prune_family_except(
        &self,
        collection: &str,
        entity_type: &str,
        live_point_ids: &[i64],
    ) -> Result<u64> {
        let mut conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let table = vec_table_for(collection);
        let all_point_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT point_id FROM vec_payload
                  WHERE collection = ?1 AND entity_type = ?2",
            )?;
            stmt.query_map(params![collection, entity_type], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        };
        let live: std::collections::HashSet<i64> = live_point_ids.iter().copied().collect();
        let orphans: Vec<i64> = all_point_ids
            .into_iter()
            .filter(|point_id| !live.contains(point_id))
            .collect();
        if orphans.is_empty() {
            return Ok(0);
        }
        let tx = conn.transaction()?;
        let mut removed = 0u64;
        for batch in orphans.chunks(500) {
            let placeholders: Vec<String> = batch.iter().map(|_| "?".to_string()).collect();
            let in_list = placeholders.join(", ");
            tx.execute(
                &format!("DELETE FROM {table} WHERE point_id IN ({in_list})"),
                rusqlite::params_from_iter(batch.iter()),
            )?;
            let mut payload_bound = vec![collection.to_string()];
            payload_bound.extend(batch.iter().map(|id| id.to_string()));
            tx.execute(
                &format!(
                    "DELETE FROM vec_payload
                      WHERE collection = ?1 AND point_id IN ({in_list})"
                ),
                rusqlite::params_from_iter(payload_bound.iter()),
            )?;
            removed += batch.len() as u64;
        }
        tx.commit()?;
        Ok(removed)
    }

    /// Purge the vector store of a data directory: drop every vec0
    /// collection table and clear the payload/metadata bookkeeping
    /// (the demo-data purge, 1.1.3). Safe on a missing store (creates
    /// an empty one) and safe while a worker holds the store open —
    /// both connections see the same WAL file, and the pipeline
    /// re-creates collections and re-embeds chunks on demand.
    ///
    /// Returns how many entries were removed (collection tables plus
    /// payload plus metadata rows).
    pub fn purge_all(data_dir: &Path) -> Result<u64> {
        let conn = open_store_db(&store_dir(data_dir).join("vectors.db"))?;
        let collections: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND sql LIKE '%USING vec0%'",
            )?;
            stmt.query_map([], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect()
        };
        let mut removed = collections.len() as u64;
        for table in &collections {
            conn.execute(&format!("DROP TABLE IF EXISTS \"{table}\""), [])?;
        }
        removed += u64::try_from(conn.execute("DELETE FROM vec_payload", [])?).unwrap_or(0);
        removed += u64::try_from(conn.execute("DELETE FROM vec_meta", [])?).unwrap_or(0);
        Ok(removed)
    }

    fn dim_for(&self, collection: &str) -> Result<Option<usize>> {
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let dim: Option<i64> = conn
            .query_row(
                "SELECT dim FROM vec_meta WHERE collection = ?1",
                params![collection],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        Ok(dim.map(|d| usize::try_from(d).unwrap_or(0)))
    }

    fn create_vec_table(&self, collection: &str, dim: usize) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let table = vec_table_for(collection);
        conn.execute_batch(&format!(
            "DROP TABLE IF EXISTS {table};
             CREATE VIRTUAL TABLE {table} USING vec0(
                 point_id  INTEGER PRIMARY KEY,
                 embedding FLOAT[{dim}] distance_metric=cosine
             );"
        ))?;
        conn.execute(
            "INSERT INTO vec_meta (collection, dim) VALUES (?1, ?2)
             ON CONFLICT(collection) DO UPDATE SET dim = excluded.dim",
            params![collection, i64::try_from(dim).unwrap_or(i64::MAX)],
        )?;
        Ok(())
    }

    /// Exact point count of `collection`; `None` when the collection has
    /// never been created.
    pub fn try_point_count(&self, collection: &str) -> Result<Option<usize>> {
        if self.dim_for(collection)?.is_none() {
            return Ok(None);
        }
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vec_payload WHERE collection = ?1",
            params![collection],
            |r| r.get(0),
        )?;
        Ok(Some(usize::try_from(count).unwrap_or(0)))
    }

    /// Points per `entity_type` (the xtask D4 breakdown and the delete
    /// pipeline's bookkeeping).
    pub fn count_by_entity_type(&self, collection: &str, entity_type: &str) -> Result<usize> {
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vec_payload WHERE collection = ?1 AND entity_type = ?2",
            params![collection, entity_type],
            |r| r.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    fn payload_of(&self, collection: &str, point_id: &PointId) -> Result<serde_json::Value> {
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let json: Option<String> = conn
            .query_row(
                "SELECT payload_json FROM vec_payload WHERE collection = ?1 AND point_id = ?2",
                params![collection, point_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        Ok(json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or(serde_json::Value::Null))
    }
}

fn numeric_point_id(id: &PointId) -> Result<i64> {
    id.parse::<i64>().map_err(|_| {
        Error::Config(format!(
            "sqlite-vec store requires numeric point ids; got '{id}'"
        ))
    })
}

impl VectorStore for SqliteVecStore {
    fn create_collection(&self, name: &str, dense_dim: Option<usize>) -> Result<()> {
        let Some(dim) = dense_dim else {
            return Err(Error::Config(
                "sqlite-vec store requires a fixed dense dimension".to_string(),
            ));
        };
        match self.dim_for(name)? {
            Some(existing) if existing == dim => Ok(()),
            Some(existing) => {
                // Dimension change = model change: rebuild the collection
                // (chunks re-embed through the normal pending pipeline).
                tracing::warn!(
                    collection = name,
                    old_dim = existing,
                    new_dim = dim,
                    "dimension change — rebuilding the vector collection"
                );
                let _ = self.drop_collection(name);
                self.create_vec_table(name, dim)
            }
            None => self.create_vec_table(name, dim),
        }
    }

    fn drop_collection(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {};", vec_table_for(name)))?;
        conn.execute(
            "DELETE FROM vec_payload WHERE collection = ?1",
            params![name],
        )?;
        conn.execute("DELETE FROM vec_meta WHERE collection = ?1", params![name])?;
        Ok(())
    }

    fn upsert(&self, collection: &str, point: Point) -> Result<()> {
        let dim = match self.dim_for(collection)? {
            Some(d) => d,
            None => {
                // Auto-create on first upsert (facade parity with the
                // qdrant path).
                let d = point.dense.as_ref().map(Vec::len).ok_or_else(|| {
                    Error::Config("sqlite-vec upsert requires a dense vector".to_string())
                })?;
                self.create_vec_table(collection, d)?;
                d
            }
        };
        let Some(dense) = point.dense else {
            return Err(Error::Config(
                "sqlite-vec upsert requires a dense vector".to_string(),
            ));
        };
        if dense.len() != dim {
            return Err(Error::Config(format!(
                "sqlite-vec dimension mismatch: collection '{collection}' is {dim}, vector is {}",
                dense.len()
            )));
        }
        let id = numeric_point_id(&point.id)?;
        let entity_type = point
            .payload
            .get("entity_type")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let entity_id = point.payload.get("entity_id").and_then(|v| v.as_i64());
        let payload_json = serde_json::to_string(&point.payload)
            .map_err(|e| Error::Config(format!("payload serialization failed: {e}")))?;

        let mut conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let tx = conn.transaction()?;
        let table = vec_table_for(collection);
        // Audit item 21 guard: refuse the silent cross-family overwrite.
        // With namespaced point ids this never fires; a future writer that
        // goes back to raw per-table ids gets a loud failure instead of
        // quiet data loss. Rows without an entity_type cannot be compared
        // and keep the old replace semantics.
        if let Some(new_family) = entity_type.as_ref() {
            let stored_family: Option<String> = tx
                .query_row(
                    "SELECT entity_type FROM vec_payload
                      WHERE collection = ?1 AND point_id = ?2",
                    params![collection, id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten();
            if let Some(old_family) = stored_family
                && old_family != *new_family
            {
                return Err(Error::Config(format!(
                    "vector point id {id} in collection '{collection}' already belongs \
                     to entity family '{old_family}'; refusing to overwrite it with \
                     '{new_family}' (cross-family point-id collision)"
                )));
            }
        }
        tx.execute(
            &format!("DELETE FROM {table} WHERE point_id = ?1"),
            params![id],
        )?;
        tx.execute(
            &format!("INSERT INTO {table} (point_id, embedding) VALUES (?1, ?2)"),
            params![id, vec_to_blob(&dense)],
        )?;
        tx.execute(
            "INSERT INTO vec_payload (collection, point_id, entity_type, entity_id, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(collection, point_id) DO UPDATE SET
                entity_type  = excluded.entity_type,
                entity_id    = excluded.entity_id,
                payload_json = excluded.payload_json",
            params![collection, id, entity_type, entity_id, payload_json],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn delete(&self, collection: &str, id: &PointId) -> Result<()> {
        let id = numeric_point_id(id)?;
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE point_id = ?1",
                vec_table_for(collection)
            ),
            params![id],
        )?;
        conn.execute(
            "DELETE FROM vec_payload WHERE collection = ?1 AND point_id = ?2",
            params![collection, id],
        )?;
        Ok(())
    }

    fn search_dense(
        &self,
        collection: &str,
        query: &[f32],
        filter: Option<&Filter>,
        top_k: usize,
    ) -> Result<Vec<ScoredPoint>> {
        if self.dim_for(collection)?.is_none() || query.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }
        // Over-fetch so post-filtering on payload fields can still fill
        // `top_k` slots (the KNN itself cannot see JSON payload fields).
        let overfetch = top_k.saturating_mul(8).max(top_k);
        let raw: Vec<(i64, f32)> = {
            let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
            let mut stmt = conn.prepare(&format!(
                "SELECT point_id, distance FROM {} WHERE embedding MATCH ?1 AND k = ?2",
                vec_table_for(collection)
            ))?;
            stmt.query_map(
                params![
                    vec_to_blob(query),
                    i64::try_from(overfetch).unwrap_or(i64::MAX)
                ],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f32>(1)?)),
            )?
            .filter_map(|row| row.ok())
            .collect()
        };

        let mut out = Vec::with_capacity(top_k);
        for (id, distance) in raw {
            let payload = self.payload_of(collection, &id.to_string())?;
            if let Some(f) = filter
                && !f.matches(&payload)
            {
                continue;
            }
            // cosine distance -> similarity, matching the qdrant cosine
            // score direction (higher = closer).
            out.push(ScoredPoint {
                id: id.to_string(),
                score: 1.0 - distance,
                payload,
            });
            if out.len() == top_k {
                break;
            }
        }
        Ok(out)
    }

    fn search_sparse(
        &self,
        _collection: &str,
        _query: &SparseVector,
        _filter: Option<&Filter>,
        _top_k: usize,
    ) -> Result<Vec<ScoredPoint>> {
        // The sparse lane of hybrid search runs through FTS5; no points are
        // ever written with sparse vectors. Pinned by test.
        Ok(Vec::new())
    }

    fn count(&self, collection: &str, filter: Option<&Filter>) -> Result<usize> {
        let Some(filter) = filter else {
            return self.try_point_count(collection).map(|c| c.unwrap_or(0));
        };
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT payload_json FROM vec_payload WHERE collection = ?1")?;
        let rows: Vec<String> = stmt
            .query_map(params![collection], |r| r.get(0))?
            .filter_map(|row| row.ok())
            .collect();
        drop(stmt);
        drop(conn);
        Ok(rows
            .iter()
            .filter_map(|j| serde_json::from_str::<serde_json::Value>(j).ok())
            .filter(|payload| filter.matches(payload))
            .count())
    }

    fn collection_info(&self, name: &str) -> Result<Option<CollectionInfo>> {
        let Some(dim) = self.dim_for(name)? else {
            return Ok(None);
        };
        let count = self.try_point_count(name)?.unwrap_or(0);
        Ok(Some(CollectionInfo {
            name: name.to_string(),
            dense_dim: Some(dim),
            point_count: count,
        }))
    }

    fn snapshot(&self, name: &str) -> Result<Vec<u8>> {
        let Some(dim) = self.dim_for(name)? else {
            return Err(Error::Config(format!("collection '{name}' does not exist")));
        };
        let conn = self.conn.lock().expect("sqlite-vec store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT v.point_id, v.embedding, p.payload_json
               FROM {} v LEFT JOIN vec_payload p
                 ON p.collection = ?1 AND p.point_id = v.point_id",
            vec_table_for(name)
        ))?;
        let rows: Vec<(i64, Option<Vec<u8>>, Option<String>)> = stmt
            .query_map(params![name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|row| row.ok())
            .collect();
        drop(stmt);
        drop(conn);

        // Same wire format as the qdrant adapter snapshots, so backups
        // stay interoperable across the swap.
        let points: Vec<Point> = rows
            .into_iter()
            .map(|(id, embedding, payload_json)| Point {
                id: id.to_string(),
                dense: embedding.map(|blob| blob_to_vec(&blob)),
                sparse: None,
                payload: payload_json
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect();
        let snapshot = CollectionSnapshot {
            name: name.to_string(),
            dense_dim: Some(dim),
            points,
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

/// Point shape the worker pipeline upserts (id, dense vector, JSON
/// payload) — carried over unchanged from the qdrant facade.
#[derive(Debug, Clone)]
pub struct VectorPoint {
    pub id: i64,
    pub vector: Vec<f32>,
    pub payload: serde_json::Value,
}

/// Search result the worker pipeline consumes.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: i64,
    pub score: f32,
    pub payload: serde_json::Value,
}

/// Store health for the settings/health surfaces.
#[derive(Debug, Clone)]
pub struct Health {
    pub connected: bool,
    /// The sqlite-vec database path (the qdrant URL slot).
    pub url: String,
    pub collections: Vec<String>,
    pub error: Option<String>,
}

/// App-facing vector index facade over [`SqliteVecStore`] — the same
/// surface the old `EmbeddedQdrant` facade offered (lazy init, enable
/// flag, boolean return codes), now backed by sqlite-vec per D4.
pub struct VectorIndex {
    data_dir: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    enabled: bool,
    store: Option<Arc<SqliteVecStore>>,
    last_error: Option<String>,
}

impl VectorIndex {
    #[must_use]
    pub fn new(data_dir: impl Into<PathBuf>, enabled: bool) -> Self {
        Self {
            data_dir: data_dir.into(),
            inner: Mutex::new(Inner {
                enabled,
                store: None,
                last_error: None,
            }),
        }
    }

    /// Toggle the index (the settings `qdrant_enabled` slot).
    pub fn reconfigure(&self, enabled: Option<bool>) {
        let mut inner = self.inner.lock().expect("VectorIndex mutex poisoned");
        if let Some(e) = enabled {
            inner.enabled = e;
        }
    }

    /// Lazily open (or reuse) the backing store handle.
    fn store_handle(&self) -> Option<Arc<SqliteVecStore>> {
        let mut inner = self.inner.lock().expect("VectorIndex mutex poisoned");
        if !inner.enabled {
            return None;
        }
        if inner.store.is_none() {
            match SqliteVecStore::open_default(&self.data_dir) {
                Ok(store) => {
                    inner.store = Some(Arc::new(store));
                    inner.last_error = None;
                }
                Err(e) => {
                    inner.last_error = Some(format!("sqlite-vec storage open failed: {e}"));
                    return None;
                }
            }
        }
        inner.store.as_ref().map(Arc::clone)
    }

    /// Store health for the settings/health surfaces.
    pub fn health(&self) -> Health {
        let url = store_dir(&self.data_dir)
            .join("vectors.db")
            .to_string_lossy()
            .into_owned();
        if !self.enabled() {
            return Health {
                connected: false,
                url,
                collections: Vec::new(),
                error: Some("Vector index disabled in settings".to_string()),
            };
        }
        let Some(store) = self.store_handle() else {
            let inner = self.inner.lock().expect("VectorIndex mutex poisoned");
            return Health {
                connected: false,
                url,
                collections: Vec::new(),
                error: Some(
                    inner
                        .last_error
                        .clone()
                        .unwrap_or_else(|| "sqlite-vec storage unavailable".to_string()),
                ),
            };
        };
        let collections = match store.collection_info(VECTOR_COLLECTION) {
            Ok(Some(_)) => vec![VECTOR_COLLECTION.to_string()],
            _ => Vec::new(),
        };
        Health {
            connected: true,
            url,
            collections,
            error: None,
        }
    }

    /// Exact point count of the store's collection; `Ok(None)` when the
    /// collection does not exist yet, an error when the store cannot be
    /// opened.
    pub fn try_point_count(&self) -> Result<Option<usize>> {
        let Some(store) = self.store_handle() else {
            return Err(Error::Config(
                self.inner
                    .lock()
                    .expect("VectorIndex mutex poisoned")
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "vector index disabled".to_string()),
            ));
        };
        store.try_point_count(VECTOR_COLLECTION)
    }

    /// Ensure the collection exists at `dimension`, rebuilding it when the
    /// dimension changed (model swap → re-embed via the pending pipeline).
    pub fn ensure_collection(&self, dimension: usize) -> bool {
        let Some(store) = self.store_handle() else {
            return false;
        };
        store
            .create_collection(VECTOR_COLLECTION, Some(dimension))
            .is_ok()
    }

    pub fn upsert(&self, points: &[VectorPoint]) -> bool {
        if points.is_empty() {
            return false;
        }
        let Some(store) = self.store_handle() else {
            return false;
        };
        for p in points {
            let point = Point {
                id: p.id.to_string(),
                dense: Some(p.vector.clone()),
                sparse: None,
                payload: p.payload.clone(),
            };
            if store.upsert(VECTOR_COLLECTION, point).is_err() {
                return false;
            }
        }
        true
    }

    pub fn search(&self, vector: &[f32], limit: usize) -> Vec<SearchHit> {
        let Some(store) = self.store_handle() else {
            return Vec::new();
        };
        match store.search_dense(VECTOR_COLLECTION, vector, None, limit) {
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

    /// Delete every point of `entity_type` matching one of `entity_ids`.
    pub fn delete_by_entity(&self, entity_type: &str, entity_ids: &[i64]) -> bool {
        if entity_ids.is_empty() {
            return false;
        }
        let Some(store) = self.store_handle() else {
            return false;
        };
        store
            .delete_by_entity(VECTOR_COLLECTION, entity_type, entity_ids)
            .is_ok()
    }

    /// Clean-audit item 60: remove this `entity_type` family's points whose
    /// ids are not in `live_point_ids` — orphaned vectors of deleted or
    /// re-chunked rows (whose payloads still carry the stale text snapshot
    /// the semantic lanes serve). Returns the number of points removed, or
    /// `None` when the store is disabled or cannot be opened.
    pub fn prune_orphaned_points(&self, entity_type: &str, live_point_ids: &[i64]) -> Option<u64> {
        let store = self.store_handle()?;
        store
            .prune_family_except(VECTOR_COLLECTION, entity_type, live_point_ids)
            .ok()
    }

    pub fn count_by_entity(&self, entity_type: &str) -> usize {
        let Some(store) = self.store_handle() else {
            return 0;
        };
        store
            .count_by_entity_type(VECTOR_COLLECTION, entity_type)
            .unwrap_or(0)
    }

    pub fn drop_collection(&self) -> bool {
        let Some(store) = self.store_handle() else {
            return false;
        };
        store.drop_collection(VECTOR_COLLECTION).is_ok()
    }

    pub fn enabled(&self) -> bool {
        self.inner
            .lock()
            .expect("VectorIndex mutex poisoned")
            .enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorstore::Filter;

    fn temp_data_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-sqlitevec-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn point(id: i64, vector: Vec<f32>, entity_type: &str, entity_id: i64) -> Point {
        Point {
            id: id.to_string(),
            dense: Some(vector),
            sparse: None,
            payload: serde_json::json!({
                "entity_type": entity_type,
                "entity_id": entity_id,
                "text": format!("chunk {id}"),
            }),
        }
    }

    fn seeded_store(tag: &str, dim: usize) -> (PathBuf, SqliteVecStore) {
        let dir = temp_data_dir(tag);
        let store = SqliteVecStore::open_default(&dir).expect("open store");
        store
            .create_collection(VECTOR_COLLECTION, Some(dim))
            .expect("create collection");
        (dir, store)
    }

    #[test]
    fn vec0_extension_is_available_and_versioned() {
        let dir = temp_data_dir("version");
        let store = SqliteVecStore::open_default(&dir).expect("open store");
        let version: String = store
            .conn
            .lock()
            .expect("mutex")
            .query_row("SELECT vec_version()", [], |r| r.get(0))
            .expect("vec_version()");
        assert!(
            version.starts_with('v'),
            "unexpected vec_version: {version}"
        );
    }

    #[test]
    fn create_count_upsert_delete_roundtrip() {
        let (_dir, store) = seeded_store("roundtrip", 2);
        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).expect("count"),
            Some(0)
        );

        store
            .upsert(
                VECTOR_COLLECTION,
                point(1, vec![1.0, 0.0], "knowledge_chunk", 10),
            )
            .expect("upsert 1");
        store
            .upsert(
                VECTOR_COLLECTION,
                point(2, vec![0.0, 1.0], "docs_chunk", 20),
            )
            .expect("upsert 2");
        // Same id twice = replace, not duplicate.
        store
            .upsert(
                VECTOR_COLLECTION,
                point(2, vec![0.0, 1.0], "docs_chunk", 20),
            )
            .expect("upsert 2 again");

        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).expect("count"),
            Some(2)
        );
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "knowledge_chunk")
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "docs_chunk")
                .unwrap(),
            1
        );

        store
            .delete(VECTOR_COLLECTION, &"1".to_string())
            .expect("delete");
        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).expect("count"),
            Some(1)
        );
    }

    #[test]
    fn dimension_mismatch_is_an_error_and_rebuild_is_explicit() {
        let (_dir, store) = seeded_store("dims", 2);
        let err = store
            .upsert(VECTOR_COLLECTION, point(1, vec![1.0, 2.0, 3.0], "x", 1))
            .expect_err("3-dim vector into 2-dim collection must fail");
        assert!(err.to_string().contains("dimension"));

        // ensure-collection with a new dimension rebuilds cleanly.
        store
            .create_collection(VECTOR_COLLECTION, Some(3))
            .expect("rebuild");
        store
            .upsert(VECTOR_COLLECTION, point(1, vec![1.0, 2.0, 3.0], "x", 1))
            .expect("3-dim upsert after rebuild");
        assert_eq!(store.try_point_count(VECTOR_COLLECTION).unwrap(), Some(1));
    }

    #[test]
    fn knn_search_orders_by_cosine_similarity() {
        let (_dir, store) = seeded_store("knn", 2);
        store
            .upsert(VECTOR_COLLECTION, point(1, vec![1.0, 0.0], "a", 1))
            .expect("upsert");
        store
            .upsert(VECTOR_COLLECTION, point(2, vec![0.9, 0.1], "a", 2))
            .expect("upsert");
        store
            .upsert(VECTOR_COLLECTION, point(3, vec![0.0, 1.0], "a", 3))
            .expect("upsert");

        let hits = store
            .search_dense(VECTOR_COLLECTION, &[1.0, 0.0], None, 3)
            .expect("search");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].id, "1", "identical vector must rank first");
        assert_eq!(hits[1].id, "2");
        assert_eq!(hits[2].id, "3");
        assert!(hits[0].score > hits[1].score, "scores must be similarities");
    }

    #[test]
    fn search_filter_post_filters_on_payload() {
        let (_dir, store) = seeded_store("filter", 2);
        store
            .upsert(
                VECTOR_COLLECTION,
                point(1, vec![1.0, 0.0], "knowledge_chunk", 10),
            )
            .expect("upsert");
        store
            .upsert(
                VECTOR_COLLECTION,
                point(2, vec![0.9, 0.1], "docs_chunk", 20),
            )
            .expect("upsert");

        let filter = Filter::new().must_eq("entity_type", "docs_chunk");
        let hits = store
            .search_dense(VECTOR_COLLECTION, &[1.0, 0.0], Some(&filter), 5)
            .expect("filtered search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "2");
    }

    #[test]
    fn sparse_search_is_empty_by_design() {
        let (_dir, store) = seeded_store("sparse", 2);
        let hits = store
            .search_sparse(
                VECTOR_COLLECTION,
                &crate::vectorstore::SparseVector::new(vec![0], vec![1.0]),
                None,
                5,
            )
            .expect("sparse search");
        assert!(hits.is_empty());
    }

    #[test]
    fn snapshot_and_restore_roundtrip() {
        let dir_a = temp_data_dir("snap-a");
        let dir_b = temp_data_dir("snap-b");
        let a = SqliteVecStore::open_default(&dir_a).expect("open a");
        a.create_collection(VECTOR_COLLECTION, Some(2))
            .expect("create");
        a.upsert(VECTOR_COLLECTION, point(1, vec![1.0, 0.0], "k", 1))
            .expect("upsert");
        a.upsert(VECTOR_COLLECTION, point(2, vec![0.0, 1.0], "k", 2))
            .expect("upsert");

        let bytes = a.snapshot(VECTOR_COLLECTION).expect("snapshot");
        let b = SqliteVecStore::open_default(&dir_b).expect("open b");
        b.restore(&bytes).expect("restore");
        assert_eq!(b.try_point_count(VECTOR_COLLECTION).unwrap(), Some(2));
        let hits = b
            .search_dense(VECTOR_COLLECTION, &[1.0, 0.0], None, 1)
            .expect("search restored");
        assert_eq!(hits[0].id, "1");
        assert_eq!(hits[0].payload["entity_type"], "k");
    }

    #[test]
    fn delete_by_entity_removes_both_halves() {
        let (_dir, store) = seeded_store("delentity", 2);
        store
            .upsert(
                VECTOR_COLLECTION,
                point(1, vec![1.0, 0.0], "conversation_chunk", 5),
            )
            .expect("upsert 1");
        store
            .upsert(
                VECTOR_COLLECTION,
                point(2, vec![0.0, 1.0], "conversation_chunk", 6),
            )
            .expect("upsert 2");
        store
            .upsert(
                VECTOR_COLLECTION,
                point(3, vec![0.5, 0.5], "knowledge_chunk", 6),
            )
            .expect("upsert 3");

        store
            .delete_by_entity(VECTOR_COLLECTION, "conversation_chunk", &[5, 6])
            .expect("delete by entity");

        assert_eq!(store.try_point_count(VECTOR_COLLECTION).unwrap(), Some(1));
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "conversation_chunk")
                .unwrap(),
            0
        );
        let remaining = store
            .search_dense(VECTOR_COLLECTION, &[0.5, 0.5], None, 5)
            .expect("search");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].payload["entity_type"], "knowledge_chunk");
    }

    #[test]
    fn count_with_generic_filter_scans_payloads() {
        let (_dir, store) = seeded_store("countfilter", 2);
        store
            .upsert(VECTOR_COLLECTION, point(1, vec![1.0, 0.0], "k", 1))
            .expect("upsert");
        store
            .upsert(VECTOR_COLLECTION, point(2, vec![0.0, 1.0], "d", 2))
            .expect("upsert");

        let all = store.count(VECTOR_COLLECTION, None).expect("count all");
        assert_eq!(all, 2);
        let filtered = store
            .count(
                VECTOR_COLLECTION,
                Some(&Filter::new().must_eq("entity_type", "d")),
            )
            .expect("count filtered");
        assert_eq!(filtered, 1);
    }

    // --- facade ---

    #[test]
    fn facade_disabled_and_enabled_states() {
        let dir = temp_data_dir("facade");
        let index = VectorIndex::new(&dir, false);
        assert!(!index.enabled());
        let health = index.health();
        assert!(!health.connected);
        assert!(health.error.is_some());
        assert!(index.search(&[1.0, 0.0], 5).is_empty());
        assert!(!index.upsert(&[VectorPoint {
            id: 1,
            vector: vec![1.0, 0.0],
            payload: serde_json::json!({"entity_type": "k"}),
        }]));
        assert_eq!(index.count_by_entity("k"), 0);

        index.reconfigure(Some(true));
        assert!(index.enabled());
        let health = index.health();
        assert!(health.connected, "sqlite-vec is always local: {health:?}");
        assert!(health.collections.is_empty(), "no collection yet");
        assert!(health.url.contains("vectors.db"));
    }

    #[test]
    fn facade_upsert_search_and_counts() {
        let dir = temp_data_dir("facade2");
        let index = VectorIndex::new(&dir, true);
        let points = vec![
            VectorPoint {
                id: 1,
                vector: vec![1.0, 0.0],
                payload: serde_json::json!({"entity_type": "knowledge_chunk", "entity_id": 10}),
            },
            VectorPoint {
                id: 2,
                vector: vec![0.0, 1.0],
                payload: serde_json::json!({"entity_type": "docs_chunk", "entity_id": 20}),
            },
        ];
        assert!(index.ensure_collection(2));
        assert!(index.upsert(&points));

        assert_eq!(index.try_point_count().expect("count"), Some(2));
        assert_eq!(index.count_by_entity("knowledge_chunk"), 1);
        assert_eq!(index.count_by_entity("docs_chunk"), 1);

        let hits = index.search(&[1.0, 0.0], 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, 1);

        assert!(index.delete_by_entity("docs_chunk", &[20]));
        assert_eq!(index.try_point_count().expect("count"), Some(1));

        assert!(index.drop_collection());
        assert_eq!(index.try_point_count().expect("count"), None);
    }

    // --- clean-audit item 21: point-id namespacing + collision guard ---

    /// Audit item 21: knowledge chunk 7 and conversation chunk 7 are two
    /// different vectors that used to land on the same point id (raw
    /// per-table row ids in one shared collection) — the second upsert
    /// silently overwrote the first. Namespaced ids keep both.
    /// Clean-audit item 60: prune_family_except removes a family's points
    /// whose ids are not in the live set — the orphaned vectors of deleted
    /// or re-chunked rows — while leaving other families and live points
    /// untouched.
    #[test]
    fn audit60_prune_family_except_removes_only_orphans_of_that_family() {
        let (_dir, store) = seeded_store("prune60", 2);
        // knowledge_chunk family: point ids are band|chunk-id (band 0, so
        // point id == chunk id here). Chunks 1 and 2 belong to document
        // 100, chunk 3 to document 101.
        for (point_id, entity_id) in [(1, 100), (2, 100), (3, 101)] {
            store
                .upsert(
                    VECTOR_COLLECTION,
                    point(point_id, vec![1.0, 0.0], "knowledge_chunk", entity_id),
                )
                .expect("upsert knowledge");
        }
        // docs_chunk family (band 1) shares the collection.
        store
            .upsert(
                VECTOR_COLLECTION,
                point(1_000_000_001, vec![0.0, 1.0], "docs_chunk", 200),
            )
            .expect("upsert docs");

        // Document 100 was re-chunked: its old chunk points (1 and 2) are
        // orphaned; only chunk 3's point (id 3) is still live.
        let removed = store
            .prune_family_except(VECTOR_COLLECTION, "knowledge_chunk", &[3])
            .expect("prune");
        assert_eq!(removed, 2, "the two orphaned knowledge points are gone");

        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).unwrap(),
            Some(2),
            "the surviving knowledge point and the docs point remain"
        );
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "knowledge_chunk")
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "docs_chunk")
                .unwrap(),
            1,
            "other families are never touched by the sweep"
        );
        // Idempotent: a second sweep with the same live set removes nothing.
        let removed = store
            .prune_family_except(VECTOR_COLLECTION, "knowledge_chunk", &[3])
            .expect("prune again");
        assert_eq!(removed, 0);
        // An empty live set wipes the whole family.
        let removed = store
            .prune_family_except(VECTOR_COLLECTION, "knowledge_chunk", &[])
            .expect("prune all");
        assert_eq!(removed, 1);
        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).unwrap(),
            Some(1),
            "only the docs point is left"
        );
    }

    /// Item 60, facade level: the VectorIndex wrapper reports the removal
    /// and stays silent (None) only when the store cannot be opened.
    #[test]
    fn audit60_vector_index_prune_facade_removes_orphans() {
        let dir = temp_data_dir("prune-facade");
        let index = VectorIndex::new(&dir, true);
        index.ensure_collection(2);
        assert!(
            index.upsert(&[
                VectorPoint {
                    id: namespaced_chunk_point_id("knowledge_chunk", 7).unwrap(),
                    vector: vec![1.0, 0.0],
                    payload: serde_json::json!({
                        "entity_type": "knowledge_chunk",
                        "entity_id": 1,
                        "chunk_id": 7,
                        "text": "stale text",
                    }),
                },
                VectorPoint {
                    id: namespaced_chunk_point_id("knowledge_chunk", 9).unwrap(),
                    vector: vec![0.0, 1.0],
                    payload: serde_json::json!({
                        "entity_type": "knowledge_chunk",
                        "entity_id": 1,
                        "chunk_id": 9,
                        "text": "live text",
                    }),
                },
            ]),
            "upsert both points"
        );
        assert_eq!(index.count_by_entity("knowledge_chunk"), 2);

        // Chunk 7 was deleted/re-chunked away; only chunk 9 is live.
        let live = [namespaced_chunk_point_id("knowledge_chunk", 9).unwrap()];
        let removed = index
            .prune_orphaned_points("knowledge_chunk", &live)
            .expect("the enabled store sweeps");
        assert_eq!(removed, 1);
        assert_eq!(index.count_by_entity("knowledge_chunk"), 1);

        // A disabled index reports nothing and removes nothing.
        let disabled = VectorIndex::new(&dir, false);
        let live = [namespaced_chunk_point_id("knowledge_chunk", 9).unwrap()];
        assert!(
            disabled
                .prune_orphaned_points("knowledge_chunk", &live)
                .is_none(),
            "the disabled store is reported as unavailable"
        );
    }

    #[test]
    fn audit21_namespaced_point_ids_keep_chunk_families_apart() {
        let (_dir, store) = seeded_store("audit21-ns", 2);
        let knowledge_id = namespaced_chunk_point_id("knowledge_chunk", 7).expect("namespaced id");
        let conversation_id =
            namespaced_chunk_point_id("conversation_chunk", 7).expect("namespaced id");
        assert_ne!(knowledge_id, conversation_id);

        store
            .upsert(
                VECTOR_COLLECTION,
                point(knowledge_id, vec![1.0, 0.0], "knowledge_chunk", 3),
            )
            .expect("knowledge upsert");
        store
            .upsert(
                VECTOR_COLLECTION,
                point(conversation_id, vec![0.0, 1.0], "conversation_chunk", 7),
            )
            .expect("conversation upsert");

        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).expect("count"),
            Some(2),
            "both families' chunk 7 must coexist"
        );
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "knowledge_chunk")
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .count_by_entity_type(VECTOR_COLLECTION, "conversation_chunk")
                .unwrap(),
            1
        );
        // Each id retrieves its own payload, not its neighbour's.
        assert_eq!(
            store
                .payload_of(VECTOR_COLLECTION, &knowledge_id.to_string())
                .expect("payload")["entity_type"],
            "knowledge_chunk"
        );
        assert_eq!(
            store
                .payload_of(VECTOR_COLLECTION, &conversation_id.to_string())
                .expect("payload")["entity_type"],
            "conversation_chunk"
        );
    }

    /// Audit item 21 guard: even if a future writer goes back to raw ids,
    /// the store refuses the silent cross-family overwrite instead of
    /// quietly destroying the earlier family's vector.
    #[test]
    fn audit21_cross_family_point_id_overwrite_is_refused() {
        let (_dir, store) = seeded_store("audit21-guard", 2);
        store
            .upsert(
                VECTOR_COLLECTION,
                point(1, vec![1.0, 0.0], "knowledge_chunk", 10),
            )
            .expect("first family upsert");

        let err = store
            .upsert(
                VECTOR_COLLECTION,
                point(1, vec![0.0, 1.0], "conversation_chunk", 10),
            )
            .expect_err("cross-family overwrite must be refused");
        assert!(
            err.to_string().contains("cross-family point-id collision"),
            "the refusal must be legible: {err}"
        );

        // Same-family re-upsert still replaces (the legitimate case).
        store
            .upsert(
                VECTOR_COLLECTION,
                point(1, vec![0.5, 0.5], "knowledge_chunk", 11),
            )
            .expect("same family replace");
        assert_eq!(
            store.try_point_count(VECTOR_COLLECTION).expect("count"),
            Some(1)
        );
        assert_eq!(
            store
                .payload_of(VECTOR_COLLECTION, &"1".to_string())
                .expect("payload")["entity_id"],
            11
        );
    }

    /// Audit item 21: unknown families and band-overflowing chunk ids fail
    /// loudly at mapping time — never a silent id alias.
    #[test]
    fn audit21_unknown_family_and_band_overflow_are_loud() {
        assert!(namespaced_chunk_point_id("thread", 1).is_err());
        assert!(namespaced_chunk_point_id("", 1).is_err());
        assert!(namespaced_chunk_point_id("knowledge_chunk", 1_000_000_000).is_err());
        // In-band ids of every family map deterministically.
        assert_eq!(namespaced_chunk_point_id("knowledge_chunk", 0).unwrap(), 0);
        assert_eq!(
            namespaced_chunk_point_id("docs_chunk", 5).unwrap(),
            1_000_000_005
        );
        assert_eq!(
            namespaced_chunk_point_id("conversation_chunk", 5).unwrap(),
            2_000_000_005
        );
    }
}

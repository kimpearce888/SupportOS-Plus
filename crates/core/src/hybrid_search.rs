use rusqlite::Connection;

use crate::error::Result;
use crate::search;
use crate::vectorstore::{Filter, ScoredPoint, VectorStore};

pub const RRF_K: u32 = 60;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HybridResult {
    pub id: String,
    pub rrf_score: f64,
    pub dense_score: Option<f32>,
    pub sparse_score: Option<f32>,
    pub payload: serde_json::Value,
}

#[must_use]
pub fn fuse_results(
    dense_results: &[ScoredPoint],
    sparse_results: &[ScoredPoint],
    top_k: usize,
) -> Vec<HybridResult> {
    fuse_results_with_k(dense_results, sparse_results, top_k, RRF_K)
}

#[must_use]
pub fn fuse_results_with_k(
    dense_results: &[ScoredPoint],
    sparse_results: &[ScoredPoint],
    top_k: usize,
    k: u32,
) -> Vec<HybridResult> {
    let mut map: std::collections::HashMap<String, HybridResult> = std::collections::HashMap::new();

    for (rank, point) in dense_results.iter().enumerate() {
        let rrf = 1.0 / (f64::from(k) + (rank + 1) as f64);
        let entry = map.entry(point.id.clone()).or_insert_with(|| HybridResult {
            id: point.id.clone(),
            rrf_score: 0.0,
            dense_score: None,
            sparse_score: None,
            payload: point.payload.clone(),
        });
        entry.rrf_score += rrf;
        entry.dense_score = Some(point.score);
    }

    for (rank, point) in sparse_results.iter().enumerate() {
        let rrf = 1.0 / (f64::from(k) + (rank + 1) as f64);
        let entry = map.entry(point.id.clone()).or_insert_with(|| HybridResult {
            id: point.id.clone(),
            rrf_score: 0.0,
            dense_score: None,
            sparse_score: None,
            payload: point.payload.clone(),
        });
        entry.rrf_score += rrf;
        entry.sparse_score = Some(point.score);
    }

    let mut results: Vec<HybridResult> = map.into_values().collect();
    results.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);
    results
}

pub const DOC_RRF_K: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FtsDocHit {
    pub article_id: i64,
    pub rank: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticDocHit {
    pub article_id: i64,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedDocHit {
    pub article_id: i64,
    pub score: f64,
    pub why: Vec<&'static str>,
}

#[must_use]
pub fn merge_doc_hits(
    fts: &[FtsDocHit],
    semantic: &[SemanticDocHit],
    limit: usize,
) -> Vec<MergedDocHit> {
    fn entry<'a>(
        entries: &'a mut Vec<(i64, f64, Vec<&'static str>)>,
        id: i64,
    ) -> &'a mut (i64, f64, Vec<&'static str>) {
        if let Some(pos) = entries.iter().position(|(aid, _, _)| *aid == id) {
            &mut entries[pos]
        } else {
            entries.push((id, 0.0, Vec::new()));
            entries.last_mut().expect("just pushed")
        }
    }
    let mut entries: Vec<(i64, f64, Vec<&'static str>)> = Vec::new();
    for (i, h) in fts.iter().enumerate() {
        let e = entry(&mut entries, h.article_id);
        e.1 += 1.0 / (DOC_RRF_K as f64 + i as f64 + 1.0);
        if !e.2.contains(&"fts") {
            e.2.push("fts");
        }
    }
    for (i, h) in semantic.iter().enumerate() {
        let e = entry(&mut entries, h.article_id);
        e.1 += 1.0 / (DOC_RRF_K as f64 + i as f64 + 1.0);
        if !e.2.contains(&"semantic") {
            e.2.push("semantic");
        }
    }
    let mut merged: Vec<MergedDocHit> = entries
        .into_iter()
        .map(|(article_id, s, why)| MergedDocHit {
            article_id,
            score: (s * 10_000.0).round() / 10_000.0,
            why,
        })
        .collect();
    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.article_id.cmp(&b.article_id))
    });
    merged.truncate(limit);
    merged
}

#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for i in 0..a.len() {
        dot += f64::from(a[i]) * f64::from(b[i]);
        na += f64::from(a[i]) * f64::from(a[i]);
        nb += f64::from(b[i]) * f64::from(b[i]);
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())) as f32
}

pub fn hybrid_search(
    conn: &Connection,
    vectorstore: &dyn VectorStore,
    collection: &str,
    query_vector: &[f32],
    query_text: &str,
    filter: Option<&Filter>,
    top_k: usize,
) -> Result<Vec<HybridResult>> {
    let dense_results = vectorstore.search_dense(collection, query_vector, filter, top_k)?;

    let fts_results = search::universal_search(conn, query_text)?;
    let sparse_results: Vec<ScoredPoint> = fts_results
        .into_iter()
        .enumerate()
        .map(|(rank, r)| ScoredPoint {
            id: r.remote_id.to_string(),
            score: 1.0 / (1.0 + rank as f32),
            payload: serde_json::json!({
                "type": r.resource_type,
                "title": r.title,
                "snippet": r.snippet,
            }),
        })
        .take(top_k)
        .collect();

    Ok(fuse_results(&dense_results, &sparse_results, top_k))
}

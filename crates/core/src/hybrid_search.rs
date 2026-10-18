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

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(id: &str, score: f32, tag: &str) -> ScoredPoint {
        ScoredPoint {
            id: id.to_string(),
            score,
            payload: serde_json::json!({ "tag": tag }),
        }
    }

    #[test]
    fn rrf_constants_are_pinned() {
        // Characterization: both fusion paths use the classic RRF k = 60.
        assert_eq!(RRF_K, 60);
        assert_eq!(DOC_RRF_K, 60);
    }

    #[test]
    fn fuse_results_with_empty_inputs_is_empty() {
        assert!(fuse_results(&[], &[], 10).is_empty());
    }

    #[test]
    fn fuse_results_dense_only_rrf_math_and_order() {
        let dense = vec![sp("a", 0.9, "d"), sp("b", 0.8, "d"), sp("c", 0.7, "d")];
        let out = fuse_results(&dense, &[], 10);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id, "a");
        assert_eq!(out[1].id, "b");
        assert_eq!(out[2].id, "c");
        assert!((out[0].rrf_score - 1.0 / 61.0).abs() < 1e-12);
        assert!((out[1].rrf_score - 1.0 / 62.0).abs() < 1e-12);
        assert!((out[2].rrf_score - 1.0 / 63.0).abs() < 1e-12);
        assert_eq!(out[0].dense_score, Some(0.9));
        assert_eq!(out[0].sparse_score, None);
        assert_eq!(out[0].payload["tag"], "d");
    }

    #[test]
    fn fuse_results_item_hit_by_both_lists_outranks_single_list() {
        // "a" is rank 0 in dense only: 1/61.
        // "b" is rank 1 in dense plus rank 0 in sparse: 1/62 + 1/61 > 1/61.
        let dense = vec![sp("a", 0.9, "d"), sp("b", 0.8, "d")];
        let sparse = vec![sp("b", 5.0, "s")];
        let out = fuse_results(&dense, &sparse, 10);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "b");
        assert_eq!(out[1].id, "a");
        assert!((out[0].rrf_score - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-12);
        assert_eq!(out[0].dense_score, Some(0.8));
        assert_eq!(out[0].sparse_score, Some(5.0));
    }

    #[test]
    fn fuse_results_truncates_to_top_k() {
        let dense: Vec<ScoredPoint> = ["a", "b", "c", "d", "e"]
            .iter()
            .enumerate()
            .map(|(i, id)| sp(id, 1.0 - i as f32, "d"))
            .collect();
        let out = fuse_results(&dense, &[], 3);
        assert_eq!(out.len(), 3);
        assert_eq!(
            out.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
    }

    #[test]
    fn fuse_results_payload_prefers_dense_source() {
        let dense = vec![sp("a", 0.9, "dense")];
        let sparse = vec![sp("a", 7.0, "sparse"), sp("z", 6.0, "sparse")];
        let out = fuse_results(&dense, &sparse, 10);
        let a = out.iter().find(|r| r.id == "a").expect("a present");
        assert_eq!(a.payload["tag"], "dense");
        let z = out.iter().find(|r| r.id == "z").expect("z present");
        assert_eq!(z.payload["tag"], "sparse");
        assert_eq!(z.dense_score, None);
        assert_eq!(z.sparse_score, Some(6.0));
    }

    #[test]
    fn fuse_results_with_custom_k_uses_it() {
        let dense = vec![sp("a", 0.9, "d")];
        let out = fuse_results_with_k(&dense, &[], 5, 1);
        assert_eq!(out.len(), 1);
        // k = 1, rank 1: 1 / (1 + 1)
        assert!((out[0].rrf_score - 0.5).abs() < 1e-12);
    }

    #[test]
    fn merge_doc_hits_fuses_fts_and_semantic_with_labels() {
        let fts = vec![
            FtsDocHit {
                article_id: 1,
                rank: 0,
            },
            FtsDocHit {
                article_id: 2,
                rank: 1,
            },
        ];
        let semantic = vec![
            SemanticDocHit {
                article_id: 2,
                score: 0.9,
            },
            SemanticDocHit {
                article_id: 3,
                score: 0.8,
            },
        ];
        let merged = merge_doc_hits(&fts, &semantic, 10);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].article_id, 2);
        let id2 = (1.0_f64 / 61.0 + 1.0 / 62.0) * 10_000.0;
        assert!((merged[0].score - id2.round() / 10_000.0).abs() < 1e-12);
        assert_eq!(merged[0].why, vec!["fts", "semantic"]);
        assert_eq!(merged[1].article_id, 1);
        assert_eq!(merged[1].why, vec!["fts"]);
        assert_eq!(merged[2].article_id, 3);
        assert_eq!(merged[2].why, vec!["semantic"]);
    }

    #[test]
    fn merge_doc_hits_uses_list_position_not_rank_field() {
        // Characterization: the `rank` field is ignored; enumerate() position drives RRF.
        let fts = vec![
            FtsDocHit {
                article_id: 1,
                rank: 9,
            },
            FtsDocHit {
                article_id: 2,
                rank: 0,
            },
        ];
        let merged = merge_doc_hits(&fts, &[], 10);
        assert_eq!(merged.len(), 2);
        let first = (1.0_f64 / 61.0 * 10_000.0).round() / 10_000.0;
        let second = (1.0_f64 / 62.0 * 10_000.0).round() / 10_000.0;
        assert!((merged[0].score - first).abs() < 1e-12);
        assert!((merged[1].score - second).abs() < 1e-12);
    }

    #[test]
    fn merge_doc_hits_tie_breaks_by_article_id() {
        let fts = vec![FtsDocHit {
            article_id: 5,
            rank: 0,
        }];
        let semantic = vec![SemanticDocHit {
            article_id: 3,
            score: 1.0,
        }];
        // Both accumulate exactly 1/61: tie -> lower article_id sorts first.
        let merged = merge_doc_hits(&fts, &semantic, 10);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].article_id, 3);
        assert_eq!(merged[1].article_id, 5);
        assert!((merged[0].score - merged[1].score).abs() < 1e-12);
    }

    #[test]
    fn merge_doc_hits_empty_input_and_limit() {
        assert!(merge_doc_hits(&[], &[], 10).is_empty());
        let fts: Vec<FtsDocHit> = (1..=5)
            .map(|article_id| FtsDocHit {
                article_id,
                rank: 0,
            })
            .collect();
        let merged = merge_doc_hits(&fts, &[], 3);
        assert_eq!(merged.len(), 3);
        assert_eq!(
            merged.iter().map(|m| m.article_id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn merge_doc_hits_dedupes_repeated_ids_and_labels() {
        let fts = vec![
            FtsDocHit {
                article_id: 7,
                rank: 0,
            },
            FtsDocHit {
                article_id: 7,
                rank: 1,
            },
        ];
        let merged = merge_doc_hits(&fts, &[], 10);
        assert_eq!(merged.len(), 1);
        let expected = (1.0_f64 / 61.0 + 1.0 / 62.0) * 10_000.0;
        assert!((merged[0].score - expected.round() / 10_000.0).abs() < 1e-12);
        assert_eq!(merged[0].why, vec!["fts"]);
    }

    #[test]
    fn cosine_similarity_edge_cases() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-6);
        assert!((cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]) - (-1.0)).abs() < 1e-6);
        assert!(
            (cosine_similarity(&[1.0, 0.0], &[1.0, 1.0]) - std::f32::consts::FRAC_1_SQRT_2).abs()
                < 1e-6
        );
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine_similarity(&[1.0, 2.0], &[0.0, 0.0]), 0.0);
    }
}

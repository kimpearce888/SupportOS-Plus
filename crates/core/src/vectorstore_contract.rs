use serde_json::json;

use crate::vectorstore::{Filter, Point, SparseVector, VectorStore};

#[allow(clippy::missing_errors_doc)]
pub fn run_contract_tests(store: &dyn VectorStore) -> crate::error::Result<()> {
    test_dense_vectors(store)?;
    test_sparse_vectors(store)?;
    test_payload_filters(store)?;
    test_count(store)?;
    test_snapshots_and_restore(store)?;
    test_collection_lifecycle(store)?;
    Ok(())
}

fn test_dense_vectors(store: &dyn VectorStore) -> crate::error::Result<()> {
    store.create_collection("contract_dense", Some(3))?;

    store.upsert(
        "contract_dense",
        Point {
            id: "a".into(),
            dense: Some(vec![1.0, 0.0, 0.0]),
            sparse: None,
            payload: json!({}),
        },
    )?;
    store.upsert(
        "contract_dense",
        Point {
            id: "b".into(),
            dense: Some(vec![0.0, 1.0, 0.0]),
            sparse: None,
            payload: json!({}),
        },
    )?;
    store.upsert(
        "contract_dense",
        Point {
            id: "c".into(),
            dense: Some(vec![0.7, 0.7, 0.0]),
            sparse: None,
            payload: json!({}),
        },
    )?;

    let results = store.search_dense("contract_dense", &[1.0, 0.0, 0.0], None, 3)?;
    assert_eq!(results.len(), 3, "dense search returns all 3 points");
    assert_eq!(results[0].id, "a", "closest match first");
    assert!(
        results[0].score > results[1].score,
        "sorted by score descending"
    );

    let results = store.search_dense("contract_dense", &[1.0, 0.0, 0.0], None, 1)?;
    assert_eq!(results.len(), 1, "top_k=1 truncates");

    store.drop_collection("contract_dense")?;
    Ok(())
}

fn test_sparse_vectors(store: &dyn VectorStore) -> crate::error::Result<()> {
    store.create_collection("contract_sparse", None)?;

    store.upsert(
        "contract_sparse",
        Point {
            id: "s1".into(),
            dense: None,
            sparse: Some(SparseVector::new(vec![0, 1], vec![1.0, 1.0])),
            payload: json!({}),
        },
    )?;
    store.upsert(
        "contract_sparse",
        Point {
            id: "s2".into(),
            dense: None,
            sparse: Some(SparseVector::new(vec![1, 2], vec![1.0, 1.0])),
            payload: json!({}),
        },
    )?;

    let query = SparseVector::new(vec![0, 1], vec![1.0, 1.0]);
    let results = store.search_sparse("contract_sparse", &query, None, 10)?;
    assert_eq!(results.len(), 2, "sparse search returns both points");
    assert_eq!(results[0].id, "s1", "closest match first");
    assert!(
        results[0].score > results[1].score,
        "sorted by score descending"
    );

    store.drop_collection("contract_sparse")?;
    Ok(())
}

fn test_payload_filters(store: &dyn VectorStore) -> crate::error::Result<()> {
    store.create_collection("contract_filter", Some(2))?;

    store.upsert(
        "contract_filter",
        Point {
            id: "f1".into(),
            dense: Some(vec![1.0, 0.0]),
            sparse: None,
            payload: json!({"mailbox": "101", "status": "active"}),
        },
    )?;
    store.upsert(
        "contract_filter",
        Point {
            id: "f2".into(),
            dense: Some(vec![0.9, 0.1]),
            sparse: None,
            payload: json!({"mailbox": "102", "status": "active"}),
        },
    )?;

    let filter = Filter::new().must_eq("mailbox", "101");
    let results = store.search_dense("contract_filter", &[1.0, 0.0], Some(&filter), 10)?;
    assert_eq!(results.len(), 1, "filter excludes f2");
    assert_eq!(results[0].id, "f1");

    let count = store.count("contract_filter", Some(&filter))?;
    assert_eq!(count, 1, "count respects filter");

    store.drop_collection("contract_filter")?;
    Ok(())
}

fn test_count(store: &dyn VectorStore) -> crate::error::Result<()> {
    store.create_collection("contract_count", Some(2))?;

    assert_eq!(store.count("contract_count", None)?, 0);

    for i in 0..3 {
        store.upsert(
            "contract_count",
            Point {
                id: format!("p{i}"),
                dense: Some(vec![i as f32, 0.0]),
                sparse: None,
                payload: json!({}),
            },
        )?;
    }
    assert_eq!(store.count("contract_count", None)?, 3, "count = 3");

    store.delete("contract_count", &"p1".to_string())?;
    assert_eq!(
        store.count("contract_count", None)?,
        2,
        "count = 2 after delete"
    );

    assert_eq!(store.count("nonexistent", None)?, 0);

    store.drop_collection("contract_count")?;
    Ok(())
}

fn test_snapshots_and_restore(store: &dyn VectorStore) -> crate::error::Result<()> {
    store.create_collection("contract_snap", Some(2))?;
    store.upsert(
        "contract_snap",
        Point {
            id: "snap1".into(),
            dense: Some(vec![1.0, 0.0]),
            sparse: None,
            payload: json!({"k": "v"}),
        },
    )?;

    let bytes = store.snapshot("contract_snap")?;
    assert!(!bytes.is_empty(), "snapshot produces bytes");

    store.restore(&bytes)?;
    let info = store
        .collection_info("contract_snap")?
        .ok_or_else(|| crate::error::Error::Config("collection_info returned None".into()))?;
    assert_eq!(info.point_count, 1, "restored collection has 1 point");

    let results = store.search_dense("contract_snap", &[1.0, 0.0], None, 10)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "snap1");

    store.drop_collection("contract_snap")?;
    Ok(())
}

fn test_collection_lifecycle(store: &dyn VectorStore) -> crate::error::Result<()> {
    store.create_collection("contract_lifecycle", Some(3))?;
    let info = store
        .collection_info("contract_lifecycle")?
        .ok_or_else(|| crate::error::Error::Config("collection_info returned None".into()))?;
    assert_eq!(info.name, "contract_lifecycle");
    assert_eq!(info.dense_dim, Some(3));

    store.create_collection("contract_lifecycle", Some(3))?;

    store.drop_collection("contract_lifecycle")?;
    assert!(
        store.collection_info("contract_lifecycle")?.is_none(),
        "collection gone after drop"
    );

    store.drop_collection("contract_lifecycle")?;
    Ok(())
}

//! The shared Reciprocal-Rank-Fusion constant.
//!
//! Clean-audit item 154: this module used to carry a whole fusion stack —
//! `HybridResult` + `fuse_results(_with_k)` + `merge_doc_hits` with its
//! hit types + `cosine_similarity` + a `hybrid_search` entry point — and
//! every piece of it was dead. The live search lanes implement RRF
//! directly in `search.rs` (which also reads this constant), and the old
//! entry point's two sides were even latent-broken: it fused sqlite-vec
//! point ids (chunk-family namespaced since item 21) against conversation
//! remote ids, so a match between the lists was unrepresentable.

/// The classic RRF rank constant (k = 60), pinned by the search tests and
/// used by the live fusion lanes in [`crate::search`].
pub const RRF_K: u32 = 60;

#[cfg(test)]
mod tests {
    use super::*;

    /// Characterization: the live fusion lanes all fuse with k = 60.
    #[test]
    fn rrf_constant_is_pinned() {
        assert_eq!(RRF_K, 60);
    }
}

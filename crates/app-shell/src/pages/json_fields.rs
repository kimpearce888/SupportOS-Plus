//! Shared JSON field readers for API payloads (GA-2).
//!
//! The graphify knowledge-graph audit flagged these helpers as living inside
//! a page module (`queries.rs`) while 9+ sibling pages consume them through
//! the `pages` re-export; they now have a neutral home. The `pub(crate) use`
//! in `pages/mod.rs` keeps the `use super::{...}` import path of every
//! consumer unchanged.

/// First string field of `v`, or `""` when absent or not a string.
pub(crate) fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|f| f.as_str())
        .unwrap_or_default()
        .to_string()
}

/// First i64 field of `v`, or `0` when absent or not an integer.
pub(crate) fn i64_field(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key).and_then(|f| f.as_i64()).unwrap_or_default()
}

/// First non-empty string field of `v`, or `None`.
pub(crate) fn opt_str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|f| f.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

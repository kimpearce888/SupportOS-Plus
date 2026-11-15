use super::*;

#[test]
fn hash_prefixed_number_entries_parse() {
    assert_eq!(parse_number_entry("#778"), Some(778));
    assert_eq!(parse_number_entry("778"), Some(778));
    assert_eq!(parse_number_entry("  # 778  "), Some(778));
    assert_eq!(parse_number_entry("# 778"), Some(778));
    assert_eq!(parse_number_entry(""), None);
    assert_eq!(parse_number_entry("#abc"), None);
    assert_eq!(
        parse_number_entry("##42"),
        Some(42),
        "leading hashes all strip"
    );
}

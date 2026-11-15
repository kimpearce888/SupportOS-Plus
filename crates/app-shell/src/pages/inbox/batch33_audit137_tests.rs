use super::*;
use crate::testsupport::fixture;

#[test]
fn invalidation_refreshes_the_filter_refs() {
    let db = fixture(vec![("conv", 1, "First", "active", "email", 1, 5, 9)]);
    let mut page = InboxPage::new();
    page.select(&db, 1);
    page.reload_list(&db);
    assert!(!page.mailboxes.is_empty(), "refs load with the page");
    assert!(page.tags.is_empty(), "no tags seeded yet");

    db.with_write(|conn| crate::testsupport::seed_tag(conn, 1, "billing"));

    page.invalidate();
    assert!(
        page.mailboxes.is_empty(),
        "invalidation clears the refs caches"
    );
    page.reload_list(&db);
    assert!(
        page.tags.iter().any(|t| t == "billing"),
        "the synced tag reaches the tag filter: {:?}",
        page.tags
    );
}

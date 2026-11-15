use super::*;
use crate::testsupport::{fixture, seed_conversation};

fn seed_many(conn: &mut rusqlite::Connection) {
    for i in 1..=55i64 {
        seed_conversation(conn, i, &format!("conv {i}"), "active", "email", 1, 5, 9);
    }
}

#[test]
fn page_one_reaches_conversations_past_the_cap() {
    let db = fixture(Vec::new());
    db.with_write(seed_many);

    let mut page = InboxPage::new();
    page.reload_list(&db);
    assert_eq!(page.total, 55, "the total counts every match");
    assert_eq!(page.list.len(), 50, "page 0 holds the page size");
    let page_zero_ids: std::collections::HashSet<i64> = page.list.iter().map(|i| i.id).collect();
    assert_eq!(page_zero_ids.len(), 50, "no duplicates within page 0");

    page.page = 1;
    page.reload_list(&db);
    assert_eq!(page.list.len(), 5, "page 1 holds the rest");
    assert_eq!(page.total, 55, "the total is page-independent");
    let mut seen = page_zero_ids;
    for item in &page.list {
        assert!(seen.insert(item.id), "no row appears on two pages");
    }
    assert_eq!(seen.len(), 55, "every conversation is reachable");
}

#[test]
fn offset_compiles_from_the_page_index() {
    let db = fixture(Vec::new());
    let bar = FilterBar::default();
    let filters = db.with_read(|conn| filters_from_bar(&bar, 2, conn));
    assert_eq!(filters.limit, Some(INBOX_PAGE_SIZE));
    assert_eq!(filters.offset, Some(2 * INBOX_PAGE_SIZE));
}

#[test]
fn a_page_past_the_end_falls_back_to_the_last_page() {
    let db = fixture(Vec::new());
    db.with_write(seed_many);

    let mut page = InboxPage::new();
    page.reload_list(&db);
    assert_eq!(page.list.len(), 50);

    db.with_write(|conn| {
        conn.execute("DELETE FROM conversations WHERE id > 12", [])
            .expect("delete");
    });
    page.reload_list(&db);
    assert_eq!(page.page, 0, "the past-the-end page clamped");
    assert_eq!(page.list.len(), 12, "the last-page rows render");
    assert!(!page.list.is_empty());
}

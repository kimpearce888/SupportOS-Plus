use super::*;
use crate::testsupport::fixture;

#[test]
fn row_click_stashes_and_restores_the_typed_draft() {
    let db = fixture(vec![("conv", 1, "First", "active", "email", 1, 5, 9)]);
    db.with_write(|conn| {
        crate::testsupport::seed_conversation(conn, 2, "Second", "active", "email", 1, 5, 10);
    });

    let mut page = InboxPage::new();
    page.select(&db, 1);
    page.composer_mode = ComposerMode::Note;
    page.composer_body = "typed but unsent reply".into();
    page.composer_cc = "cc@example.com".into();

    page.select(&db, 2);
    assert_eq!(page.composer_body, "", "the incoming row starts clean");
    assert_eq!(
        page.composer_mode,
        ComposerMode::Reply,
        "the incoming row starts in reply mode"
    );

    page.select(&db, 1);
    assert_eq!(page.composer_body, "typed but unsent reply");
    assert_eq!(page.composer_cc, "cc@example.com");
    assert_eq!(page.composer_mode, ComposerMode::Note);
}

#[test]
fn reselecting_the_same_row_keeps_the_typing_untouched() {
    let db = fixture(vec![("conv", 1, "First", "active", "email", 1, 5, 9)]);
    let mut page = InboxPage::new();
    page.select(&db, 1);
    page.composer_body = "mid-thought".into();

    page.select(&db, 1);
    assert_eq!(
        page.composer_body, "mid-thought",
        "clicking the open row changes nothing"
    );
}

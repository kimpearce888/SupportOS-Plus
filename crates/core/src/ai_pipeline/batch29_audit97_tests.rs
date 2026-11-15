use super::*;

#[test]
fn audit97_rewrite_prompt_is_redacted() {
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    let draft = "Please charge my card 4111 1111 1111 1111, cvv: 1234 soon.";
    let safe = redact_for_model(&conn, draft).expect("static patterns compile");
    assert!(
        safe.contains("[REDACTED-CARD]"),
        "the card number must not reach the model verbatim: {safe}"
    );
    assert!(
        safe.contains("[REDACTED-CVV]"),
        "the cvv must not reach the model verbatim: {safe}"
    );
    assert!(
        !safe.contains("4111 1111 1111 1111"),
        "raw card digits are forbidden in the rewrite prompt: {safe}"
    );
    let user = format!(
        "{}\n\nReply to rewrite:\n\"\"\"\n{}\n\"\"\"",
        rewrite_instruction("warmer"),
        safe
    );
    assert!(user.contains("[REDACTED-CARD]"));
    assert!(user.starts_with("Rewrite the reply with a warmer"));

    crate::settings::set_bool(&conn, "redaction_enabled", false).expect("setting");
    assert_eq!(
        redact_for_model(&conn, draft).expect("static patterns compile"),
        draft
    );
}

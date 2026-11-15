use super::*;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn list_ratings_and_chats_state_the_platform_reality() {
    let conn = Connection::open_in_memory().expect("db");
    let provider =
        RealHelpScoutProvider::new(crate::db::DbHandle::new(conn), HsCredentials::default());
    let runtime = rt();
    let ratings_err = runtime
        .block_on(provider.list_ratings())
        .expect_err("list_ratings must refuse to pretend the platform lists ratings");
    assert!(
        ratings_err
            .to_string()
            .contains("no list endpoint for ratings"),
        "unexpected message: {ratings_err}"
    );
    let chats_err = runtime
        .block_on(provider.list_beacon_chats())
        .expect_err("list_beacon_chats must refuse to pretend the platform lists chats");
    assert!(
        chats_err.to_string().contains("no list endpoint for chats"),
        "unexpected message: {chats_err}"
    );
}

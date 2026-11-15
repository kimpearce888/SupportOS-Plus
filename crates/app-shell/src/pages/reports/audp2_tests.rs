use super::*;

#[test]
fn audp2_csv_id_helpers_round_trip() {
    let mut csv = String::new();
    assert!(!csv_contains_id(&csv, 1));
    csv = toggle_csv_id(&csv, 1);
    assert_eq!(csv, "1");
    assert!(csv_contains_id(&csv, 1));
    csv = toggle_csv_id(&csv, 2);
    assert_eq!(csv, "1,2");
    assert!(csv_contains_id(&csv, 2));
    csv = toggle_csv_id(&csv, 1);
    assert_eq!(csv, "2");
    assert!(!csv_contains_id(&csv, 1));
    let junk = " 5 , x , 7 ";
    assert!(csv_contains_id(junk, 5));
    assert!(csv_contains_id(junk, 7));
    assert!(!csv_contains_id(junk, 6));
}

#[test]
fn audp2_delete_graph_node_confirm_request_shapes() {
    let req = ConfirmRequest::delete_graph_node(7, "Kim Pearce");
    assert!(req.danger);
    assert!(req.message.contains("Kim Pearce"));
    assert!(matches!(
        req.action,
        super::super::DestructiveAction::DeleteGraphNode { id: 7 }
    ));
}

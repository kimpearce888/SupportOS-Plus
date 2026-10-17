use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::super::server::AppState;
use crate::catalog::{GraphHumanRelation, GraphNodeKind};

pub async fn stats(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::graph_stats(&conn) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn meta(State(_state): State<AppState>) -> Json<Value> {
    let kinds: Vec<&str> = GraphNodeKind::ALL.iter().map(|k| k.as_str()).collect();
    let relations: Vec<&str> = GraphHumanRelation::ALL.iter().map(|r| r.as_str()).collect();
    Json(json!({
        "node_kinds": kinds,
        "human_relations": relations,
        "notes": [
            "Derived edges are computed live from the local mirror; only human-asserted edges are stored.",
            "Connector rows have no derived links by design."
        ]
    }))
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let query = params.get("q").cloned().unwrap_or_default();
    if query.chars().count() > 200 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Search query too long (max 200 chars)."
            })),
        )
            .into_response();
    }
    let mut kinds: Option<Vec<GraphNodeKind>> = None;
    if let Some(raw) = params.get("kinds").filter(|k| !k.trim().is_empty()) {
        let mut parsed: Vec<GraphNodeKind> = Vec::new();
        let mut ok = true;
        for k in raw.split(',').map(|k| k.trim()).filter(|k| !k.is_empty()) {
            match GraphNodeKind::parse(k) {
                Some(kind) => parsed.push(kind),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "Unknown node kind in kinds filter."
                })),
            )
                .into_response();
        }
        kinds = Some(parsed);
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::graph_search(&conn, &query, kinds.as_deref()) {
        Ok(results) => Json(json!({ "results": results })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

fn parse_positive_int(raw: &str) -> Option<i64> {
    let n: f64 = raw.trim().parse().ok()?;
    if n.fract() == 0.0 && n > 0.0 && n <= i64::MAX as f64 {
        Some(n as i64)
    } else {
        None
    }
}

pub async fn node(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, String)>,
) -> Response {
    let Some(kind) = GraphNodeKind::parse(&kind) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Unknown node kind."
            })),
        )
            .into_response();
    };
    let Some(id) = parse_positive_int(&id) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Node id must be a positive integer."
            })),
        )
            .into_response();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::neighbors_json(&conn, kind, id, "both", 5) {
        Ok(Some(nb)) => Json(json!({
            "node": nb["node"],
            "edge_count": nb["total_edges"],
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Node not found."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn neighbors(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some(kind) = GraphNodeKind::parse(&kind) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Unknown node kind."
            })),
        )
            .into_response();
    };
    let Some(id) = parse_positive_int(&id) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Node id must be a positive integer."
            })),
        )
            .into_response();
    };
    let direction = params
        .get("direction")
        .map(|s| s.as_str())
        .unwrap_or("both");
    if !matches!(direction, "out" | "in" | "both") {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "direction must be out, in or both."
            })),
        )
            .into_response();
    }
    let limit = match params
        .get("limit")
        .map(|l| l.trim().parse::<f64>())
        .unwrap_or(Ok(200.0))
    {
        Ok(n) if n.is_finite() && n >= 1.0 && n <= 200.0 => n as i64,
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "limit must be between 1 and 200."
                })),
            )
                .into_response();
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::neighbors_json(&conn, kind, id, direction, limit) {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Node not found."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn subgraph(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some(kind) = GraphNodeKind::parse(&kind) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Unknown node kind."
            })),
        )
            .into_response();
    };
    let Some(id) = parse_positive_int(&id) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Node id must be a positive integer."
            })),
        )
            .into_response();
    };
    let depth = match params
        .get("depth")
        .map(|d| d.trim().parse::<f64>())
        .unwrap_or(Ok(1.0))
    {
        Ok(n) if n.is_finite() && n.fract() == 0.0 && (1.0..=2.0).contains(&n) => n as u32,
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "depth must be 1 or 2."
                })),
            )
                .into_response();
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::subgraph_json(&conn, kind, id, depth, 120) {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Node not found."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn list_edges(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let limit = js_number_or(&params, "limit", 50.0);
    let limit = limit.clamp(1.0, 200.0) as i64;
    let offset = js_number_or(&params, "offset", 0.0).max(0.0) as i64;

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let (edges, total) =
        crate::support_graph::list_human_edges(&conn, limit, offset).unwrap_or_default();
    Json(json!({
        "edges": edges.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
        "total": total,
    }))
}

fn js_number_or(params: &HashMap<String, String>, key: &str, default: f64) -> f64 {
    match params.get(key) {
        None => default,
        Some(raw) => {
            let n = crate::conversation_ops::js_number(raw).unwrap_or(f64::NAN);
            if n == 0.0 || n.is_nan() {
                default
            } else {
                n
            }
        }
    }
}

pub async fn create_edge(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let input = match crate::support_graph::validate_link_body(&body) {
        Ok(input) => input,
        Err(issues) => {
            let refs: Vec<(&str, &str)> = issues
                .iter()
                .map(|(path, message)| (*path, message.as_str()))
                .collect();
            return crate::conversation_ops::zod_422_multi(&refs);
        }
    };

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::link_human_edge(&conn, &input) {
        Ok(edge) => Json(json!({"ok": true, "edge": edge.to_json()})).into_response(),
        Err(crate::support_graph::LinkError::SelfEdge) => (
            StatusCode::CONFLICT,
            Json(json!({
                "statusCode": 409,
                "error": "Conflict",
                "message": "A node cannot be linked to itself."
            })),
        )
            .into_response(),
        Err(crate::support_graph::LinkError::SourceNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Source node not found."
            })),
        )
            .into_response(),
        Err(crate::support_graph::LinkError::TargetNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Target node not found."
            })),
        )
            .into_response(),
        Err(crate::support_graph::LinkError::Duplicate) => (
            StatusCode::CONFLICT,
            Json(json!({
                "statusCode": 409,
                "error": "Conflict",
                "message": "This edge already exists (duplicate)."
            })),
        )
            .into_response(),
    }
}

pub async fn delete_edge(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    if id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Edge id must be a positive integer."
            })),
        )
            .into_response();
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::support_graph::unlink_human_edge(&conn, id) {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Human edge not found."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

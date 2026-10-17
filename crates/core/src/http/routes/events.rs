use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::http::{header, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use futures_util::stream::{self, StreamExt as _};
use tokio_stream::wrappers::{BroadcastStream, IntervalStream};

use super::super::server::AppState;
use crate::events::ServerEvent;

pub const MAX_STREAMS: usize = 25;

pub const HELLO_CHANNELS: [&str; 6] = [
    "ratings",
    "sync",
    "conversations",
    "campaigns",
    "notifications",
    "errors",
];

pub async fn sse_handler(State(state): State<AppState>) -> axum::response::Response {
    if state.bus.subscriber_count() >= MAX_STREAMS {
        let body = axum::body::Body::from(
            "event: error\ndata: {\"message\":\"Too many event streams open. Close another tab.\"}\n\n",
        );
        return axum::response::Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache, no-transform")
            .header("connection", "keep-alive")
            .header("x-accel-buffering", "no")
            .body(body)
            .expect("building SSE cap response from literals must succeed");
    }

    sse_stream(state)
}

fn sse_stream(state: AppState) -> axum::response::Response {
    let rx = state.bus.subscribe();

    let event_stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => Some(Ok::<_, Infallible>(server_event_to_sse(&event))),
            Err(_lagged) => Some(Ok(Event::default().comment("lagged"))),
        }
    });

    let keep_alive = IntervalStream::new(tokio::time::interval(Duration::from_secs(25)))
        .map(|_| Ok::<_, Infallible>(Event::default().comment("ping")));

    let hello = stream::once(async {
        Ok::<_, Infallible>(
            Event::default().event("hello").data(
                serde_json::json!({
                    "at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    "channels": HELLO_CHANNELS,
                    "version": crate::http::routes::system::APP_VERSION,
                })
                .to_string(),
            ),
        )
    });

    let merged = hello.chain(stream::select(event_stream, keep_alive));

    use axum::response::IntoResponse;
    let mut response = Sse::new(merged)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(25))
                .text("ping"),
        )
        .into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
}

fn server_event_to_sse(event: &ServerEvent) -> Event {
    Event::default()
        .event(event.wire_name())
        .data(event.payload_json())
}

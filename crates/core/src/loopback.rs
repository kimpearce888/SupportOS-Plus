use std::net::SocketAddr;

use axum::{
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Router,
};
use tokio::net::TcpListener;

use crate::error::Result;

pub struct Loopback {
    addr: SocketAddr,
}

impl Loopback {
    pub async fn bind(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        let addr = listener.local_addr()?;
        drop(listener);
        Ok(Self { addr })
    }

    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn serve(self) -> std::io::Result<()> {
        let app = Router::new()
            .route("/oauth/callback", get(oauth_callback))
            .route("/webhooks/helpscout", post(webhook_helpscout))
            .fallback(any(not_found));

        let listener = TcpListener::bind(self.addr).await?;
        tracing::info!(addr = %self.addr, "loopback listener bound");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

fn json_ok(route: &str, todo: &str) -> Response {
    let body = format!(r#"{{"ok":true,"route":"{route}","todo":"{todo}"}}"#);
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}

fn json_404() -> Response {
    let body = r#"{"error":"not found"}"#;
    (
        axum::http::StatusCode::NOT_FOUND,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}

async fn oauth_callback() -> Response {
    json_ok("oauth_callback", "M2")
}

async fn webhook_helpscout() -> Response {
    json_ok("webhook_helpscout", "M2")
}

async fn not_found() -> Response {
    json_404()
}

pub async fn smoke_bind() -> Result<()> {
    let _l = Loopback::bind(0).await.map_err(crate::error::Error::Io)?;
    Ok(())
}

//! axum web UI: serves a single-page status view of the latest tick plus
//! the JSON snapshot behind it.
//!
//! The server shares [`WebState`] with the polling loop: each tick's
//! [`TickResult`] is written into the state by the output fan-out, and the
//! HTTP handlers only ever read that snapshot, so a slow or absent browser
//! never blocks or is needed by the tracker itself.

use crate::model::TickResult;
use crate::output::WebState;
use anyhow::Context;
use axum::{Router, extract::State, response::Html, routing::get};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn latest(State(state): State<Arc<WebState>>) -> axum::Json<Option<TickResult>> {
    axum::Json(state.latest())
}

pub fn router(state: Arc<WebState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/latest", get(latest))
        .with_state(state)
}

/// Run the web server until the process exits (or the server errors).
pub async fn serve(bind: String, state: Arc<WebState>) -> anyhow::Result<()> {
    let addr: SocketAddr = bind
        .parse()
        .with_context(|| format!("invalid web bind address {bind:?}"))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding web server to {addr}"))?;
    println!("web UI: http://{addr}");
    axum::serve(listener, router(state))
        .await
        .context("web server error")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::FlightOutput;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_index_page() {
        let response = router(Arc::new(WebState::new()))
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("rustisflying"));
        assert!(html.contains("/api/latest"));
    }

    #[tokio::test]
    async fn serves_null_before_first_tick() {
        let response = router(Arc::new(WebState::new()))
            .oneshot(
                Request::builder()
                    .uri("/api/latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn serves_latest_tick_as_json() {
        let state = Arc::new(WebState::new());
        state.emit(&TickResult::Empty { radius_km: 30.0 });
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
        assert_eq!(json["status"], "empty");
        assert_eq!(json["radius_km"], 30.0);
    }
}

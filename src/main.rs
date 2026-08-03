use axum::extract::State;
use axum::http::HeaderValue;
use axum::{Json, Router, http::StatusCode, routing::post};
use clap::Parser;
use futures::TryFutureExt;
use reqwest::{Url, header};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::future::ready;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{Instrument, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct ProgramArgs {
    #[arg(
        short,
        long,
        help = "URL to route requests to",
        env = "PROXY_TARGET_URL"
    )]
    url: Url,

    #[arg(
        short,
        long,
        help = "Authentication token to use for requests",
        env = "PROXY_TARGET_AUTH"
    )]
    auth: Option<HeaderValue>,

    #[arg(
        long,
        help = "Address to listen on",
        env = "PROXY_LISTEN_ADDRESS",
        default_value = "0.0.0.0:3000"
    )]
    listen: String,

    #[arg(
        long,
        help = "Request timeout duration (e.g., '30s', '1m')",
        env = "PROXY_REQUEST_TIMEOUT",
        default_value = "30s"
    )]
    request_timeout: humantime::Duration,
}

#[derive(Debug)]
struct AppState {
    client: reqwest::Client,
    url: Url,
    auth: Option<HeaderValue>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        // This allows you to use, e.g., `RUST_LOG=info` or `RUST_LOG=debug`
        // when running the app to set log levels.
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("info"))
                .unwrap(),
        )
        .init();

    let args = ProgramArgs::parse();

    let client = match reqwest::Client::builder()
        .timeout(args.request_timeout.into())
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::error!(error=?e, "Failed to build HTTP client");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to build HTTP client",
            ));
        }
    };

    let state = AppState {
        client,
        url: args.url,
        auth: args.auth,
    };

    info!(listen = args.listen, "Starting server");

    // build our application with a route
    let app = Router::new()
        .route("/", post(query))
        .with_state(Arc::new(state))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await
}

async fn query(
    State(state): State<Arc<AppState>>,
    Json(query): Json<SGQuery>,
) -> Result<Json<IndexerResponse>, (StatusCode, String)> {
    let mut req = state.client.post(state.url.clone()).json(&query);

    if let Some(auth) = &state.auth {
        req = req.header(header::AUTHORIZATION, auth);
    }

    let res = req
        .send()
        .and_then(|x| ready(x.error_for_status()))
        .and_then(|x| x.text())
        .map_err(|e| e.to_string())
        .instrument(tracing::info_span!("Sending request to provider"))
        .await;

    match res {
        Ok(data) => Ok(Json(IndexerResponse {
            graphql_response: Some(data),
        })),
        Err(err) => {
            tracing::error!("Error sending request to provider: {}", err);
            Err((
                StatusCode::BAD_GATEWAY,
                "Error sending request to provider".to_string(),
            ))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SGQuery {
    pub query: Box<RawValue>,
    #[serde(default)]
    pub variables: Option<Box<RawValue>>,
}

#[derive(Debug, Serialize)]
struct IndexerResponse {
    #[serde(rename = "graphQLResponse")]
    pub graphql_response: Option<String>,
}

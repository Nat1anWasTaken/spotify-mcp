mod api;
mod auth;
mod config;
mod crypto;
mod error;
mod links;
mod mcp;
mod net;
mod oauth;
mod sessions;
mod state;
#[cfg(test)]
mod testing;
mod web;

use std::sync::Arc;

use anyhow::Context;
use axum::{Router, routing::get};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,spotify_mcp=debug,spotify=debug".into()),
        )
        .init();

    let config = Arc::new(Config::from_env()?);
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;
    sqlx::migrate!("../../migrations").run(&db).await?;
    mcp::settle_interrupted_calls(&db).await?;
    let state = AppState::new(config.clone(), db)?;

    oauth::spawn_cleanup(state.db.clone());
    let app = app(state);

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, url = %config.public_url, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(auth::router())
        .merge(oauth::router())
        .nest("/api", api::router(state.clone()))
        .merge(mcp::router(state.clone()))
        .fallback_service(web::service(&state.config.web_dist))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.ok() };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

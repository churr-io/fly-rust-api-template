use std::env::args;

use crate::api::{api_router, Context};
use axum::middleware::from_fn_with_state;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing::{info, Level};

use crate::config::AppConfig;
use crate::http_util::{cors_layer, rate_limit_middleware, rate_limiter, ProxyProtoAcceptor};

mod api;
mod config;
mod http_util;
mod util;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config_path = args().skip(1).next();
    let config = AppConfig::load(config_path.as_ref())?;
    let middleware = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .include_headers(true)
                        .level(Level::INFO),
                )
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .include_headers(true)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .layer(cors_layer(&config)?)
        .layer(from_fn_with_state(
            rate_limiter(&config),
            rate_limit_middleware,
        ))
        .layer(TimeoutLayer::new(config.http.timeout));

    let route_middleware = ServiceBuilder::new().layer(CompressionLayer::new());

    let ctx = Context::new(config.clone());
    let app = api_router(ctx.clone())
        .layer(middleware)
        .route_layer(route_middleware);

    let addr = format!("{}:{}", config.http.address, config.http.port);
    info!("Starting http server on {}", addr);

    let http_server = axum_server::bind(addr.parse()?).acceptor(ProxyProtoAcceptor::new(
        config.http.proxy_proto_enabled,
        Some(config.http.conn_read_timeout),
        Some(config.http.conn_write_timeout),
    ));
    http_server.serve(app.into_make_service()).await?;
    Ok(())
}

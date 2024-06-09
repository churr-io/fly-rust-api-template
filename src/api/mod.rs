use std::fmt::Debug;

use axum::{debug_handler, Json, Router};
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::from_fn;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;
use tracing::instrument;
use crate::config::AppConfig;

use crate::http_util::{deny_external_request, health_handler};

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("resource not found: {0}")]
    #[allow(dead_code)]
    NotFound(String),
}

#[derive(Clone, Debug)]
pub struct Context {
    pub config: AppConfig
}

impl Context {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        #[derive(Debug, Serialize)]
        struct Body {
            message: String,
        }
        tracing::error!("api error: {}", self);
        match self {
            ApiError::NotFound(id) => {
                (StatusCode::NOT_FOUND, Json(Body { message: id })).into_response()
            }
        }
    }
}

pub fn api_router(ctx: Context) -> Router {
    let deny_external = from_fn(deny_external_request);
    Router::new()
        .route("/", get(hello_world))
        .route("/info", get(info))
        .route("/internal", get(hello_world).layer(deny_external))
        .route("/health", get(health_handler))
        .with_state(ctx)
}

#[derive(Serialize)]
pub struct HelloWorldResponse {
    hello: String,
}

#[instrument]
pub async fn hello_world() -> Result<Json<HelloWorldResponse>, ApiError> {
    Ok(Json(HelloWorldResponse {
        hello: "World".to_owned(),
    }))
}

#[instrument]
#[debug_handler]
pub async fn info(State(ctx): State<Context>) -> Result<Json<AppConfig>, ApiError> {
    Ok(Json(ctx.config))
}
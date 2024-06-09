use std::fmt::Debug;
use std::future::Future;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::api::ApiError;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_server::accept::Accept;
use futures_util::FutureExt;
use governor::clock::DefaultClock;
use governor::state::keyed::DashMapStateStore;
use governor::Quota;
use proxy_header::io::ProxiedStream;
use proxy_header::ParseConfig;
use tokio::net::TcpStream;
use tokio_io_timeout::TimeoutStream;
use tower::util::BoxCloneService;
use tower::{Layer, Service, ServiceExt};
use tower_http::cors::{Any, CorsLayer};
use tracing::instrument;

use crate::config::AppConfig;

pub fn cors_layer(_config: &AppConfig) -> Result<CorsLayer, anyhow::Error> {
    let layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Method::GET);
    Ok(layer)
}

#[instrument]
pub async fn health_handler() -> Result<StatusCode, ApiError> {
    Ok(StatusCode::NO_CONTENT)
}

pub type RateLimiter = governor::RateLimiter<String, DashMapStateStore<String>, DefaultClock>;

pub async fn rate_limit_middleware(
    State(rate_limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Response {
    // no rate limiting if ip is not set, indicates internal request
    let ip = req.extensions().get::<ProxyProtoAddr>();
    if let Some(ip) = ip {
        match rate_limiter.check_key(&ip.0.to_string()) {
            Ok(_) => next.run(req).await,
            Err(_err) => StatusCode::TOO_MANY_REQUESTS.into_response(),
        }
    } else {
        next.run(req).await
    }
}

pub fn rate_limiter(config: &AppConfig) -> Arc<RateLimiter> {
    RateLimiter::dashmap(
        Quota::with_period(config.http.rate_limit.refill_interval)
            .unwrap()
            .allow_burst(NonZeroU32::new(config.http.rate_limit.max_burst_limit as u32).unwrap()),
    )
    .into()
}

#[derive(Debug)]
pub struct ProxyProtoAcceptor<B> {
    enabled: bool,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    phantom: PhantomData<B>,
}

#[derive(Clone, Debug)]
pub struct ProxyProtoAddr(IpAddr);

impl<B> Clone for ProxyProtoAcceptor<B> {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            read_timeout: self.read_timeout,
            write_timeout: self.write_timeout,
            phantom: Default::default(),
        }
    }
}

impl<B> ProxyProtoAcceptor<B> {
    pub fn new(
        enabled: bool,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> Self {
        Self {
            enabled,
            read_timeout,
            write_timeout,
            phantom: Default::default(),
        }
    }
    async fn is_using_proxy_protocol(stream: &TcpStream) -> Result<bool, std::io::Error> {
        let mut buf = [0; 5];
        stream.peek(&mut buf).await?;
        Ok(&buf == b"PROXY")
    }
}

impl<S, B> Accept<TcpStream, S> for ProxyProtoAcceptor<B>
where
    for<'a> S: Service<Request<B>> + Clone + Send + 'a,
    for<'a> S::Future: Send + 'a,
    for<'a> S::Response: 'a,
    for<'a> S::Error: 'a,
    for<'a> B: 'a,
{
    type Stream = Pin<Box<TimeoutStream<ProxiedStream<TcpStream>>>>;
    type Service = BoxCloneService<Request<B>, S::Response, S::Error>;
    type Future =
        Pin<Box<dyn Future<Output = std::io::Result<(Self::Stream, Self::Service)>> + Send>>;

    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        let enabled = self.enabled;
        let read_timeout = self.read_timeout;
        let write_timeout = self.write_timeout;
        async move {
            let proxied;
            let stream = if enabled && Self::is_using_proxy_protocol(&stream).await? {
                proxied = true;
                ProxiedStream::create_from_tokio(stream, ParseConfig::default()).await?
            } else {
                proxied = false;
                ProxiedStream::unproxied(stream)
            };
            let ip = stream
                .proxy_header()
                .proxied_address()
                .map(|v| v.source.ip().to_canonical());

            // Fail if no address is set, all external connections should use the proxy protocol
            if proxied && ip.is_none() {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    "no proxied address",
                ));
            }
            let service = service
                .map_request(move |mut req: Request<B>| {
                    if let Some(ip) = ip.as_ref() {
                        req.extensions_mut().insert(ProxyProtoAddr(ip.clone()));
                    }
                    req
                })
                .boxed_clone();

            // Wrap in timeout to close out stale connections
            let mut stream = TimeoutStream::new(stream);
            stream.set_read_timeout(read_timeout);
            stream.set_write_timeout(write_timeout);
            Ok((Box::pin(stream), service))
        }
        .boxed()
    }
}


#[derive(Clone)]
#[allow(dead_code)]
struct InternalRequestFilterLayer;

impl<S> Layer<S> for InternalRequestFilterLayer {
    type Service = S;

    fn layer(&self, inner: S) -> Self::Service {
        inner
    }
}

pub trait InternalRequest {
    #[allow(dead_code)]
    fn is_internal(&self) -> bool;
}

#[allow(dead_code)]
pub async fn deny_external_request(req: Request, next: Next) -> Response {
    if !req.is_internal() {
        StatusCode::FORBIDDEN.into_response()
    } else {
        next.run(req).await
    }
}

impl<B> InternalRequest for Request<B> {
    fn is_internal(&self) -> bool {
        let proxy_ip: Option<&ProxyProtoAddr> = self.extensions().get();
        proxy_ip.is_none()
    }
}
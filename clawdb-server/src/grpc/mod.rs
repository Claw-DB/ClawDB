pub mod service;

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use anyhow::{Context as _, Result};
use http::{Request, Response, StatusCode};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::{
    body::empty_body,
    transport::{Identity, Server, ServerTlsConfig},
};
use tower::{Layer, Service, ServiceBuilder};

use crate::{
    grpc::service::{
        proto::{claw_db_service_server::ClawDbServiceServer, FILE_DESCRIPTOR_SET},
        ClawDbServiceImpl,
    },
    state::AppState,
};

#[derive(Clone)]
struct GrpcRateLimitLayer {
    state: Arc<AppState>,
}

impl<S> Layer<S> for GrpcRateLimitLayer {
    type Service = GrpcRateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcRateLimitService {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
struct GrpcRateLimitService<S> {
    inner: S,
    state: Arc<AppState>,
}

impl<S, B> Service<Request<B>> for GrpcRateLimitService<S>
where
    S: Service<Request<B>, Response = Response<tonic::body::BoxBody>> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B: Send + 'static,
{
    type Response = Response<tonic::body::BoxBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let token = request
            .headers()
            .get("x-claw-session")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("anonymous")
            .to_string();

        if let Err(not_until) = self.state.grpc_limiter.check_key(&token) {
            let mut response = Response::new(empty_body());
            *response.status_mut() = StatusCode::OK;
            response.headers_mut().insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("application/grpc"),
            );
            response.headers_mut().insert(
                http::HeaderName::from_static("grpc-status"),
                http::HeaderValue::from_static("8"),
            );
            response.headers_mut().insert(
                http::HeaderName::from_static("grpc-message"),
                http::HeaderValue::from_static("rate limit exceeded"),
            );
            if let Ok(value) =
                http::HeaderValue::from_str(&AppState::retry_after_seconds(&not_until).to_string())
            {
                response
                    .headers_mut()
                    .insert(http::HeaderName::from_static("retry-delay"), value);
            }
            return Box::pin(async move { Ok(response) });
        }

        Box::pin(self.inner.call(request))
    }
}

pub async fn serve(
    listener: TcpListener,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> Result<()> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .context("failed to build gRPC reflection service")?;

    let cert_path = std::env::var("CLAW_TLS_CERT_PATH").ok();
    let key_path = std::env::var("CLAW_TLS_KEY_PATH").ok();

    let builder = match (cert_path, key_path) {
        (Some(cert_path), Some(key_path)) => {
            let cert = tokio::fs::read(cert_path)
                .await
                .context("failed to read TLS certificate")?;
            let key = tokio::fs::read(key_path)
                .await
                .context("failed to read TLS key")?;
            let identity = Identity::from_pem(cert, key);
            Server::builder()
                .tls_config(ServerTlsConfig::new().identity(identity))
                .context("failed to configure gRPC TLS")?
        }
        _ => {
            tracing::warn!(
                "TLS not configured — use CLAW_TLS_CERT_PATH and CLAW_TLS_KEY_PATH for production."
            );
            Server::builder()
        }
    };

    builder
        .layer(ServiceBuilder::new().layer(GrpcRateLimitLayer {
            state: state.clone(),
        }))
        .add_service(reflection)
        .add_service(ClawDbServiceServer::new(ClawDbServiceImpl::new(state)))
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            async move {
                shutdown.cancelled().await;
            },
        )
        .await
        .context("gRPC server failed")
}

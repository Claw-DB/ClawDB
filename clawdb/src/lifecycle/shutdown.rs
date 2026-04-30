//! `GracefulShutdown`: signal handling and cooperative cancellation.

use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Manages graceful shutdown via OS signals and a `CancellationToken`.
pub struct GracefulShutdown {
    token: CancellationToken,
    timeout: Duration,
}

impl GracefulShutdown {
    /// Creates a new `GracefulShutdown` with the given timeout in seconds.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            token: CancellationToken::new(),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Blocks until SIGTERM, SIGINT, or SIGHUP is received, then cancels the token.
    pub async fn wait_for_signal(&self) {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
            let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");
            let mut sighup = signal(SignalKind::hangup()).expect("SIGHUP handler");

            tokio::select! {
                _ = sigterm.recv() => tracing::info!("received SIGTERM"),
                _ = sigint.recv()  => tracing::info!("received SIGINT"),
                _ = sighup.recv()  => tracing::info!("received SIGHUP"),
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.expect("ctrl-c handler");
            tracing::info!("received Ctrl-C");
        }

        self.token.cancel();
    }

    /// Returns a clone of the underlying `CancellationToken`.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Cancels the token immediately with the given reason.
    pub async fn initiate(&self, reason: &str) {
        tracing::info!(reason, "initiating graceful shutdown");
        self.token.cancel();
    }

    /// Returns the configured shutdown timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

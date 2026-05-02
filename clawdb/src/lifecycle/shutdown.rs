//! Graceful shutdown helpers.

/// Waits for OS shutdown signals.
pub struct GracefulShutdown {
    timeout_secs: u64,
}

impl GracefulShutdown {
    /// Creates a new graceful shutdown helper.
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Waits for CTRL-C or SIGTERM.
    pub async fn wait_for_signal(&self) {
        let _ = self.timeout_secs;
        #[cfg(unix)]
        {
            if let Ok(mut term) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            } else {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

//! Signal handling for graceful daemon shutdown.
//!
//! Listens for SIGTERM and SIGINT on Unix platforms and Ctrl+C on all
//! platforms. When a signal is received, the async function returns so the
//! caller can begin its shutdown sequence.

use tracing::info;

/// Wait for a shutdown signal (SIGTERM, SIGINT, or Ctrl+C).
///
/// This function resolves once any termination signal is received.
pub async fn wait_for_shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("received SIGINT (Ctrl+C)");
        }
        _ = terminate => {
            info!("received SIGTERM");
        }
    }
}

//! P2P Punch Hole Implementation
//!
//! This module implements TCP direct connection attempts with relay fallback.
//! When a PunchHole request is received, we try to connect directly to the peer.
//! If the direct connection fails (timeout), we fall back to relay.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use super::connection::ConnectionManager;

/// Timeout for direct TCP connection attempt
const DIRECT_CONNECT_TIMEOUT_MS: u64 = 3000;

/// Result of a punch hole attempt
#[derive(Debug)]
pub enum PunchResult {
    /// Direct connection succeeded
    DirectConnection(TcpStream),
    /// Direct connection failed, should use relay
    NeedRelay,
}

/// Attempt direct TCP connection to peer
///
/// This is a simplified P2P approach:
/// 1. Try to connect directly to the peer's address
/// 2. If successful within timeout, use direct connection
/// 3. If failed or timeout, fall back to relay
pub async fn try_direct_connection(peer_addr: SocketAddr) -> PunchResult {
    info!("Attempting direct TCP connection to {}", peer_addr);

    match tokio::time::timeout(
        Duration::from_millis(DIRECT_CONNECT_TIMEOUT_MS),
        TcpStream::connect(peer_addr),
    )
    .await
    {
        Ok(Ok(stream)) => {
            info!("Direct TCP connection to {} succeeded", peer_addr);
            PunchResult::DirectConnection(stream)
        }
        Ok(Err(e)) => {
            debug!("Direct TCP connection to {} failed: {}", peer_addr, e);
            PunchResult::NeedRelay
        }
        Err(_) => {
            debug!("Direct TCP connection to {} timed out", peer_addr);
            PunchResult::NeedRelay
        }
    }
}

/// Punch hole handler that tries direct connection first, then falls back to relay
pub struct PunchHoleHandler {
    connection_manager: Arc<ConnectionManager>,
}

impl PunchHoleHandler {
    pub fn new(connection_manager: Arc<ConnectionManager>) -> Self {
        Self { connection_manager }
    }

    /// Handle punch hole request
    ///
    /// Tries direct connection first, falls back to relay if needed.
    /// Returns true if direct connection succeeded, false if relay is needed.
    pub async fn handle_punch_hole(
        &self,
        peer_addr: Option<SocketAddr>,
    ) -> bool {
        let peer_addr = match peer_addr {
            Some(addr) => addr,
            None => {
                warn!("No peer address available for punch hole");
                return false;
            }
        };

        match try_direct_connection(peer_addr).await {
            PunchResult::DirectConnection(stream) => {
                // Direct connection succeeded, accept it
                match self.connection_manager.accept_connection(stream, peer_addr).await {
                    Ok(_) => {
                        info!("P2P direct connection established with {}", peer_addr);
                        true
                    }
                    Err(e) => {
                        warn!("Failed to accept direct connection: {}", e);
                        false
                    }
                }
            }
            PunchResult::NeedRelay => {
                debug!("Direct connection failed, need relay for {}", peer_addr);
                false
            }
        }
    }
}

/// Spawn a punch hole attempt with relay fallback
///
/// This function spawns an async task that:
/// 1. Tries direct TCP connection to peer
/// 2. If successful, accepts the connection
/// 3. If failed, calls the relay callback
pub fn spawn_punch_with_fallback<F>(
    connection_manager: Arc<ConnectionManager>,
    peer_addr: Option<SocketAddr>,
    relay_callback: F,
) where
    F: FnOnce() + Send + 'static,
{
    tokio::spawn(async move {
        let handler = PunchHoleHandler::new(connection_manager);

        if !handler.handle_punch_hole(peer_addr).await {
            // Direct connection failed, use relay
            info!("Falling back to relay connection");
            relay_callback();
        }
    });
}

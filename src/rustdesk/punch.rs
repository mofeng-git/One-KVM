//! Direct TCP attempt before relay fallback.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tracing::{debug, info};

const DIRECT_CONNECT_TIMEOUT_MS: u64 = 3000;

#[derive(Debug)]
pub enum PunchResult {
    DirectConnection(TcpStream),
    NeedRelay,
}

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

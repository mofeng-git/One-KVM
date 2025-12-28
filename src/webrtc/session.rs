//! WebRTC session management

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::config::WebRtcConfig;
use super::peer::PeerConnection;
use super::signaling::{IceCandidate, SdpAnswer, SdpOffer};
use crate::error::{AppError, Result};

/// Maximum concurrent WebRTC sessions
const MAX_SESSIONS: usize = 8;

/// WebRTC session info
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at: std::time::Instant,
    pub state: String,
}

/// WebRTC session manager
pub struct WebRtcSessionManager {
    config: WebRtcConfig,
    sessions: Arc<RwLock<HashMap<String, Arc<PeerConnection>>>>,
}

impl WebRtcSessionManager {
    /// Create a new session manager
    pub fn new(config: WebRtcConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(WebRtcConfig::default())
    }

    /// Create a new WebRTC session
    pub async fn create_session(&self) -> Result<String> {
        let sessions = self.sessions.read().await;

        // Check session limit
        if sessions.len() >= MAX_SESSIONS {
            return Err(AppError::WebRtcError(format!(
                "Maximum sessions ({}) reached",
                MAX_SESSIONS
            )));
        }
        drop(sessions);

        // Generate session ID
        let session_id = uuid::Uuid::new_v4().to_string();

        // Create new peer connection
        let pc = PeerConnection::new(&self.config, session_id.clone()).await?;

        // Store session
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), Arc::new(pc));

        info!("WebRTC session created: {}", session_id);
        Ok(session_id)
    }

    /// Handle SDP offer and return answer
    pub async fn handle_offer(&self, session_id: &str, offer: SdpOffer) -> Result<SdpAnswer> {
        let sessions = self.sessions.read().await;
        let pc = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?
            .clone();
        drop(sessions);

        pc.handle_offer(offer).await
    }

    /// Add ICE candidate
    pub async fn add_ice_candidate(&self, session_id: &str, candidate: IceCandidate) -> Result<()> {
        let sessions = self.sessions.read().await;
        let pc = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?
            .clone();
        drop(sessions);

        pc.add_ice_candidate(candidate).await
    }

    /// Get session info
    pub async fn get_session(&self, session_id: &str) -> Option<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|pc| SessionInfo {
            session_id: pc.session_id.clone(),
            created_at: std::time::Instant::now(), // TODO: store actual time
            state: format!("{:?}", pc.state()),
        })
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(pc) = sessions.remove(session_id) {
            info!("WebRTC session closed: {}", session_id);
            pc.close().await?;
        }

        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .map(|pc| SessionInfo {
                session_id: pc.session_id.clone(),
                created_at: std::time::Instant::now(),
                state: format!("{:?}", pc.state()),
            })
            .collect()
    }

    /// Clean up disconnected sessions
    pub async fn cleanup_stale_sessions(&self) {
        let sessions_to_remove: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter(|(_, pc)| {
                    matches!(
                        pc.state(),
                        super::signaling::ConnectionState::Disconnected
                            | super::signaling::ConnectionState::Failed
                            | super::signaling::ConnectionState::Closed
                    )
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        if !sessions_to_remove.is_empty() {
            let mut sessions = self.sessions.write().await;
            for id in sessions_to_remove {
                debug!("Removing stale WebRTC session: {}", id);
                sessions.remove(&id);
            }
        }
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Start video streaming to a session
    pub async fn start_video(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        let _pc = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?
            .clone();
        drop(sessions);

        // Video track should already be added during peer creation
        // This is a placeholder for additional video control logic
        info!("Video streaming started for session: {}", session_id);
        Ok(())
    }

    /// Stop video streaming to a session
    pub async fn stop_video(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        let _pc = sessions
            .get(session_id)
            .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?
            .clone();
        drop(sessions);

        // Placeholder for video stop logic
        info!("Video streaming stopped for session: {}", session_id);
        Ok(())
    }
}

impl Default for WebRtcSessionManager {
    fn default() -> Self {
        Self::default_config()
    }
}

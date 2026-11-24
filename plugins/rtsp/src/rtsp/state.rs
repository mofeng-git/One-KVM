use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[derive(Default, Clone)]
pub(crate) struct ParameterSets {
    pub h264_sps: Option<Bytes>,
    pub h264_pps: Option<Bytes>,
    pub h265_vps: Option<Bytes>,
    pub h265_sps: Option<Bytes>,
    pub h265_pps: Option<Bytes>,
}

#[derive(Clone)]
pub(crate) struct SharedRtspState {
    pub active_client: Arc<Mutex<Option<SocketAddr>>>,
    pub parameter_sets: Arc<RwLock<ParameterSets>>,
}

impl SharedRtspState {
    pub fn new() -> Self {
        Self {
            active_client: Arc::new(Mutex::new(None)),
            parameter_sets: Arc::new(RwLock::new(ParameterSets::default())),
        }
    }
}

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::audio::AudioController;
use crate::config::{AppConfig, ConfigStore, RtspConfig, VncConfig};
use crate::error::{AppError, Result};
use crate::hid::HidController;
use crate::rtsp::{RtspService, RtspServiceStatus};
use crate::rustdesk::config::RustDeskConfig;
use crate::rustdesk::RustDeskService;
use crate::video::codec_constraints::{
    enforce_constraints_with_stream_manager, validate_third_party_codec_compatibility,
    StreamCodecConstraints,
};
use crate::video::VideoStreamManager;
use crate::vnc::{VncService, VncServiceStatus};

use super::ConfigApplyOptions;

#[derive(Debug, Clone)]
pub struct RustDeskRuntimeStatus {
    pub service_status: String,
    pub rendezvous_status: Option<String>,
    pub connection_count: usize,
    pub listening: bool,
    pub listen_port: Option<u16>,
}

pub struct RemoteAccessCoordinator {
    config: ConfigStore,
    stream_manager: Arc<VideoStreamManager>,
    hid: Arc<HidController>,
    audio: Arc<AudioController>,
    rustdesk: RwLock<Option<Arc<RustDeskService>>>,
    vnc: RwLock<Option<Arc<VncService>>>,
    rtsp: RwLock<Option<Arc<RtspService>>>,
}

impl RemoteAccessCoordinator {
    pub fn new(
        config: ConfigStore,
        stream_manager: Arc<VideoStreamManager>,
        hid: Arc<HidController>,
        audio: Arc<AudioController>,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            stream_manager,
            hid,
            audio,
            rustdesk: RwLock::new(None),
            vnc: RwLock::new(None),
            rtsp: RwLock::new(None),
        })
    }

    pub async fn start_configured(&self, config: &AppConfig) {
        if let Err(error) = validate_third_party_codec_compatibility(config) {
            tracing::warn!(
                "Third-party access codec configuration is invalid; RustDesk/VNC/RTSP will not start: {}",
                error
            );
            return;
        }

        if config.rustdesk.is_valid() {
            if let Err(error) = self
                .apply_rustdesk(
                    &RustDeskConfig::default(),
                    &config.rustdesk,
                    ConfigApplyOptions::default(),
                )
                .await
            {
                tracing::error!("Failed to start RustDesk service: {}", error);
            }
        } else if config.rustdesk.enabled {
            tracing::warn!(
                "RustDesk enabled but configuration is incomplete (missing server or credentials)"
            );
        } else {
            tracing::info!("RustDesk disabled in configuration");
        }

        if config.vnc.enabled {
            if let Err(error) = self
                .apply_vnc(
                    &VncConfig::default(),
                    &config.vnc,
                    ConfigApplyOptions::default(),
                )
                .await
            {
                tracing::error!("Failed to start VNC service: {}", error);
            }
        } else {
            tracing::info!("VNC disabled in configuration");
        }

        if config.rtsp.enabled {
            if let Err(error) = self
                .apply_rtsp(
                    &RtspConfig::default(),
                    &config.rtsp,
                    ConfigApplyOptions::default(),
                )
                .await
            {
                tracing::error!("Failed to start RTSP service: {}", error);
            }
        } else {
            tracing::info!("RTSP disabled in configuration");
        }

        if let Err(error) = self.enforce_codec_constraints().await {
            tracing::warn!("Failed to enforce startup codec constraints: {}", error);
        }
    }

    pub async fn runtime_config(&self) -> AppConfig {
        let mut config = self.config.get().as_ref().clone();
        let rustdesk = self.rustdesk.read().await.clone();
        let vnc = self.vnc.read().await.clone();
        let rtsp = self.rtsp.read().await.clone();

        config.rustdesk.enabled = rustdesk.is_some_and(|service| service.is_running());
        config.vnc.enabled = match vnc {
            Some(service) => matches!(
                service.status().await,
                VncServiceStatus::Starting | VncServiceStatus::Running
            ),
            None => false,
        };
        config.rtsp.enabled = match rtsp {
            Some(service) => matches!(
                service.status().await,
                RtspServiceStatus::Starting | RtspServiceStatus::Running
            ),
            None => false,
        };
        config
    }

    pub async fn rustdesk_status(&self) -> RustDeskRuntimeStatus {
        let service = self.rustdesk.read().await.clone();
        match service {
            Some(service) => RustDeskRuntimeStatus {
                service_status: service.status().to_string(),
                rendezvous_status: service.rendezvous_status().map(|status| status.to_string()),
                connection_count: service.connection_count(),
                listening: service.is_listening(),
                listen_port: service.is_listening().then(|| service.listen_port()),
            },
            None => RustDeskRuntimeStatus {
                service_status: "not_initialized".to_string(),
                rendezvous_status: None,
                connection_count: 0,
                listening: false,
                listen_port: None,
            },
        }
    }

    pub async fn vnc_status(&self) -> (VncServiceStatus, usize) {
        let service = self.vnc.read().await.clone();
        match service {
            Some(service) => (service.status().await, service.connection_count()),
            None => (VncServiceStatus::Stopped, 0),
        }
    }

    pub async fn rtsp_status(&self) -> RtspServiceStatus {
        let service = self.rtsp.read().await.clone();
        match service {
            Some(service) => service.status().await,
            None => RtspServiceStatus::Stopped,
        }
    }

    pub async fn enforce_codec_constraints(&self) -> Result<Option<String>> {
        let config = self.runtime_config().await;
        let constraints = StreamCodecConstraints::from_config(&config);
        self.stream_manager
            .set_runtime_codec_constraints(constraints.clone())
            .await;
        let enforcement =
            enforce_constraints_with_stream_manager(&self.stream_manager, &constraints).await?;
        Ok(enforcement.message)
    }

    pub async fn apply_rustdesk(
        &self,
        old_config: &RustDeskConfig,
        new_config: &RustDeskConfig,
        options: ConfigApplyOptions,
    ) -> Result<()> {
        tracing::info!("Applying RustDesk config changes...");
        self.validate_rustdesk_candidate(new_config, options.runtime_only)
            .await?;

        let need_restart = options.force
            || old_config.mode != new_config.mode
            || old_config.codec != new_config.codec
            || old_config.direct_access_port != new_config.direct_access_port
            || old_config.rendezvous_server != new_config.rendezvous_server
            || old_config.relay_server != new_config.relay_server
            || old_config.relay_key != new_config.relay_key
            || old_config.device_id != new_config.device_id
            || old_config.device_password != new_config.device_password;
        let current = self.rustdesk.read().await.clone();
        let mut credentials_to_save = None;

        if !options.preserve_service_state && !new_config.enabled {
            if let Some(service) = current.as_ref() {
                service.stop().await.map_err(|error| {
                    AppError::Config(format!("Failed to stop RustDesk service: {error}"))
                })?;
                tracing::info!("RustDesk service stopped");
            }
            *self.rustdesk.write().await = None;
        } else if !options.preserve_service_state && new_config.enabled {
            match current {
                None => {
                    tracing::info!("Initializing RustDesk service...");
                    let service = Arc::new(RustDeskService::new(
                        new_config.clone(),
                        self.stream_manager.clone(),
                        self.hid.clone(),
                        self.audio.clone(),
                    ));
                    *self.rustdesk.write().await = Some(service.clone());
                    service.start().await.map_err(|error| {
                        AppError::Config(format!("Failed to start RustDesk service: {error}"))
                    })?;
                    tracing::info!("RustDesk service started with ID: {}", new_config.device_id);
                    credentials_to_save = service.save_credentials();
                }
                Some(service) => {
                    if service.is_running() {
                        if need_restart {
                            service.restart(new_config.clone()).await.map_err(|error| {
                                AppError::Config(format!(
                                    "Failed to restart RustDesk service: {error}"
                                ))
                            })?;
                            tracing::info!(
                                "RustDesk service restarted with ID: {}",
                                new_config.device_id
                            );
                        }
                    } else {
                        service.update_config(new_config.clone());
                        service.start().await.map_err(|error| {
                            AppError::Config(format!("Failed to start RustDesk service: {error}"))
                        })?;
                    }
                    credentials_to_save = service.save_credentials();
                }
            }
        } else if options.preserve_service_state && need_restart {
            if let Some(service) = current {
                let mut runtime_config = new_config.clone();
                runtime_config.enabled = true;
                service.restart(runtime_config).await.map_err(|error| {
                    AppError::Config(format!("Failed to restart RustDesk service: {error}"))
                })?;
                credentials_to_save = service.save_credentials();
            }
        }

        if let Some(updated) = credentials_to_save {
            tracing::info!("Saving RustDesk credentials to config store...");
            self.config
                .update(|config| {
                    config.rustdesk.public_key = updated.public_key.clone();
                    config.rustdesk.private_key = updated.private_key.clone();
                    config.rustdesk.signing_public_key = updated.signing_public_key.clone();
                    config.rustdesk.signing_private_key = updated.signing_private_key.clone();
                    config.rustdesk.uuid = updated.uuid.clone();
                })
                .await?;
            tracing::info!("RustDesk credentials saved successfully");
        }

        self.log_enforced_constraints().await?;
        Ok(())
    }

    pub async fn apply_vnc(
        &self,
        old_config: &VncConfig,
        new_config: &VncConfig,
        options: ConfigApplyOptions,
    ) -> Result<()> {
        tracing::info!("Applying VNC config changes...");
        self.validate_vnc_candidate(new_config, options.runtime_only)
            .await?;

        let runtime_config = self.runtime_config().await;
        let will_run = if options.preserve_service_state {
            runtime_config.vnc.enabled
        } else {
            new_config.enabled
        };
        if will_run {
            let mut candidate = runtime_config;
            candidate.vnc = new_config.clone();
            candidate.vnc.enabled = true;
            let constraints = StreamCodecConstraints::from_config(&candidate);
            match enforce_constraints_with_stream_manager(&self.stream_manager, &constraints).await
            {
                Ok(result) if result.changed => {
                    if let Some(message) = result.message {
                        tracing::info!("{}", message);
                    }
                }
                Ok(_) => {}
                Err(error) => tracing::warn!(
                    "Failed to enforce VNC stream constraints before start: {}",
                    error
                ),
            }
        }

        let need_restart = options.force
            || old_config.bind != new_config.bind
            || old_config.port != new_config.port
            || old_config.encoding != new_config.encoding
            || old_config.password != new_config.password
            || old_config.allow_one_client != new_config.allow_one_client;
        let current = self.vnc.read().await.clone();

        if !options.preserve_service_state && !new_config.enabled {
            if let Some(service) = current.as_ref() {
                service.stop().await?;
            }
            *self.vnc.write().await = None;
        } else if !options.preserve_service_state && new_config.enabled {
            match current {
                None => {
                    let service = Arc::new(VncService::new(
                        new_config.clone(),
                        self.stream_manager.clone(),
                        self.hid.clone(),
                    ));
                    *self.vnc.write().await = Some(service.clone());
                    service.start().await?;
                    tracing::info!("VNC service started");
                }
                Some(service) => {
                    if matches!(service.status().await, VncServiceStatus::Running) {
                        if need_restart {
                            service.restart(new_config.clone()).await?;
                            tracing::info!("VNC service restarted");
                        }
                    } else {
                        service.update_config(new_config.clone()).await;
                        service.start().await?;
                    }
                }
            }
        } else if options.preserve_service_state && need_restart {
            if let Some(service) = current {
                let mut runtime_config = new_config.clone();
                runtime_config.enabled = true;
                service.restart(runtime_config).await?;
            }
        }

        self.log_enforced_constraints().await?;
        Ok(())
    }

    pub async fn apply_rtsp(
        &self,
        old_config: &RtspConfig,
        new_config: &RtspConfig,
        options: ConfigApplyOptions,
    ) -> Result<()> {
        tracing::info!("Applying RTSP config changes...");
        self.validate_rtsp_candidate(new_config, options.runtime_only)
            .await?;

        let need_restart = options.force
            || old_config.bind != new_config.bind
            || old_config.port != new_config.port
            || old_config.path != new_config.path
            || old_config.codec != new_config.codec
            || old_config.username != new_config.username
            || old_config.password != new_config.password
            || old_config.allow_one_client != new_config.allow_one_client;
        let current = self.rtsp.read().await.clone();

        if !options.preserve_service_state && !new_config.enabled {
            if let Some(service) = current.as_ref() {
                service.stop().await.map_err(|error| {
                    AppError::Config(format!("Failed to stop RTSP service: {error}"))
                })?;
            }
            *self.rtsp.write().await = None;
        } else if !options.preserve_service_state && new_config.enabled {
            match current {
                None => {
                    let service = Arc::new(RtspService::new(
                        new_config.clone(),
                        self.stream_manager.clone(),
                    ));
                    *self.rtsp.write().await = Some(service.clone());
                    service.start().await?;
                    tracing::info!("RTSP service started");
                }
                Some(service) => {
                    if matches!(service.status().await, RtspServiceStatus::Running) {
                        if need_restart {
                            service.restart(new_config.clone()).await?;
                            tracing::info!("RTSP service restarted");
                        }
                    } else {
                        service.update_config(new_config.clone()).await;
                        service.start().await?;
                    }
                }
            }
        } else if options.preserve_service_state && need_restart {
            if let Some(service) = current {
                let mut runtime_config = new_config.clone();
                runtime_config.enabled = true;
                service.restart(runtime_config).await?;
            }
        }

        self.log_enforced_constraints().await?;
        Ok(())
    }

    pub async fn shutdown(&self) {
        let rustdesk = self.rustdesk.write().await.take();
        let vnc = self.vnc.write().await.take();
        let rtsp = self.rtsp.write().await.take();

        if let Some(service) = rustdesk {
            if let Err(error) = service.stop().await {
                tracing::warn!("Failed to stop RustDesk service: {}", error);
            } else {
                tracing::info!("RustDesk service stopped");
            }
        }
        if let Some(service) = vnc {
            if let Err(error) = service.stop().await {
                tracing::warn!("Failed to stop VNC service: {}", error);
            } else {
                tracing::info!("VNC service stopped");
            }
        }
        if let Some(service) = rtsp {
            if let Err(error) = service.stop().await {
                tracing::warn!("Failed to stop RTSP service: {}", error);
            } else {
                tracing::info!("RTSP service stopped");
            }
        }
    }

    async fn validate_rustdesk_candidate(
        &self,
        new_config: &RustDeskConfig,
        runtime_only: bool,
    ) -> Result<()> {
        let mut candidate = self.candidate_config(runtime_only).await;
        candidate.rustdesk = new_config.clone();
        validate_third_party_codec_compatibility(&candidate)
    }

    async fn validate_vnc_candidate(
        &self,
        new_config: &VncConfig,
        runtime_only: bool,
    ) -> Result<()> {
        let mut candidate = self.candidate_config(runtime_only).await;
        candidate.vnc = new_config.clone();
        validate_third_party_codec_compatibility(&candidate)
    }

    async fn validate_rtsp_candidate(
        &self,
        new_config: &RtspConfig,
        runtime_only: bool,
    ) -> Result<()> {
        let mut candidate = self.candidate_config(runtime_only).await;
        candidate.rtsp = new_config.clone();
        validate_third_party_codec_compatibility(&candidate)
    }

    async fn candidate_config(&self, runtime_only: bool) -> AppConfig {
        if runtime_only {
            self.runtime_config().await
        } else {
            self.config.get().as_ref().clone()
        }
    }

    async fn log_enforced_constraints(&self) -> Result<()> {
        if let Some(message) = self.enforce_codec_constraints().await? {
            tracing::info!("{}", message);
        }
        Ok(())
    }
}

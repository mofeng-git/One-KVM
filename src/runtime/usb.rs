use std::path::PathBuf;
use std::sync::Arc;

#[cfg(unix)]
use tokio::sync::RwLock;

use crate::config::{AppConfig, HidBackend, HidConfig, MsdConfig, OtgNetworkConfig, UacConfig};
use crate::error::{AppError, Result};
use crate::events::EventBus;
use crate::hid::{HidBackendType, HidController};
#[cfg(unix)]
use crate::msd::MsdController;
#[cfg(unix)]
use crate::otg::OtgService;

use super::ConfigApplyOptions;

pub struct UsbCoordinator {
    hid: Arc<HidController>,
    #[cfg(unix)]
    otg: Arc<OtgService>,
    #[cfg(unix)]
    msd: Arc<RwLock<Option<MsdController>>>,
    #[cfg(unix)]
    uac_playback: Arc<RwLock<Option<crate::audio::uac::UacPlayback>>>,
    events: Arc<EventBus>,
    data_dir: PathBuf,
}

impl UsbCoordinator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hid: Arc<HidController>,
        #[cfg(unix)] otg: Arc<OtgService>,
        #[cfg(unix)] msd: Arc<RwLock<Option<MsdController>>>,
        #[cfg(unix)] uac_playback: Arc<RwLock<Option<crate::audio::uac::UacPlayback>>>,
        events: Arc<EventBus>,
        data_dir: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            hid,
            #[cfg(unix)]
            otg,
            #[cfg(unix)]
            msd,
            #[cfg(unix)]
            uac_playback,
            events,
            data_dir,
        })
    }

    pub async fn apply_config(&self, old_config: &AppConfig, new_config: &AppConfig) -> Result<()> {
        #[cfg(unix)]
        {
            let transitioning_away_from_otg = old_config.hid.backend == HidBackend::Otg
                && new_config.hid.backend != HidBackend::Otg;
            let hid_unchanged = old_config.hid == new_config.hid;
            let gadget_rebuilt = old_config.msd != new_config.msd
                || old_config.otg_network != new_config.otg_network
                || old_config.uac != new_config.uac
                || old_config.hid.otg_udc != new_config.hid.otg_udc
                || old_config.hid.otg_descriptor != new_config.hid.otg_descriptor
                || old_config.hid.backend != new_config.hid.backend
                || old_config.hid.constrained_otg_functions()
                    != new_config.hid.constrained_otg_functions()
                || old_config.hid.effective_otg_keyboard_leds()
                    != new_config.hid.effective_otg_keyboard_leds();
            let restart_uac =
                old_config.uac != new_config.uac || (new_config.uac.enabled && gadget_rebuilt);

            if restart_uac {
                let playback = self.uac_playback.write().await.take();
                if let Some(playback) = playback {
                    playback.stop();
                    tracing::info!("UAC playback writer stopped before OTG reconcile");
                }
            }

            if transitioning_away_from_otg {
                self.apply_hid(
                    &old_config.hid,
                    &new_config.hid,
                    &new_config.msd,
                    &new_config.otg_network,
                    &new_config.uac,
                    ConfigApplyOptions::default(),
                )
                .await?;
            } else {
                self.reconcile_otg(
                    &new_config.hid,
                    &new_config.msd,
                    &new_config.otg_network,
                    &new_config.uac,
                )
                .await?;
                self.apply_hid(
                    &old_config.hid,
                    &new_config.hid,
                    &new_config.msd,
                    &new_config.otg_network,
                    &new_config.uac,
                    ConfigApplyOptions::default(),
                )
                .await?;
            }

            if hid_unchanged && gadget_rebuilt && new_config.hid.backend == HidBackend::Otg {
                tracing::info!("OTG gadget rebuilt, reloading HID backend for new devices");
                self.hid
                    .reload(hid_backend_type(&new_config.hid))
                    .await
                    .map_err(|error| {
                        AppError::Config(format!("HID reload after gadget rebuild failed: {error}"))
                    })?;
            }

            self.apply_msd(
                &old_config.msd,
                &new_config.msd,
                &new_config.hid,
                &new_config.otg_network,
                &new_config.uac,
                ConfigApplyOptions::default(),
            )
            .await?;

            if restart_uac && new_config.uac.enabled {
                let config = crate::audio::uac::UacPlaybackConfig {
                    sample_rate: new_config.uac.sample_rate,
                    channels: new_config.uac.channels as u16,
                    ..Default::default()
                };
                let writer = crate::audio::uac::UacPlayback::start(config).map_err(|error| {
                    AppError::Config(format!("Failed to start UAC playback: {error}"))
                })?;
                *self.uac_playback.write().await = Some(writer);
                tracing::info!("UAC playback writer started after OTG reconcile");
            } else if restart_uac {
                tracing::info!("UAC playback remains disabled");
            }

            Ok(())
        }

        #[cfg(not(unix))]
        {
            self.apply_hid(
                &old_config.hid,
                &new_config.hid,
                &new_config.msd,
                &new_config.otg_network,
                &new_config.uac,
                ConfigApplyOptions::default(),
            )
            .await
        }
    }

    async fn apply_hid(
        &self,
        old_config: &HidConfig,
        new_config: &HidConfig,
        msd_config: &MsdConfig,
        network_config: &OtgNetworkConfig,
        uac_config: &UacConfig,
        options: ConfigApplyOptions,
    ) -> Result<()> {
        new_config.validate_otg_functions()?;
        new_config.bluetooth.validate()?;

        let descriptor_changed = old_config.otg_descriptor != new_config.otg_descriptor;
        let hid_functions_changed =
            old_config.constrained_otg_functions() != new_config.constrained_otg_functions();
        let keyboard_leds_changed =
            old_config.effective_otg_keyboard_leds() != new_config.effective_otg_keyboard_leds();
        let ch9329_runtime_changed = old_config.ch9329_hybrid_mouse != new_config.ch9329_hybrid_mouse
            || old_config.ch9329_macos_drag != new_config.ch9329_macos_drag;

        if old_config.backend == new_config.backend
            && old_config.ch9329_port == new_config.ch9329_port
            && old_config.ch9329_baudrate == new_config.ch9329_baudrate
            && old_config.bluetooth == new_config.bluetooth
            && !ch9329_runtime_changed
            && old_config.otg_udc == new_config.otg_udc
            && !descriptor_changed
            && !hid_functions_changed
            && !keyboard_leds_changed
            && !options.force
        {
            tracing::info!("HID config unchanged, skipping reload");
            return Ok(());
        }

        tracing::info!("Applying HID config changes...");
        let backend = hid_backend_type(new_config);
        let transitioning_away_from_otg =
            old_config.backend == HidBackend::Otg && new_config.backend != HidBackend::Otg;
        let otg_changed = hid_otg_config_changed(old_config, new_config);

        if transitioning_away_from_otg {
            self.hid
                .reload(backend.clone())
                .await
                .map_err(|error| AppError::Config(format!("HID reload failed: {error}")))?;
        }
        if otg_changed {
            self.reconcile_otg(new_config, msd_config, network_config, uac_config)
                .await?;
        }
        if !transitioning_away_from_otg {
            self.hid
                .reload(backend)
                .await
                .map_err(|error| AppError::Config(format!("HID reload failed: {error}")))?;
        }

        tracing::info!(
            "HID backend reloaded successfully: {:?}",
            new_config.backend
        );
        Ok(())
    }

    async fn reconcile_otg(
        &self,
        hid: &HidConfig,
        msd: &MsdConfig,
        network: &OtgNetworkConfig,
        uac: &UacConfig,
    ) -> Result<()> {
        #[cfg(unix)]
        {
            self.otg
                .apply_config(hid, msd, network, uac)
                .await
                .map_err(|error| AppError::Config(format!("OTG reconcile failed: {error}")))
        }
        #[cfg(not(unix))]
        {
            let _ = (hid, msd, network, uac);
            Ok(())
        }
    }

    #[cfg(unix)]
    async fn apply_msd(
        &self,
        old_config: &MsdConfig,
        new_config: &MsdConfig,
        hid_config: &HidConfig,
        network_config: &OtgNetworkConfig,
        uac_config: &UacConfig,
        options: ConfigApplyOptions,
    ) -> Result<()> {
        let old_enabled = old_config.enabled;
        let new_enabled = new_config.enabled && hid_config.backend == HidBackend::Otg;
        let directory_changed = old_config.msd_dir != new_config.msd_dir;
        let inquiry_changed = old_config.flash_inquiry_string != new_config.flash_inquiry_string
            || old_config.cdrom_inquiry_string != new_config.cdrom_inquiry_string;

        if !options.force && old_enabled == new_enabled && !directory_changed && !inquiry_changed {
            tracing::info!("MSD configuration unchanged, no reload needed");
            return Ok(());
        }

        if new_enabled {
            tracing::info!("(Re)initializing MSD...");
            self.reconcile_otg(hid_config, new_config, network_config, uac_config)
                .await?;

            let old_msd = self.msd.write().await.take();
            if let Some(msd) = old_msd {
                msd.shutdown()
                    .await
                    .map_err(|error| AppError::Config(format!("MSD shutdown failed: {error}")))?;
            }

            let msd = MsdController::new(self.otg.clone(), new_config.msd_dir_path());
            msd.init(&self.data_dir.join("ventoy"))
                .await
                .map_err(|error| AppError::Config(format!("MSD initialization failed: {error}")))?;
            msd.set_event_bus(self.events.clone()).await;
            *self.msd.write().await = Some(msd);
            tracing::info!("MSD initialized successfully");
        } else {
            tracing::info!("MSD disabled in config, shutting down...");
            let old_msd = self.msd.write().await.take();
            if let Some(msd) = old_msd {
                msd.shutdown()
                    .await
                    .map_err(|error| AppError::Config(format!("MSD shutdown failed: {error}")))?;
            }
            tracing::info!("MSD shutdown complete");
            self.reconcile_otg(hid_config, new_config, network_config, uac_config)
                .await?;
        }

        if hid_config.backend == HidBackend::Otg && (options.force || old_enabled != new_enabled) {
            self.hid
                .reload(HidBackendType::Otg)
                .await
                .map_err(|error| AppError::Config(format!("OTG HID reload failed: {error}")))?;
        }
        Ok(())
    }
}

fn hid_backend_type(config: &HidConfig) -> HidBackendType {
    match config.backend {
        HidBackend::Otg => HidBackendType::Otg,
        HidBackend::Ch9329 => HidBackendType::Ch9329 {
            port: config.ch9329_port.clone(),
            baud_rate: config.ch9329_baudrate,
            hybrid_mouse: config.ch9329_hybrid_mouse,
            macos_drag: config.ch9329_macos_drag,
        },
        HidBackend::None => HidBackendType::None,
        HidBackend::Bluetooth => HidBackendType::Bluetooth {
            config: config.bluetooth.clone(),
        },
    }
}

fn hid_otg_config_changed(old_config: &HidConfig, new_config: &HidConfig) -> bool {
    old_config.backend == HidBackend::Otg
        || new_config.backend == HidBackend::Otg
        || old_config.otg_udc != new_config.otg_udc
        || old_config.otg_descriptor != new_config.otg_descriptor
        || old_config.constrained_otg_functions() != new_config.constrained_otg_functions()
        || old_config.effective_otg_keyboard_leds() != new_config.effective_otg_keyboard_leds()
}

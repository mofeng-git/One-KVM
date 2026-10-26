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
        new_config.hid.validate_platform()?;
        #[cfg(unix)]
        {
            let transitioning_away_from_otg = old_config.hid.backend == HidBackend::Otg
                && new_config.hid.backend != HidBackend::Otg;
            let hid_unchanged = !hid_requires_reload(&old_config.hid, &new_config.hid);
            let gadget_rebuilt = crate::otg::service::OtgDesiredState::from_config(
                &old_config.hid,
                &old_config.msd,
                &old_config.otg_network,
                &old_config.uac,
            )? != crate::otg::service::OtgDesiredState::from_config(
                &new_config.hid,
                &new_config.msd,
                &new_config.otg_network,
                &new_config.uac,
            )?;
            let restart_uac =
                old_config.uac != new_config.uac || (new_config.uac.enabled && gadget_rebuilt);

            if gadget_rebuilt {
                // This is a no-op for CH9329, including the composite mode.
                self.hid.prepare_otg_rebuild().await?;
            }

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
        new_config.validate_platform()?;
        new_config.validate_otg_functions()?;
        new_config.bluetooth.validate()?;

        if !hid_requires_reload(old_config, new_config) && !options.force {
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
        let new_enabled = new_config.enabled && hid_config.backend.supports_otg_functions();
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
                .reload(hid_backend_type(hid_config))
                .await
                .map_err(|error| AppError::Config(format!("OTG HID reload failed: {error}")))?;
        }
        Ok(())
    }
}

fn hid_backend_type(config: &HidConfig) -> HidBackendType {
    HidBackendType::from(config)
}

fn hid_requires_reload(old: &HidConfig, new: &HidConfig) -> bool {
    HidBackendType::from(old) != HidBackendType::from(new)
        || (new.backend.uses_otg_hid() && hid_otg_config_changed(old, new))
}

fn hid_otg_config_changed(old: &HidConfig, new: &HidConfig) -> bool {
    old.backend.uses_otg_hid() != new.backend.uses_otg_hid()
        || old.otg_udc != new.otg_udc
        || old.otg_descriptor != new.otg_descriptor
        || old.constrained_otg_functions() != new.constrained_otg_functions()
        || old.effective_otg_keyboard_leds() != new.effective_otg_keyboard_leds()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_otg_preferences_do_not_reload_ch9329_but_input_changes_do() {
        let old = HidConfig {
            backend: HidBackend::Ch9329Otg,
            otg_udc: Some("first.udc".into()),
            ..Default::default()
        };
        let mut new = old.clone();
        new.otg_udc = Some("second.udc".into());
        new.otg_descriptor.product = "Advanced USB".into();
        new.otg_keyboard_leds = !old.otg_keyboard_leds;
        new.otg_profile = crate::config::OtgHidProfile::LegacyKeyboard;
        assert!(!hid_requires_reload(&old, &new));
        assert!(!hid_requires_reload(&new, &old));

        let input_changes: [fn(&mut HidConfig); 4] = [
            |hid: &mut HidConfig| hid.ch9329_port.push('1'),
            |hid: &mut HidConfig| hid.ch9329_baudrate = 115200,
            |hid: &mut HidConfig| hid.ch9329_hybrid_mouse = !hid.ch9329_hybrid_mouse,
            |hid: &mut HidConfig| hid.mouse_macos_drag = !hid.mouse_macos_drag,
        ];
        for change in input_changes {
            let mut changed = old.clone();
            change(&mut changed);
            assert!(hid_requires_reload(&old, &changed));
            assert!(hid_requires_reload(&changed, &old));
        }
    }

    #[test]
    fn composite_mode_switch_only_reloads_when_input_driver_changes() {
        let composite = HidConfig {
            backend: HidBackend::Ch9329Otg,
            otg_udc: Some("test.udc".into()),
            ..Default::default()
        };
        for (backend, reload) in [
            (HidBackend::Ch9329, false),
            (HidBackend::Otg, true),
            (HidBackend::Bluetooth, true),
            (HidBackend::None, true),
        ] {
            let mut other = composite.clone();
            other.backend = backend;
            assert_eq!(hid_requires_reload(&composite, &other), reload);
            assert_eq!(hid_requires_reload(&other, &composite), reload);
        }
    }
}

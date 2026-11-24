use one_kvm_plugin_sdk::runtime::Client;
pub use one_kvm_plugin_sdk::{keyboard::CanonicalKey, types::*};

pub struct HidController {
    client: Option<Client>,
    #[cfg(test)]
    recording: Option<std::sync::Arc<test_support::RecordingBackend>>,
}

impl HidController {
    pub fn new(client: Client) -> Self {
        Self {
            client: Some(client),
            #[cfg(test)]
            recording: None,
        }
    }

    pub async fn send_keyboard(&self, event: KeyboardEvent) -> anyhow::Result<()> {
        if let Some(client) = &self.client {
            client.send_keyboard(event).await?;
        }
        Ok(())
    }

    pub async fn send_mouse(&self, event: MouseEvent) -> anyhow::Result<()> {
        if let Some(client) = &self.client {
            return client.send_mouse(event).await;
        }
        #[cfg(test)]
        if let Some(recording) = &self.recording {
            recording.record(event);
        }
        Ok(())
    }
}

#[cfg(test)]
pub async fn test_controller(
    recording: std::sync::Arc<test_support::RecordingBackend>,
) -> std::sync::Arc<HidController> {
    std::sync::Arc::new(HidController {
        client: None,
        recording: Some(recording),
    })
}

#[cfg(test)]
pub mod test_support {
    use super::MouseEvent;
    use std::sync::Arc;

    #[derive(Default)]
    pub struct RecordingBackend {
        pub events: parking_lot::Mutex<Vec<MouseEvent>>,
        changed: tokio::sync::Notify,
    }

    impl RecordingBackend {
        pub fn new() -> Arc<Self> {
            Arc::default()
        }

        pub fn record(&self, event: MouseEvent) {
            self.events.lock().push(event);
            self.changed.notify_one();
        }

        pub async fn wait_for_events(&self, count: usize) {
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while self.events.lock().len() < count {
                    self.changed.notified().await;
                }
            })
            .await
            .unwrap();
        }
    }
}

//! Event bus: [`SystemEvent`] fan-out to WebSocket subscribers and internal tasks.

pub mod types;

use self::types::EXACT_EVENT_TOPICS;

pub use types::{
    AtxDeviceInfo, AudioDeviceInfo, ClientStats, HidDeviceInfo, LedState, MsdDeviceInfo,
    StreamDeviceLostKind, SystemEvent, TtydDeviceInfo, VideoDeviceInfo,
};

use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

fn collect_prefix_wildcards(exact: &[&'static str]) -> Vec<String> {
    use std::collections::BTreeSet;
    let mut segments = BTreeSet::new();
    for name in exact {
        if let Some((seg, _)) = name.split_once('.') {
            segments.insert(seg);
        }
    }
    segments.into_iter().map(|s| format!("{}.*", s)).collect()
}

fn make_sender() -> broadcast::Sender<SystemEvent> {
    let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    tx
}

fn topic_prefix(event_name: &str) -> Option<String> {
    event_name
        .split_once('.')
        .map(|(prefix, _)| format!("{}.*", prefix))
}

pub struct EventBus {
    tx: broadcast::Sender<SystemEvent>,
    exact_topics: std::collections::HashMap<&'static str, broadcast::Sender<SystemEvent>>,
    prefix_topics: std::collections::HashMap<String, broadcast::Sender<SystemEvent>>,
    device_info_dirty_tx: broadcast::Sender<()>,
}

impl EventBus {
    pub fn new() -> Self {
        let tx = make_sender();
        let exact_topics = EXACT_EVENT_TOPICS
            .iter()
            .map(|topic| (*topic, make_sender()))
            .collect();
        let prefix_topics = collect_prefix_wildcards(EXACT_EVENT_TOPICS)
            .into_iter()
            .map(|topic| (topic, make_sender()))
            .collect();
        let (device_info_dirty_tx, _dirty_rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        Self {
            tx,
            exact_topics,
            prefix_topics,
            device_info_dirty_tx,
        }
    }

    pub fn publish(&self, event: SystemEvent) {
        let event_name = event.event_name();

        if let Some(tx) = self.exact_topics.get(event_name) {
            let _ = tx.send(event.clone());
        }

        if let Some(prefix) = topic_prefix(event_name) {
            if let Some(tx) = self.prefix_topics.get(&prefix) {
                let _ = tx.send(event.clone());
            }
        }

        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }

    pub fn subscribe_topic(&self, topic: &str) -> Option<broadcast::Receiver<SystemEvent>> {
        if topic == "*" {
            return Some(self.tx.subscribe());
        }

        if topic.ends_with(".*") {
            return self.prefix_topics.get(topic).map(|tx| tx.subscribe());
        }

        self.exact_topics.get(topic).map(|tx| tx.subscribe())
    }

    pub fn mark_device_info_dirty(&self) {
        let _ = self.device_info_dirty_tx.send(());
    }

    pub fn subscribe_device_info_dirty(&self) -> broadcast::Receiver<()> {
        self.device_info_dirty_tx.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(SystemEvent::StreamStateChanged {
            state: "streaming".to_string(),
            device: Some("/dev/video0".to_string()),
            reason: None,
            next_retry_ms: None,
        });

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, SystemEvent::StreamStateChanged { .. }));
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(SystemEvent::StreamStateChanged {
            state: "ready".to_string(),
            device: Some("/dev/video0".to_string()),
            reason: None,
            next_retry_ms: None,
        });

        let event1 = rx1.recv().await.unwrap();
        let event2 = rx2.recv().await.unwrap();

        assert!(matches!(event1, SystemEvent::StreamStateChanged { .. }));
        assert!(matches!(event2, SystemEvent::StreamStateChanged { .. }));
    }

    #[tokio::test]
    async fn test_subscribe_topic_exact() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe_topic("stream.state_changed").unwrap();

        bus.publish(SystemEvent::StreamStateChanged {
            state: "ready".to_string(),
            device: None,
            reason: None,
            next_retry_ms: None,
        });

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, SystemEvent::StreamStateChanged { .. }));
    }

    #[tokio::test]
    async fn test_subscribe_topic_prefix() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe_topic("stream.*").unwrap();

        bus.publish(SystemEvent::StreamStateChanged {
            state: "ready".to_string(),
            device: None,
            reason: None,
            next_retry_ms: None,
        });

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, SystemEvent::StreamStateChanged { .. }));
    }

    #[test]
    fn test_subscribe_topic_unknown() {
        let bus = EventBus::new();
        assert!(bus.subscribe_topic("unknown.topic").is_none());
    }

    #[test]
    fn test_no_subscribers() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        bus.publish(SystemEvent::StreamStateChanged {
            state: "ready".to_string(),
            device: None,
            reason: None,
            next_retry_ms: None,
        });
    }
}

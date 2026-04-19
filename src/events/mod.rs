//! Event system for real-time state notifications
//!
//! This module provides a global event bus for broadcasting system events
//! to WebSocket clients and other subscribers.

pub mod types;

pub use types::{
    AtxDeviceInfo, AudioDeviceInfo, ClientStats, HidDeviceInfo, MsdDeviceInfo, SystemEvent,
    TtydDeviceInfo, VideoDeviceInfo,
};

use tokio::sync::broadcast;

/// Event channel capacity (ring buffer size)
const EVENT_CHANNEL_CAPACITY: usize = 256;

const EXACT_TOPICS: &[&str] = &[
    "stream.mode_switching",
    "stream.state_changed",
    "stream.config_changing",
    "stream.config_applied",
    "stream.device_lost",
    "stream.reconnecting",
    "stream.recovered",
    "stream.webrtc_ready",
    "stream.stats_update",
    "stream.mode_changed",
    "stream.mode_ready",
    "webrtc.ice_candidate",
    "webrtc.ice_complete",
    "msd.upload_progress",
    "msd.download_progress",
    "system.device_info",
    "error",
];

const PREFIX_TOPICS: &[&str] = &["stream.*", "webrtc.*", "msd.*", "system.*"];

fn make_sender() -> broadcast::Sender<SystemEvent> {
    let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    tx
}

fn topic_prefix(event_name: &str) -> Option<String> {
    event_name
        .split_once('.')
        .map(|(prefix, _)| format!("{}.*", prefix))
}

/// Global event bus for broadcasting system events
///
/// The event bus uses tokio's broadcast channel to distribute events
/// to multiple subscribers. Events are delivered to all active subscribers.
///
/// # Example
///
/// ```no_run
/// use one_kvm::events::{EventBus, SystemEvent};
///
/// let bus = EventBus::new();
///
/// // Publish an event
/// bus.publish(SystemEvent::StreamStateChanged {
///     state: "streaming".to_string(),
///     device: Some("/dev/video0".to_string()),
///     reason: None,
///     next_retry_ms: None,
/// });
///
/// // Subscribe to events
/// let mut rx = bus.subscribe();
/// tokio::spawn(async move {
///     while let Ok(event) = rx.recv().await {
///         println!("Received event: {:?}", event);
///     }
/// });
/// ```
pub struct EventBus {
    tx: broadcast::Sender<SystemEvent>,
    exact_topics: std::collections::HashMap<&'static str, broadcast::Sender<SystemEvent>>,
    prefix_topics: std::collections::HashMap<&'static str, broadcast::Sender<SystemEvent>>,
    device_info_dirty_tx: broadcast::Sender<()>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let tx = make_sender();
        let exact_topics = EXACT_TOPICS
            .iter()
            .map(|topic| (*topic, make_sender()))
            .collect();
        let prefix_topics = PREFIX_TOPICS
            .iter()
            .map(|topic| (*topic, make_sender()))
            .collect();
        let (device_info_dirty_tx, _dirty_rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        Self {
            tx,
            exact_topics,
            prefix_topics,
            device_info_dirty_tx,
        }
    }

    /// Publish an event to all subscribers
    ///
    /// If there are no active subscribers, the event is silently dropped.
    /// This is by design - events are fire-and-forget notifications.
    pub fn publish(&self, event: SystemEvent) {
        let event_name = event.event_name();

        if let Some(tx) = self.exact_topics.get(event_name) {
            let _ = tx.send(event.clone());
        }

        if let Some(prefix) = topic_prefix(event_name) {
            if let Some(tx) = self.prefix_topics.get(prefix.as_str()) {
                let _ = tx.send(event.clone());
            }
        }

        // If no subscribers, send returns Err which is normal
        let _ = self.tx.send(event);
    }

    /// Subscribe to events
    ///
    /// Returns a receiver that will receive all future events.
    /// The receiver uses a ring buffer, so if a subscriber falls too far
    /// behind, it will receive a `Lagged` error and miss some events.
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }

    /// Subscribe to a specific topic.
    ///
    /// Supports exact event names, namespace wildcards like `stream.*`, and
    /// `*` for the full event stream.
    pub fn subscribe_topic(&self, topic: &str) -> Option<broadcast::Receiver<SystemEvent>> {
        if topic == "*" {
            return Some(self.tx.subscribe());
        }

        if topic.ends_with(".*") {
            return self.prefix_topics.get(topic).map(|tx| tx.subscribe());
        }

        self.exact_topics.get(topic).map(|tx| tx.subscribe())
    }

    /// Mark the device-info snapshot as stale.
    ///
    /// This is an internal trigger used to refresh the latest `system.device_info`
    /// snapshot without exposing another public WebSocket event.
    pub fn mark_device_info_dirty(&self) {
        let _ = self.device_info_dirty_tx.send(());
    }

    /// Subscribe to internal device-info refresh triggers.
    pub fn subscribe_device_info_dirty(&self) -> broadcast::Receiver<()> {
        self.device_info_dirty_tx.subscribe()
    }

    /// Get the current number of active subscribers
    ///
    /// Useful for monitoring and debugging.
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

        // Should not panic when publishing with no subscribers
        bus.publish(SystemEvent::StreamStateChanged {
            state: "ready".to_string(),
            device: None,
            reason: None,
            next_retry_ms: None,
        });
    }
}

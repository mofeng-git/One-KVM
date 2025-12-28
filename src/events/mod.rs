//! Event system for real-time state notifications
//!
//! This module provides a global event bus for broadcasting system events
//! to WebSocket clients and other subscribers.

pub mod types;

pub use types::{
    AtxDeviceInfo, AudioDeviceInfo, ClientStats, HidDeviceInfo, MsdDeviceInfo, SystemEvent, VideoDeviceInfo,
};

use tokio::sync::broadcast;

/// Event channel capacity (ring buffer size)
const EVENT_CHANNEL_CAPACITY: usize = 256;

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
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Publish an event to all subscribers
    ///
    /// If there are no active subscribers, the event is silently dropped.
    /// This is by design - events are fire-and-forget notifications.
    pub fn publish(&self, event: SystemEvent) {
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

        bus.publish(SystemEvent::SystemError {
            module: "test".to_string(),
            severity: "info".to_string(),
            message: "test message".to_string(),
        });

        let event1 = rx1.recv().await.unwrap();
        let event2 = rx2.recv().await.unwrap();

        assert!(matches!(event1, SystemEvent::SystemError { .. }));
        assert!(matches!(event2, SystemEvent::SystemError { .. }));
    }

    #[test]
    fn test_no_subscribers() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        // Should not panic when publishing with no subscribers
        bus.publish(SystemEvent::SystemError {
            module: "test".to_string(),
            severity: "info".to_string(),
            message: "test".to_string(),
        });
    }
}

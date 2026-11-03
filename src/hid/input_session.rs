//! Per-connection admitted input and disconnect release. One controlling client
//! is supported: overlapping same-key presses from different clients need a
//! controller-level ownership policy, not a connection-wide reset.
use super::datachannel::HidChannelEvent;
use super::{
    CanonicalKey, HidController, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton,
    MouseEvent, MouseEventType,
};
use crate::error::{AppError, Result};
use std::collections::HashSet;
use tokio::sync::Mutex;

#[async_trait::async_trait]
pub(crate) trait InputSink: Send + Sync {
    async fn admit(&self, event: HidChannelEvent) -> Result<()>;
}
#[async_trait::async_trait]
impl InputSink for HidController {
    async fn admit(&self, event: HidChannelEvent) -> Result<()> {
        match event {
            HidChannelEvent::Keyboard(e) => self.send_keyboard(e).await,
            HidChannelEvent::Mouse(e) => self.send_mouse(e).await,
            HidChannelEvent::Consumer(e) => self.send_consumer(e).await,
        }
    }
}

#[derive(Default)]
struct HeldInputs {
    keys: HashSet<CanonicalKey>,
    modifiers: u8,
    buttons: u8,
}
impl HeldInputs {
    fn observe(&mut self, event: &HidChannelEvent) {
        match event {
            HidChannelEvent::Keyboard(e) => {
                if e.event_type == KeyEventType::Down {
                    self.keys.insert(e.key);
                } else {
                    self.keys.remove(&e.key);
                }
                self.modifiers = e.modifiers.to_hid_byte();
            }
            HidChannelEvent::Mouse(e) => {
                if let Some(button) = e.button {
                    match e.event_type {
                        MouseEventType::Down => self.buttons |= button.to_hid_bit(),
                        MouseEventType::Up => self.buttons &= !button.to_hid_bit(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    fn releases(&self) -> Vec<HidChannelEvent> {
        let mut keys = self.keys.clone();
        for (bit, key) in [
            CanonicalKey::ControlLeft,
            CanonicalKey::ShiftLeft,
            CanonicalKey::AltLeft,
            CanonicalKey::MetaLeft,
            CanonicalKey::ControlRight,
            CanonicalKey::ShiftRight,
            CanonicalKey::AltRight,
            CanonicalKey::MetaRight,
        ]
        .into_iter()
        .enumerate()
        {
            if self.modifiers & (1 << bit) != 0 {
                keys.insert(key);
            }
        }
        let mut result: Vec<_> = keys
            .into_iter()
            .map(|key| {
                HidChannelEvent::Keyboard(KeyboardEvent::key_up(key, KeyboardModifiers::default()))
            })
            .collect();
        for button in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
        ] {
            if self.buttons & button.to_hid_bit() != 0 {
                result.push(HidChannelEvent::Mouse(MouseEvent::button_up(button)));
            }
        }
        result
    }
}
#[derive(Default)]
struct SessionState {
    closed: bool,
    held: HeldInputs,
}
#[derive(Default)]
pub(crate) struct HidInputSession {
    state: Mutex<SessionState>,
}
impl HidInputSession {
    /// Admission is ordered against close. Success means queued, not delivered.
    pub(crate) async fn dispatch(
        &self,
        sink: &(impl InputSink + ?Sized),
        event: HidChannelEvent,
    ) -> Result<()> {
        let mut state = self.state.lock().await;
        if state.closed {
            return Err(AppError::BadRequest("HID connection closed".into()));
        }
        sink.admit(event.clone()).await?;
        state.held.observe(&event);
        Ok(())
    }
    /// Release only this connection's held inputs, never reset an idle peer.
    /// Keep failed releases recorded so a repeated close can retry them.
    pub(crate) async fn close(&self, sink: &(impl InputSink + ?Sized)) -> Result<()> {
        let mut state = self.state.lock().await;
        state.closed = true;
        for event in state.held.releases() {
            sink.admit(event.clone()).await?;
        }
        state.held = HeldInputs::default();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[derive(Default)]
    struct Sink {
        events: Mutex<Vec<HidChannelEvent>>,
        fail: AtomicBool,
    }
    #[async_trait::async_trait]
    impl InputSink for Sink {
        async fn admit(&self, e: HidChannelEvent) -> Result<()> {
            if self.fail.load(Ordering::Acquire) {
                return Err(AppError::BadRequest("injected".into()));
            }
            self.events.lock().await.push(e);
            Ok(())
        }
    }
    #[tokio::test]
    async fn actual_right_click_packets_admit_in_order_and_leave_no_held_state() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        for bytes in [
            [2, 1, 102, 70, 0, 64, 0],
            [2, 2, 0, 0, 0, 0, 2],
            [2, 3, 0, 0, 0, 0, 2],
        ] {
            session
                .dispatch(
                    &sink,
                    super::super::datachannel::parse_hid_message(&bytes).unwrap(),
                )
                .await
                .unwrap();
        }
        session.close(&sink).await.unwrap();
        let events = sink.events.lock().await;
        assert_eq!(events.len(), 3);
        assert!(
            matches!(&events[0],HidChannelEvent::Mouse(e) if e.event_type==MouseEventType::MoveAbs && e.x==18022 && e.y==16384)
        );
        assert!(
            matches!(&events[1],HidChannelEvent::Mouse(e) if e.event_type==MouseEventType::Down && e.button==Some(MouseButton::Right))
        );
        assert!(
            matches!(&events[2],HidChannelEvent::Mouse(e) if e.event_type==MouseEventType::Up && e.button==Some(MouseButton::Right))
        );
    }
    #[tokio::test]
    async fn idle_disconnect_does_not_reset_peer() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        session.close(&sink).await.unwrap();
        assert!(sink.events.lock().await.is_empty());
    }
    #[tokio::test]
    async fn only_own_held_button_is_released_once() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        session
            .dispatch(
                &sink,
                HidChannelEvent::Mouse(MouseEvent::button_down(MouseButton::Right)),
            )
            .await
            .unwrap();
        session.close(&sink).await.unwrap();
        session.close(&sink).await.unwrap();
        let events = sink.events.lock().await;
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[1],HidChannelEvent::Mouse(e) if e.event_type==MouseEventType::Up && e.button==Some(MouseButton::Right))
        );
    }
    #[tokio::test]
    async fn complete_click_disconnect_sends_nothing_extra() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        for event in [
            MouseEvent::button_down(MouseButton::Left),
            MouseEvent::button_up(MouseButton::Left),
        ] {
            session
                .dispatch(&sink, HidChannelEvent::Mouse(event))
                .await
                .unwrap();
        }
        session.close(&sink).await.unwrap();
        assert_eq!(sink.events.lock().await.len(), 2);
    }
    #[tokio::test]
    async fn failed_admission_is_not_claimed_held() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        sink.fail.store(true, Ordering::Release);
        assert!(session
            .dispatch(
                &sink,
                HidChannelEvent::Mouse(MouseEvent::button_down(MouseButton::Left))
            )
            .await
            .is_err());
        sink.fail.store(false, Ordering::Release);
        session.close(&sink).await.unwrap();
        assert!(sink.events.lock().await.is_empty());
    }
    #[tokio::test]
    async fn failed_release_is_retained_and_close_retries() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        session
            .dispatch(
                &sink,
                HidChannelEvent::Mouse(MouseEvent::button_down(MouseButton::Left)),
            )
            .await
            .unwrap();
        sink.fail.store(true, Ordering::Release);
        assert!(session.close(&sink).await.is_err());
        sink.fail.store(false, Ordering::Release);
        session.close(&sink).await.unwrap();
        assert_eq!(sink.events.lock().await.len(), 2);
        assert!(session
            .dispatch(&sink, HidChannelEvent::Mouse(MouseEvent::move_abs(1, 2)))
            .await
            .is_err());
    }
    #[tokio::test]
    async fn keyboard_and_modifier_disconnect_release_owned_state() {
        let sink = Sink::default();
        let session = HidInputSession::default();
        session
            .dispatch(
                &sink,
                HidChannelEvent::Keyboard(KeyboardEvent::key_down(
                    CanonicalKey::KeyA,
                    KeyboardModifiers {
                        left_ctrl: true,
                        ..Default::default()
                    },
                )),
            )
            .await
            .unwrap();
        session.close(&sink).await.unwrap();
        let events = sink.events.lock().await;
        assert_eq!(events.len(), 3);
        assert!(events.iter().any(|e|matches!(e,HidChannelEvent::Keyboard(k) if k.key==CanonicalKey::ControlLeft && k.event_type==KeyEventType::Up)));
    }
}

//! WebSocket HID for MJPEG mode; binary messages per `crate::hid::datachannel`.

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::hid::datachannel::{parse_hid_message, HidChannelEvent};
use crate::hid::HidController;

pub type ClientId = String;

#[derive(Debug)]
pub struct WsHidClient {
    pub id: ClientId,
    pub connected_at: Instant,
    pub events_processed: AtomicU64,
    shutdown_tx: mpsc::Sender<()>,
}

impl WsHidClient {
    pub fn events_count(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed)
    }

    pub fn connected_secs(&self) -> u64 {
        self.connected_at.elapsed().as_secs()
    }
}

pub struct WsHidHandler {
    hid_controller: RwLock<Option<Arc<HidController>>>,
    clients: RwLock<HashMap<ClientId, Arc<WsHidClient>>>,
    running: AtomicBool,
    total_events: AtomicU64,
}

impl WsHidHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            hid_controller: RwLock::new(None),
            clients: RwLock::new(HashMap::new()),
            running: AtomicBool::new(true),
            total_events: AtomicU64::new(0),
        })
    }

    pub fn set_hid_controller(&self, hid: Arc<HidController>) {
        *self.hid_controller.write() = Some(hid);
        info!("WsHidHandler: HID controller set");
    }

    pub fn hid_controller(&self) -> Option<Arc<HidController>> {
        self.hid_controller.read().clone()
    }

    pub fn is_hid_available(&self) -> bool {
        self.hid_controller.read().is_some()
    }

    pub fn client_count(&self) -> usize {
        self.clients.read().len()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let clients = self.clients.read();
        for client in clients.values() {
            let _ = client.shutdown_tx.try_send(());
        }
    }

    pub fn total_events(&self) -> u64 {
        self.total_events.load(Ordering::Relaxed)
    }

    pub async fn add_client(self: &Arc<Self>, client_id: ClientId, socket: WebSocket) {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let client = Arc::new(WsHidClient {
            id: client_id.clone(),
            connected_at: Instant::now(),
            events_processed: AtomicU64::new(0),
            shutdown_tx,
        });

        self.clients
            .write()
            .insert(client_id.clone(), client.clone());
        info!(
            "WsHidHandler: Client {} connected (total: {})",
            client_id,
            self.client_count()
        );

        let handler = self.clone();
        tokio::spawn(async move {
            handler
                .handle_client(client_id.clone(), socket, client, shutdown_rx)
                .await;
            handler.remove_client(&client_id);
        });
    }

    pub fn remove_client(&self, client_id: &str) {
        if let Some(client) = self.clients.write().remove(client_id) {
            info!(
                "WsHidHandler: Client {} disconnected after {}s ({} events)",
                client_id,
                client.connected_secs(),
                client.events_count()
            );
        }
    }

    async fn handle_client(
        &self,
        client_id: ClientId,
        socket: WebSocket,
        client: Arc<WsHidClient>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        let (mut sender, mut receiver) = socket.split();

        let status_byte = if self.is_hid_available() {
            0x00u8
        } else {
            0x01u8
        };
        let _ = sender.send(Message::Binary(vec![status_byte].into())).await;

        loop {
            tokio::select! {
                biased;

                _ = shutdown_rx.recv() => {
                    debug!("WsHidHandler: Client {} received shutdown signal", client_id);
                    break;
                }

                msg = receiver.next() => {
                    match msg {
                        Some(Ok(Message::Binary(data))) => {
                            if let Err(e) = self.handle_binary_message(&data, &client).await {
                                warn!("WsHidHandler: Failed to handle binary message: {}", e);
                            }
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = sender.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            debug!("WsHidHandler: Client {} closed connection", client_id);
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WsHidHandler: WebSocket error for client {}: {}", client_id, e);
                            break;
                        }
                        None => {
                            debug!("WsHidHandler: Client {} stream ended", client_id);
                            break;
                        }
                        Some(Ok(Message::Text(_))) => {
                            warn!("WsHidHandler: Ignoring text message from client {} (binary protocol only)", client_id);
                        }
                        _ => {}
                    }
                }
            }
        }

        let hid = self.hid_controller.read().clone();
        if let Some(hid) = hid {
            if let Err(e) = hid.reset().await {
                warn!(
                    "WsHidHandler: Failed to reset HID on client {} disconnect: {}",
                    client_id, e
                );
            } else {
                debug!("WsHidHandler: HID reset on client {} disconnect", client_id);
            }
        }
    }

    async fn handle_binary_message(&self, data: &[u8], client: &WsHidClient) -> Result<(), String> {
        let hid = self
            .hid_controller
            .read()
            .clone()
            .ok_or("HID controller not available")?;

        let event = parse_hid_message(data).ok_or("Invalid binary HID message")?;

        match event {
            HidChannelEvent::Keyboard(kb_event) => {
                hid.send_keyboard(kb_event)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            HidChannelEvent::Mouse(ms_event) => {
                hid.send_mouse(ms_event).await.map_err(|e| e.to_string())?;
            }
            HidChannelEvent::Consumer(consumer_event) => {
                hid.send_consumer(consumer_event)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        client.events_processed.fetch_add(1, Ordering::Relaxed);
        self.total_events.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_hid_handler_creation() {
        let handler = WsHidHandler::new();
        assert!(handler.is_running());
        assert_eq!(handler.client_count(), 0);
        assert!(!handler.is_hid_available());
    }
}

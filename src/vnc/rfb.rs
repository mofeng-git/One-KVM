use std::net::SocketAddr;

use bytes::Bytes;
use des::cipher::{BlockEncrypt, KeyInit};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use crate::config::{VncConfig, VncEncoding};
use crate::error::{AppError, Result};
use crate::hid::{
    CanonicalKey, KeyEventType, KeyboardEvent, KeyboardModifiers, MouseButton, MouseEvent,
    MouseEventType,
};

const ENCODING_TIGHT: i32 = 7;
const ENCODING_H264: i32 = 50;
const ENCODING_DESKTOP_SIZE: i32 = -223;

pub enum RfbFrame {
    Jpeg {
        data: Bytes,
        width: u16,
        height: u16,
    },
    H264 {
        data: Bytes,
        width: u16,
        height: u16,
        key: bool,
    },
}

pub enum RfbInputEvent {
    Key(RfbKeyEvent),
    Pointer(RfbPointerEvent),
    Clipboard(String),
    Ignored,
    Disconnected,
}

pub struct RfbKeyEvent {
    pub down: bool,
    pub keysym: u32,
}

pub struct RfbPointerEvent {
    pub x: u16,
    pub y: u16,
    pub button_mask: u8,
    pub previous_button_mask: u8,
}

#[derive(Default)]
struct ClientEncodings {
    has_tight: bool,
    tight_jpeg_quality: u8,
    has_h264: bool,
    has_resize: bool,
}

pub struct RfbClient {
    stream: TcpStream,
    peer: SocketAddr,
    config: VncConfig,
    encodings: ClientEncodings,
    width: u16,
    height: u16,
    last_buttons: u8,
    h264_waiting_keyframe: bool,
    shutdown_tx: broadcast::Sender<()>,
}

impl RfbClient {
    pub fn new(stream: TcpStream, peer: SocketAddr, config: VncConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            stream,
            peer,
            config,
            encodings: ClientEncodings::default(),
            width: 800,
            height: 600,
            last_buttons: 0,
            h264_waiting_keyframe: true,
            shutdown_tx,
        }
    }

    pub fn set_size(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    pub fn shutdown_receiver(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    pub async fn handshake(&mut self) -> Result<()> {
        self.stream.write_all(b"RFB 003.008\n").await?;
        let mut version = [0u8; 12];
        self.stream.read_exact(&mut version).await?;
        if !version.starts_with(b"RFB 003.00") {
            return Err(AppError::BadRequest("Invalid RFB version".to_string()));
        }

        self.stream.write_all(&[1, 2]).await?;
        let sec_type = read_u8(&mut self.stream).await?;
        if sec_type != 2 {
            return Err(AppError::BadRequest("VNCAuth is required".to_string()));
        }
        self.handle_vnc_auth().await?;

        let _shared = read_u8(&mut self.stream).await?;
        self.write_server_init().await?;
        self.read_until_set_encodings().await?;
        self.validate_encoding_policy()?;
        tracing::info!(
            "VNC client {} negotiated encoding {:?}",
            self.peer,
            self.config.encoding
        );
        Ok(())
    }

    async fn handle_vnc_auth(&mut self) -> Result<()> {
        let challenge: [u8; 16] = rand::random();
        self.stream.write_all(&challenge).await?;
        let mut response = [0u8; 16];
        self.stream.read_exact(&mut response).await?;
        let password = self.config.password.as_deref().unwrap_or("");
        let expected = encrypt_vnc_challenge(&challenge, password)?;
        let ok = response == expected;
        self.stream
            .write_all(&(if ok { 0u32 } else { 1u32 }).to_be_bytes())
            .await?;
        if !ok {
            return Err(AppError::BadRequest("Invalid VNC password".to_string()));
        }
        Ok(())
    }

    async fn write_server_init(&mut self) -> Result<()> {
        self.stream.write_all(&self.width.to_be_bytes()).await?;
        self.stream.write_all(&self.height.to_be_bytes()).await?;
        self.stream
            .write_all(&[32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0])
            .await?;
        let name = b"One-KVM VNC";
        self.stream
            .write_all(&(name.len() as u32).to_be_bytes())
            .await?;
        self.stream.write_all(name).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn read_until_set_encodings(&mut self) -> Result<()> {
        loop {
            let msg_type = read_u8(&mut self.stream).await?;
            match msg_type {
                0 => {
                    let mut buf = [0u8; 19];
                    self.stream.read_exact(&mut buf).await?;
                }
                2 => {
                    let _pad = read_u8(&mut self.stream).await?;
                    let count = read_u16(&mut self.stream).await?;
                    if count == 0 || count > 1024 {
                        return Err(AppError::BadRequest(
                            "Invalid VNC encoding list".to_string(),
                        ));
                    }
                    let mut encodings = ClientEncodings::default();
                    for _ in 0..count {
                        let enc = read_i32(&mut self.stream).await?;
                        match enc {
                            ENCODING_TIGHT => encodings.has_tight = true,
                            ENCODING_H264 => encodings.has_h264 = true,
                            ENCODING_DESKTOP_SIZE => encodings.has_resize = true,
                            -32..=-23 => {
                                let q = ((enc + 33) * 10).clamp(10, 100) as u8;
                                encodings.tight_jpeg_quality = encodings.tight_jpeg_quality.max(q);
                            }
                            _ => {}
                        }
                    }
                    self.encodings = encodings;
                    return Ok(());
                }
                3 => {
                    let mut buf = [0u8; 9];
                    self.stream.read_exact(&mut buf).await?;
                }
                4 => {
                    let mut buf = [0u8; 7];
                    self.stream.read_exact(&mut buf).await?;
                }
                5 => {
                    let mut buf = [0u8; 5];
                    self.stream.read_exact(&mut buf).await?;
                }
                6 => {
                    let mut hdr = [0u8; 7];
                    self.stream.read_exact(&mut hdr).await?;
                    let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
                    let mut data = vec![0u8; len.min(1024 * 1024)];
                    self.stream.read_exact(&mut data).await?;
                }
                _ => {
                    return Err(AppError::BadRequest(format!(
                        "Unsupported RFB message {}",
                        msg_type
                    )))
                }
            }
        }
    }

    fn validate_encoding_policy(&self) -> Result<()> {
        match self.config.encoding {
            VncEncoding::TightJpeg => {
                if !self.encodings.has_tight || self.encodings.tight_jpeg_quality == 0 {
                    return Err(AppError::BadRequest(
                        "VNC client must support Tight JPEG encoding".to_string(),
                    ));
                }
            }
            VncEncoding::H264 => {
                if !self.encodings.has_h264 {
                    return Err(AppError::BadRequest(
                        "VNC client must support Open H.264 encoding".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn read_input_event(&mut self) -> Result<RfbInputEvent> {
        let msg_type = match read_u8(&mut self.stream).await {
            Ok(v) => v,
            Err(AppError::Io(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(RfbInputEvent::Disconnected);
            }
            Err(err) => return Err(err),
        };
        match msg_type {
            0 => {
                let mut buf = [0u8; 19];
                self.stream.read_exact(&mut buf).await?;
                Ok(RfbInputEvent::Ignored)
            }
            2 => {
                let _pad = read_u8(&mut self.stream).await?;
                let count = read_u16(&mut self.stream).await?;
                for _ in 0..count {
                    let _ = read_i32(&mut self.stream).await?;
                }
                Ok(RfbInputEvent::Ignored)
            }
            3 => {
                let mut buf = [0u8; 9];
                self.stream.read_exact(&mut buf).await?;
                Ok(RfbInputEvent::Ignored)
            }
            4 => {
                let down = read_u8(&mut self.stream).await? != 0;
                let mut pad = [0u8; 2];
                self.stream.read_exact(&mut pad).await?;
                let keysym = read_u32(&mut self.stream).await?;
                Ok(RfbInputEvent::Key(RfbKeyEvent { down, keysym }))
            }
            5 => {
                let button_mask = read_u8(&mut self.stream).await?;
                let x = read_u16(&mut self.stream).await?;
                let y = read_u16(&mut self.stream).await?;
                let previous_button_mask = self.last_buttons;
                self.last_buttons = button_mask;
                Ok(RfbInputEvent::Pointer(RfbPointerEvent {
                    x,
                    y,
                    button_mask,
                    previous_button_mask,
                }))
            }
            6 => {
                let mut hdr = [0u8; 7];
                self.stream.read_exact(&mut hdr).await?;
                let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as usize;
                let mut data = vec![0u8; len.min(1024 * 1024)];
                self.stream.read_exact(&mut data).await?;
                Ok(RfbInputEvent::Clipboard(
                    String::from_utf8_lossy(&data).to_string(),
                ))
            }
            _ => Err(AppError::BadRequest(format!(
                "Unsupported RFB message {}",
                msg_type
            ))),
        }
    }

    pub async fn send_frame(&mut self, frame: RfbFrame) -> Result<()> {
        match frame {
            RfbFrame::Jpeg {
                data,
                width,
                height,
            } => {
                self.maybe_resize(width, height).await?;
                self.write_frame_header(width, height, ENCODING_TIGHT)
                    .await?;
                write_tight_jpeg_payload(&mut self.stream, &data).await?;
            }
            RfbFrame::H264 {
                data,
                width,
                height,
                key,
            } => {
                self.maybe_resize(width, height).await?;
                if self.h264_waiting_keyframe && !key {
                    return Ok(());
                }
                self.write_frame_header(width, height, ENCODING_H264)
                    .await?;
                self.stream
                    .write_all(&(data.len() as u32).to_be_bytes())
                    .await?;
                self.stream
                    .write_all(&(self.h264_waiting_keyframe as u32).to_be_bytes())
                    .await?;
                self.stream.write_all(&data).await?;
                self.h264_waiting_keyframe = false;
            }
        }
        self.stream.flush().await?;
        Ok(())
    }

    async fn maybe_resize(&mut self, width: u16, height: u16) -> Result<()> {
        if width == self.width && height == self.height {
            return Ok(());
        }
        if !self.encodings.has_resize {
            return Err(AppError::BadRequest(
                "VNC client does not support DesktopSize resize; reconnect required".to_string(),
            ));
        }
        self.write_frame_header(width, height, ENCODING_DESKTOP_SIZE)
            .await?;
        self.width = width;
        self.height = height;
        self.h264_waiting_keyframe = true;
        Ok(())
    }

    async fn write_frame_header(&mut self, width: u16, height: u16, encoding: i32) -> Result<()> {
        self.stream.write_all(&[0, 0]).await?;
        self.stream.write_all(&1u16.to_be_bytes()).await?;
        self.stream.write_all(&0u16.to_be_bytes()).await?;
        self.stream.write_all(&0u16.to_be_bytes()).await?;
        self.stream.write_all(&width.to_be_bytes()).await?;
        self.stream.write_all(&height.to_be_bytes()).await?;
        self.stream.write_all(&encoding.to_be_bytes()).await?;
        Ok(())
    }
}

async fn write_tight_jpeg_payload(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    if data.len() > 0x3f_ffff {
        return Err(AppError::BadRequest(
            "JPEG frame too large for Tight encoding".to_string(),
        ));
    }
    stream.write_all(&[0b1001_1111]).await?;
    write_compact_len(stream, data.len()).await?;
    stream.write_all(data).await?;
    Ok(())
}

async fn write_compact_len(stream: &mut TcpStream, len: usize) -> Result<()> {
    if len <= 127 {
        stream.write_all(&[(len & 0x7f) as u8]).await?;
    } else if len <= 16_383 {
        stream
            .write_all(&[((len & 0x7f) as u8) | 0x80, ((len >> 7) & 0x7f) as u8])
            .await?;
    } else {
        stream
            .write_all(&[
                ((len & 0x7f) as u8) | 0x80,
                (((len >> 7) & 0x7f) as u8) | 0x80,
                ((len >> 14) & 0xff) as u8,
            ])
            .await?;
    }
    Ok(())
}

fn encrypt_vnc_challenge(challenge: &[u8; 16], password: &str) -> Result<[u8; 16]> {
    let mut key = [0u8; 8];
    for (dst, src) in key.iter_mut().zip(password.as_bytes().iter().take(8)) {
        *dst = reverse_bits(*src);
    }
    let cipher = des::Des::new_from_slice(&key)
        .map_err(|_| AppError::BadRequest("Invalid VNC DES key".to_string()))?;
    let mut out = *challenge;
    for chunk in out.chunks_exact_mut(8) {
        cipher.encrypt_block(chunk.into());
    }
    Ok(out)
}

fn reverse_bits(byte: u8) -> u8 {
    byte.reverse_bits()
}

async fn read_u8(stream: &mut TcpStream) -> Result<u8> {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).await?;
    Ok(buf[0])
}

async fn read_u16(stream: &mut TcpStream) -> Result<u16> {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    Ok(u16::from_be_bytes(buf))
}

async fn read_u32(stream: &mut TcpStream) -> Result<u32> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    Ok(u32::from_be_bytes(buf))
}

async fn read_i32(stream: &mut TcpStream) -> Result<i32> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    Ok(i32::from_be_bytes(buf))
}

pub fn key_event_to_hid(event: RfbKeyEvent) -> Option<KeyboardEvent> {
    let key = keysym_to_key(event.keysym)?;
    Some(KeyboardEvent {
        event_type: if event.down {
            KeyEventType::Down
        } else {
            KeyEventType::Up
        },
        key,
        modifiers: KeyboardModifiers::default(),
    })
}

fn keysym_to_key(keysym: u32) -> Option<CanonicalKey> {
    match keysym {
        0xff08 => Some(CanonicalKey::Backspace),
        0xff09 => Some(CanonicalKey::Tab),
        0xff0d => Some(CanonicalKey::Enter),
        0xff1b => Some(CanonicalKey::Escape),
        0xffff => Some(CanonicalKey::Delete),
        0xff50 => Some(CanonicalKey::Home),
        0xff51 => Some(CanonicalKey::ArrowLeft),
        0xff52 => Some(CanonicalKey::ArrowUp),
        0xff53 => Some(CanonicalKey::ArrowRight),
        0xff54 => Some(CanonicalKey::ArrowDown),
        0xff55 => Some(CanonicalKey::PageUp),
        0xff56 => Some(CanonicalKey::PageDown),
        0xff57 => Some(CanonicalKey::End),
        0xff63 => Some(CanonicalKey::Insert),
        0xffbe..=0xffc9 => CanonicalKey::from_hid_usage((keysym - 0xffbe + 0x3a) as u8),
        0x20 => Some(CanonicalKey::Space),
        0x61..=0x7a => CanonicalKey::from_hid_usage((keysym - 0x61 + 0x04) as u8),
        0x41..=0x5a => CanonicalKey::from_hid_usage((keysym - 0x41 + 0x04) as u8),
        0x31..=0x39 => CanonicalKey::from_hid_usage((keysym - 0x31 + 0x1e) as u8),
        0x30 => Some(CanonicalKey::Digit0),
        0x2d => Some(CanonicalKey::Minus),
        0x3d => Some(CanonicalKey::Equal),
        0x5b => Some(CanonicalKey::BracketLeft),
        0x5d => Some(CanonicalKey::BracketRight),
        0x5c => Some(CanonicalKey::Backslash),
        0x3b => Some(CanonicalKey::Semicolon),
        0x27 => Some(CanonicalKey::Quote),
        0x60 => Some(CanonicalKey::Backquote),
        0x2c => Some(CanonicalKey::Comma),
        0x2e => Some(CanonicalKey::Period),
        0x2f => Some(CanonicalKey::Slash),
        _ => None,
    }
}

pub fn pointer_event_to_hid(event: RfbPointerEvent, width: u16, height: u16) -> Vec<MouseEvent> {
    let mut out = Vec::new();
    let abs_x = ((event.x as u64 * 32767) / width.max(1) as u64) as i32;
    let abs_y = ((event.y as u64 * 32767) / height.max(1) as u64) as i32;
    out.push(MouseEvent {
        event_type: MouseEventType::MoveAbs,
        x: abs_x,
        y: abs_y,
        button: None,
        scroll: 0,
    });

    if event.button_mask & 0x08 != 0 {
        out.push(MouseEvent::scroll(1));
    }
    if event.button_mask & 0x10 != 0 {
        out.push(MouseEvent::scroll(-1));
    }

    for (bit, button) in [
        (0x01, MouseButton::Left),
        (0x02, MouseButton::Middle),
        (0x04, MouseButton::Right),
    ] {
        if (event.button_mask ^ event.previous_button_mask) & bit == 0 {
            continue;
        }
        if event.button_mask & bit != 0 {
            out.push(MouseEvent::button_down(button));
        } else {
            out.push(MouseEvent::button_up(button));
        }
    }

    out
}

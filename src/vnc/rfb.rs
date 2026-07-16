use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use des::cipher::{Block, BlockCipherEncrypt, KeyInit};
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
const MAX_ENCODING_COUNT: usize = 1024;
const MAX_CLIPBOARD_SIZE: usize = 1024 * 1024;
const SECURITY_TYPE_VNC_AUTH: u8 = 2;

#[derive(Clone, Debug)]
pub enum RfbFrame {
    Jpeg {
        data: Bytes,
        width: u16,
        height: u16,
        sequence: u64,
    },
    H264 {
        data: Bytes,
        width: u16,
        height: u16,
        key: bool,
        sequence: u64,
    },
}

impl RfbFrame {
    pub fn sequence(&self) -> u64 {
        match self {
            Self::Jpeg { sequence, .. } | Self::H264 { sequence, .. } => *sequence,
        }
    }

    pub fn size(&self) -> (u16, u16) {
        match self {
            Self::Jpeg { width, height, .. } | Self::H264 { width, height, .. } => {
                (*width, *height)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RfbVersion {
    V3_3,
    V3_7,
    V3_8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RfbPixelFormat {
    pub bits_per_pixel: u8,
    pub depth: u8,
    pub big_endian: bool,
    pub true_colour: bool,
    pub red_max: u16,
    pub green_max: u16,
    pub blue_max: u16,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

impl Default for RfbPixelFormat {
    fn default() -> Self {
        Self {
            bits_per_pixel: 32,
            depth: 24,
            big_endian: false,
            true_colour: true,
            red_max: 255,
            green_max: 255,
            blue_max: 255,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramebufferUpdateRequest {
    pub incremental: bool,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameSendOutcome {
    NotSent,
    FrameSent,
    DesktopSizeSent,
}

#[derive(Debug)]
pub enum RfbInputEvent {
    Key(RfbKeyEvent),
    Pointer(RfbPointerEvent),
    SetPixelFormat(RfbPixelFormat),
    SetEncodings {
        encoding_enabled: bool,
        resumed: bool,
    },
    FramebufferUpdateRequest(FramebufferUpdateRequest),
    UnsupportedClientCutText,
    Disconnected,
}

#[derive(Debug)]
pub struct RfbKeyEvent {
    pub down: bool,
    pub keysym: u32,
}

#[derive(Debug)]
pub struct RfbPointerEvent {
    pub x: u16,
    pub y: u16,
    pub button_mask: u8,
    pub previous_button_mask: u8,
}

#[derive(Default, Clone, Copy)]
struct ClientEncodings {
    has_tight: bool,
    has_jpeg_quality: bool,
    has_h264: bool,
    has_resize: bool,
}

#[derive(Default)]
struct KeyboardState {
    modifiers: KeyboardModifiers,
}

pub struct RfbClient {
    stream: TcpStream,
    peer: SocketAddr,
    config: VncConfig,
    encodings: ClientEncodings,
    pixel_format: RfbPixelFormat,
    width: u16,
    height: u16,
    last_buttons: u8,
    keyboard: KeyboardState,
    input_buffer: BytesMut,
    pending_request: Option<bool>,
    last_sent_sequence: Option<u64>,
    h264_waiting_keyframe: bool,
    shared: bool,
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
            pixel_format: RfbPixelFormat::default(),
            width: 800,
            height: 600,
            last_buttons: 0,
            keyboard: KeyboardState::default(),
            input_buffer: BytesMut::with_capacity(1024),
            pending_request: None,
            last_sent_sequence: None,
            h264_waiting_keyframe: true,
            shared: false,
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

    pub fn framebuffer_size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub fn has_pending_request(&self) -> bool {
        self.pending_request.is_some()
    }

    pub fn has_complete_buffered_input(&self) -> Result<bool> {
        Ok(input_message_len(&self.input_buffer)?
            .is_some_and(|length| self.input_buffer.len() >= length))
    }

    pub fn shared(&self) -> bool {
        self.shared
    }

    pub async fn handshake(&mut self) -> Result<()> {
        self.stream.write_all(b"RFB 003.008\n").await?;
        let mut version = [0u8; 12];
        self.stream.read_exact(&mut version).await?;
        let version = parse_rfb_version(&version)?;

        match version {
            RfbVersion::V3_3 => {
                self.stream
                    .write_all(&(SECURITY_TYPE_VNC_AUTH as u32).to_be_bytes())
                    .await?;
            }
            RfbVersion::V3_7 | RfbVersion::V3_8 => {
                self.stream.write_all(&[1, SECURITY_TYPE_VNC_AUTH]).await?;
                let sec_type = read_u8(&mut self.stream).await?;
                if sec_type != SECURITY_TYPE_VNC_AUTH {
                    return Err(AppError::BadRequest("VNCAuth is required".to_string()));
                }
            }
        }
        self.handle_vnc_auth(version).await?;

        self.shared = read_u8(&mut self.stream).await? != 0;
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

    async fn handle_vnc_auth(&mut self, version: RfbVersion) -> Result<()> {
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
            if version == RfbVersion::V3_8 {
                let reason = b"Invalid VNC password";
                self.stream
                    .write_all(&(reason.len() as u32).to_be_bytes())
                    .await?;
                self.stream.write_all(reason).await?;
            }
            self.stream.flush().await?;
            return Err(AppError::BadRequest("Invalid VNC password".to_string()));
        }
        Ok(())
    }

    async fn write_server_init(&mut self) -> Result<()> {
        self.stream.write_all(&self.width.to_be_bytes()).await?;
        self.stream.write_all(&self.height.to_be_bytes()).await?;
        self.stream
            .write_all(&pixel_format_bytes(self.pixel_format))
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
            match self.read_input_event().await? {
                RfbInputEvent::SetEncodings { .. } => return Ok(()),
                RfbInputEvent::Disconnected => {
                    return Err(AppError::BadRequest(
                        "VNC client disconnected during negotiation".to_string(),
                    ))
                }
                _ => {}
            }
        }
    }

    fn validate_encoding_policy(&self) -> Result<()> {
        match self.config.encoding {
            VncEncoding::TightJpeg => {
                if !self.configured_encoding_enabled() {
                    return Err(AppError::BadRequest(
                        "VNC client must support Tight JPEG encoding".to_string(),
                    ));
                }
            }
            VncEncoding::H264 => {
                if !self.configured_encoding_enabled() {
                    return Err(AppError::BadRequest(
                        "VNC client must support Open H.264 encoding".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn read_input_event(&mut self) -> Result<RfbInputEvent> {
        loop {
            if let Some(event) = self.decode_buffered_input()? {
                return Ok(event);
            }

            // read_buf is cancellation-safe and keeps completed reads in input_buffer.
            let read = self.stream.read_buf(&mut self.input_buffer).await?;
            if read == 0 {
                if self.input_buffer.is_empty() {
                    return Ok(RfbInputEvent::Disconnected);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "VNC client disconnected during an input message",
                )
                .into());
            }
        }
    }

    fn decode_buffered_input(&mut self) -> Result<Option<RfbInputEvent>> {
        let Some(message_len) = input_message_len(&self.input_buffer)? else {
            return Ok(None);
        };
        if self.input_buffer.len() < message_len {
            return Ok(None);
        }

        let message = self.input_buffer.split_to(message_len);
        let event = match message[0] {
            0 => {
                let format = parse_pixel_format(&message[4..20]);
                self.pixel_format = format;
                RfbInputEvent::SetPixelFormat(format)
            }
            2 => {
                let was_enabled = self.configured_encoding_enabled();
                self.encodings = parse_encodings(&message[4..]);
                let encoding_enabled = self.configured_encoding_enabled();
                let resumed = !was_enabled && encoding_enabled;
                if resumed && self.config.encoding == VncEncoding::H264 {
                    self.h264_waiting_keyframe = true;
                }
                RfbInputEvent::SetEncodings {
                    encoding_enabled,
                    resumed,
                }
            }
            3 => {
                let request = FramebufferUpdateRequest {
                    incremental: message[1] != 0,
                    x: u16::from_be_bytes([message[2], message[3]]),
                    y: u16::from_be_bytes([message[4], message[5]]),
                    width: u16::from_be_bytes([message[6], message[7]]),
                    height: u16::from_be_bytes([message[8], message[9]]),
                };
                self.pending_request =
                    Some(self.pending_request.map_or(request.incremental, |pending| {
                        pending && request.incremental
                    }));
                if !request.incremental && self.config.encoding == VncEncoding::H264 {
                    self.h264_waiting_keyframe = true;
                }
                RfbInputEvent::FramebufferUpdateRequest(request)
            }
            4 => RfbInputEvent::Key(RfbKeyEvent {
                down: message[1] != 0,
                keysym: u32::from_be_bytes([message[4], message[5], message[6], message[7]]),
            }),
            5 => {
                let button_mask = message[1];
                let previous_button_mask = self.last_buttons;
                self.last_buttons = button_mask;
                RfbInputEvent::Pointer(RfbPointerEvent {
                    x: u16::from_be_bytes([message[2], message[3]]),
                    y: u16::from_be_bytes([message[4], message[5]]),
                    button_mask,
                    previous_button_mask,
                })
            }
            6 => RfbInputEvent::UnsupportedClientCutText,
            msg_type => {
                return Err(AppError::BadRequest(format!(
                    "Unsupported RFB message {}",
                    msg_type
                )))
            }
        };
        Ok(Some(event))
    }

    fn configured_encoding_enabled(&self) -> bool {
        match self.config.encoding {
            VncEncoding::TightJpeg => {
                self.encodings.has_tight
                    && self.encodings.has_jpeg_quality
                    && matches!(self.pixel_format.bits_per_pixel, 16 | 32)
                    && self.pixel_format.true_colour
            }
            VncEncoding::H264 => self.encodings.has_h264,
        }
    }

    pub fn key_event_to_hid(&mut self, event: RfbKeyEvent) -> Option<KeyboardEvent> {
        let (key, shifted) = match keysym_to_key(event.keysym) {
            Some(mapping) => mapping,
            None => {
                tracing::debug!(
                    "Ignoring unsupported VNC keysym 0x{:08x} from {}",
                    event.keysym,
                    self.peer
                );
                return None;
            }
        };

        if key.is_modifier() {
            update_modifier(&mut self.keyboard.modifiers, key, event.down);
        }

        let mut modifiers = self.keyboard.modifiers;
        if shifted && !modifiers.left_shift && !modifiers.right_shift && event.down {
            modifiers.left_shift = true;
        }

        Some(KeyboardEvent {
            event_type: if event.down {
                KeyEventType::Down
            } else {
                KeyEventType::Up
            },
            key,
            modifiers,
        })
    }

    pub async fn send_frame(&mut self, frame: &RfbFrame) -> Result<FrameSendOutcome> {
        let Some(incremental) = self.pending_request else {
            return Ok(FrameSendOutcome::NotSent);
        };
        if !self.configured_encoding_enabled() {
            return Ok(FrameSendOutcome::NotSent);
        }

        let (width, height) = frame.size();
        if width != self.width || height != self.height {
            if !self.encodings.has_resize {
                return Err(AppError::BadRequest(
                    "VNC client does not support DesktopSize resize; reconnect required"
                        .to_string(),
                ));
            }
            self.write_frame_header(width, height, ENCODING_DESKTOP_SIZE)
                .await?;
            self.stream.flush().await?;
            self.width = width.max(1);
            self.height = height.max(1);
            self.pending_request = None;
            self.last_sent_sequence = None;
            self.h264_waiting_keyframe = true;
            return Ok(FrameSendOutcome::DesktopSizeSent);
        }

        let sequence = frame.sequence();
        if incremental && self.last_sent_sequence.is_some_and(|last| sequence <= last) {
            return Ok(FrameSendOutcome::NotSent);
        }

        match frame {
            RfbFrame::Jpeg {
                data,
                width,
                height,
                ..
            } => {
                if self.config.encoding != VncEncoding::TightJpeg {
                    return Ok(FrameSendOutcome::NotSent);
                }
                self.write_frame_header(*width, *height, ENCODING_TIGHT)
                    .await?;
                write_tight_jpeg_payload(&mut self.stream, &data).await?;
            }
            RfbFrame::H264 {
                data,
                width,
                height,
                key,
                ..
            } => {
                if self.config.encoding != VncEncoding::H264 || (self.h264_waiting_keyframe && !key)
                {
                    return Ok(FrameSendOutcome::NotSent);
                }
                self.write_frame_header(*width, *height, ENCODING_H264)
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
        self.pending_request = None;
        self.last_sent_sequence = Some(sequence);
        Ok(FrameSendOutcome::FrameSent)
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

fn input_message_len(buffer: &[u8]) -> Result<Option<usize>> {
    let Some(&msg_type) = buffer.first() else {
        return Ok(None);
    };

    let len = match msg_type {
        0 => 20,
        2 => {
            if buffer.len() < 4 {
                return Ok(None);
            }
            let count = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
            if count > MAX_ENCODING_COUNT {
                return Err(AppError::BadRequest(
                    "Invalid VNC encoding list".to_string(),
                ));
            }
            4 + count * 4
        }
        3 => 10,
        4 => 8,
        5 => 6,
        6 => {
            if buffer.len() < 8 {
                return Ok(None);
            }
            let payload_len =
                u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;
            if payload_len > MAX_CLIPBOARD_SIZE {
                return Err(AppError::BadRequest(
                    "VNC clipboard message is too large".to_string(),
                ));
            }
            8 + payload_len
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unsupported RFB message {}",
                msg_type
            )))
        }
    };
    Ok(Some(len))
}

fn parse_rfb_version(version: &[u8; 12]) -> Result<RfbVersion> {
    if &version[..4] != b"RFB "
        || version[7] != b'.'
        || version[11] != b'\n'
        || !version[4..7].iter().all(u8::is_ascii_digit)
        || !version[8..11].iter().all(u8::is_ascii_digit)
    {
        return Err(AppError::BadRequest("Invalid RFB version".to_string()));
    }

    let major = (version[4] - b'0') as u16 * 100
        + (version[5] - b'0') as u16 * 10
        + (version[6] - b'0') as u16;
    if major != 3 {
        return Err(AppError::BadRequest(
            "Unsupported RFB major version".to_string(),
        ));
    }
    let minor = (version[8] - b'0') as u16 * 100
        + (version[9] - b'0') as u16 * 10
        + (version[10] - b'0') as u16;
    Ok(match minor {
        7 => RfbVersion::V3_7,
        8 => RfbVersion::V3_8,
        _ => RfbVersion::V3_3,
    })
}

fn pixel_format_bytes(format: RfbPixelFormat) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0] = format.bits_per_pixel;
    bytes[1] = format.depth;
    bytes[2] = u8::from(format.big_endian);
    bytes[3] = u8::from(format.true_colour);
    bytes[4..6].copy_from_slice(&format.red_max.to_be_bytes());
    bytes[6..8].copy_from_slice(&format.green_max.to_be_bytes());
    bytes[8..10].copy_from_slice(&format.blue_max.to_be_bytes());
    bytes[10] = format.red_shift;
    bytes[11] = format.green_shift;
    bytes[12] = format.blue_shift;
    bytes
}

fn parse_pixel_format(bytes: &[u8]) -> RfbPixelFormat {
    RfbPixelFormat {
        bits_per_pixel: bytes[0],
        depth: bytes[1],
        big_endian: bytes[2] != 0,
        true_colour: bytes[3] != 0,
        red_max: u16::from_be_bytes([bytes[4], bytes[5]]),
        green_max: u16::from_be_bytes([bytes[6], bytes[7]]),
        blue_max: u16::from_be_bytes([bytes[8], bytes[9]]),
        red_shift: bytes[10],
        green_shift: bytes[11],
        blue_shift: bytes[12],
    }
}

fn parse_encodings(bytes: &[u8]) -> ClientEncodings {
    let mut encodings = ClientEncodings::default();
    for encoding in bytes.chunks_exact(4) {
        match i32::from_be_bytes([encoding[0], encoding[1], encoding[2], encoding[3]]) {
            ENCODING_TIGHT => encodings.has_tight = true,
            ENCODING_H264 => encodings.has_h264 = true,
            ENCODING_DESKTOP_SIZE => encodings.has_resize = true,
            -32..=-23 => encodings.has_jpeg_quality = true,
            _ => {}
        }
    }
    encodings
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
        let block: &mut Block<des::Des> = chunk
            .try_into()
            .expect("VNC challenge chunks are exactly one DES block");
        cipher.encrypt_block(block);
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

fn keysym_to_key(keysym: u32) -> Option<(CanonicalKey, bool)> {
    let plain = |key| Some((key, false));
    let shifted = |key| Some((key, true));
    match keysym {
        0xff08 => plain(CanonicalKey::Backspace),
        0xff09 => plain(CanonicalKey::Tab),
        0xff0d => plain(CanonicalKey::Enter),
        0xff13 => plain(CanonicalKey::Pause),
        0xff14 => plain(CanonicalKey::ScrollLock),
        0xff15 => plain(CanonicalKey::PrintScreen),
        0xff1b => plain(CanonicalKey::Escape),
        0xff50 => plain(CanonicalKey::Home),
        0xff51 => plain(CanonicalKey::ArrowLeft),
        0xff52 => plain(CanonicalKey::ArrowUp),
        0xff53 => plain(CanonicalKey::ArrowRight),
        0xff54 => plain(CanonicalKey::ArrowDown),
        0xff55 => plain(CanonicalKey::PageUp),
        0xff56 => plain(CanonicalKey::PageDown),
        0xff57 => plain(CanonicalKey::End),
        0xff61 => plain(CanonicalKey::PrintScreen),
        0xff63 => plain(CanonicalKey::Insert),
        0xff67 => plain(CanonicalKey::ContextMenu),
        0xff6b => plain(CanonicalKey::Pause),
        0xff7f => plain(CanonicalKey::NumLock),
        0xff80 => plain(CanonicalKey::Space),
        0xff89 => plain(CanonicalKey::Tab),
        0xff8d => plain(CanonicalKey::NumpadEnter),
        0xff95 => plain(CanonicalKey::Numpad7),
        0xff96 => plain(CanonicalKey::Numpad4),
        0xff97 => plain(CanonicalKey::Numpad8),
        0xff98 => plain(CanonicalKey::Numpad6),
        0xff99 => plain(CanonicalKey::Numpad2),
        0xff9a => plain(CanonicalKey::Numpad9),
        0xff9b => plain(CanonicalKey::Numpad3),
        0xff9c => plain(CanonicalKey::Numpad1),
        0xff9d => plain(CanonicalKey::Numpad5),
        0xff9e => plain(CanonicalKey::Numpad0),
        0xff9f => plain(CanonicalKey::NumpadDecimal),
        0xffaa => plain(CanonicalKey::NumpadMultiply),
        0xffab => plain(CanonicalKey::NumpadAdd),
        0xffad => plain(CanonicalKey::NumpadSubtract),
        0xffae => plain(CanonicalKey::NumpadDecimal),
        0xffaf => plain(CanonicalKey::NumpadDivide),
        0xffb0 => plain(CanonicalKey::Numpad0),
        0xffb1 => plain(CanonicalKey::Numpad1),
        0xffb2 => plain(CanonicalKey::Numpad2),
        0xffb3 => plain(CanonicalKey::Numpad3),
        0xffb4 => plain(CanonicalKey::Numpad4),
        0xffb5 => plain(CanonicalKey::Numpad5),
        0xffb6 => plain(CanonicalKey::Numpad6),
        0xffb7 => plain(CanonicalKey::Numpad7),
        0xffb8 => plain(CanonicalKey::Numpad8),
        0xffb9 => plain(CanonicalKey::Numpad9),
        0xffbd => plain(CanonicalKey::Equal),
        0xffbe..=0xffc9 => {
            CanonicalKey::from_hid_usage((keysym - 0xffbe + 0x3a) as u8).map(|key| (key, false))
        }
        0xffca..=0xffd5 => {
            CanonicalKey::from_hid_usage((keysym - 0xffca + 0x68) as u8).map(|key| (key, false))
        }
        0xffe1 => plain(CanonicalKey::ShiftLeft),
        0xffe2 => plain(CanonicalKey::ShiftRight),
        0xffe3 => plain(CanonicalKey::ControlLeft),
        0xffe4 => plain(CanonicalKey::ControlRight),
        0xffe5 | 0xffe6 => plain(CanonicalKey::CapsLock),
        0xffe7 | 0xffeb => plain(CanonicalKey::MetaLeft),
        0xffe8 | 0xffec => plain(CanonicalKey::MetaRight),
        0xffe9 => plain(CanonicalKey::AltLeft),
        0xffea => plain(CanonicalKey::AltRight),
        0xffff => plain(CanonicalKey::Delete),
        0x20 => plain(CanonicalKey::Space),
        0x61..=0x7a => {
            CanonicalKey::from_hid_usage((keysym - 0x61 + 0x04) as u8).map(|key| (key, false))
        }
        0x41..=0x5a => {
            CanonicalKey::from_hid_usage((keysym - 0x41 + 0x04) as u8).map(|key| (key, true))
        }
        0x31..=0x39 => {
            CanonicalKey::from_hid_usage((keysym - 0x31 + 0x1e) as u8).map(|key| (key, false))
        }
        0x30 => plain(CanonicalKey::Digit0),
        0x21 => shifted(CanonicalKey::Digit1),
        0x40 => shifted(CanonicalKey::Digit2),
        0x23 => shifted(CanonicalKey::Digit3),
        0x24 => shifted(CanonicalKey::Digit4),
        0x25 => shifted(CanonicalKey::Digit5),
        0x5e => shifted(CanonicalKey::Digit6),
        0x26 => shifted(CanonicalKey::Digit7),
        0x2a => shifted(CanonicalKey::Digit8),
        0x28 => shifted(CanonicalKey::Digit9),
        0x29 => shifted(CanonicalKey::Digit0),
        0x5f => shifted(CanonicalKey::Minus),
        0x2b => shifted(CanonicalKey::Equal),
        0x7b => shifted(CanonicalKey::BracketLeft),
        0x7d => shifted(CanonicalKey::BracketRight),
        0x7c => shifted(CanonicalKey::Backslash),
        0x3a => shifted(CanonicalKey::Semicolon),
        0x22 => shifted(CanonicalKey::Quote),
        0x7e => shifted(CanonicalKey::Backquote),
        0x3c => shifted(CanonicalKey::Comma),
        0x3e => shifted(CanonicalKey::Period),
        0x3f => shifted(CanonicalKey::Slash),
        0x2d => plain(CanonicalKey::Minus),
        0x3d => plain(CanonicalKey::Equal),
        0x5b => plain(CanonicalKey::BracketLeft),
        0x5d => plain(CanonicalKey::BracketRight),
        0x5c => plain(CanonicalKey::Backslash),
        0x3b => plain(CanonicalKey::Semicolon),
        0x27 => plain(CanonicalKey::Quote),
        0x60 => plain(CanonicalKey::Backquote),
        0x2c => plain(CanonicalKey::Comma),
        0x2e => plain(CanonicalKey::Period),
        0x2f => plain(CanonicalKey::Slash),
        _ => None,
    }
}

fn update_modifier(modifiers: &mut KeyboardModifiers, key: CanonicalKey, down: bool) {
    match key {
        CanonicalKey::ControlLeft => modifiers.left_ctrl = down,
        CanonicalKey::ShiftLeft => modifiers.left_shift = down,
        CanonicalKey::AltLeft => modifiers.left_alt = down,
        CanonicalKey::MetaLeft => modifiers.left_meta = down,
        CanonicalKey::ControlRight => modifiers.right_ctrl = down,
        CanonicalKey::ShiftRight => modifiers.right_shift = down,
        CanonicalKey::AltRight => modifiers.right_alt = down,
        CanonicalKey::MetaRight => modifiers.right_meta = down,
        _ => {}
    }
}

pub fn pointer_event_to_hid(event: RfbPointerEvent, width: u16, height: u16) -> Vec<MouseEvent> {
    let mut out = Vec::new();
    let max_x = width.saturating_sub(1);
    let max_y = height.saturating_sub(1);
    let abs_x = if max_x == 0 {
        0
    } else {
        ((event.x.min(max_x) as u64 * 32767) / max_x as u64) as i32
    };
    let abs_y = if max_y == 0 {
        0
    } else {
        ((event.y.min(max_y) as u64 * 32767) / max_y as u64) as i32
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::{poll_fn, Future};
    use std::task::Poll;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn tcp_pair() -> (RfbClient, TcpStream) {
        tcp_pair_with_config(VncConfig::default()).await
    }

    async fn tcp_pair_with_config(config: VncConfig) -> (RfbClient, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind test listener");
        let addr = listener.local_addr().expect("missing listener address");
        let (accepted, connected) = tokio::join!(listener.accept(), TcpStream::connect(addr));
        let (server, peer) = accepted.expect("failed to accept test connection");
        let client = connected.expect("failed to connect test client");
        (RfbClient::new(server, peer, config), client)
    }

    fn set_encodings_message(encodings: &[i32]) -> Vec<u8> {
        let mut message = vec![2, 0];
        message.extend_from_slice(&(encodings.len() as u16).to_be_bytes());
        for encoding in encodings {
            message.extend_from_slice(&encoding.to_be_bytes());
        }
        message
    }

    fn update_request(incremental: bool) -> [u8; 10] {
        [3, u8::from(incremental), 0, 0, 0, 0, 0x03, 0x20, 0x02, 0x58]
    }

    async fn complete_client_handshake(client: &mut TcpStream, version: &[u8; 12]) {
        let mut server_version = [0u8; 12];
        client
            .read_exact(&mut server_version)
            .await
            .expect("missing server version");
        assert_eq!(&server_version, b"RFB 003.008\n");
        client
            .write_all(version)
            .await
            .expect("failed to send client version");

        match parse_rfb_version(version).expect("test version should parse") {
            RfbVersion::V3_3 => {
                let mut security = [0u8; 4];
                client
                    .read_exact(&mut security)
                    .await
                    .expect("missing 3.3 security type");
                assert_eq!(u32::from_be_bytes(security), SECURITY_TYPE_VNC_AUTH as u32);
            }
            RfbVersion::V3_7 | RfbVersion::V3_8 => {
                let mut security = [0u8; 2];
                client
                    .read_exact(&mut security)
                    .await
                    .expect("missing security list");
                assert_eq!(security, [1, SECURITY_TYPE_VNC_AUTH]);
                client
                    .write_all(&[SECURITY_TYPE_VNC_AUTH])
                    .await
                    .expect("failed to choose security type");
            }
        }

        let mut challenge = [0u8; 16];
        client
            .read_exact(&mut challenge)
            .await
            .expect("missing VNC challenge");
        client
            .write_all(
                &encrypt_vnc_challenge(&challenge, "secret").expect("challenge encryption failed"),
            )
            .await
            .expect("failed to send challenge response");
        let mut security_result = [0u8; 4];
        client
            .read_exact(&mut security_result)
            .await
            .expect("missing security result");
        assert_eq!(security_result, 0u32.to_be_bytes());

        client
            .write_all(&[1])
            .await
            .expect("failed to send ClientInit");
        let mut server_init = [0u8; 24];
        client
            .read_exact(&mut server_init)
            .await
            .expect("missing ServerInit");
        let name_len = u32::from_be_bytes([
            server_init[20],
            server_init[21],
            server_init[22],
            server_init[23],
        ]) as usize;
        let mut name = vec![0u8; name_len];
        client
            .read_exact(&mut name)
            .await
            .expect("missing desktop name");
        assert_eq!(&name, b"One-KVM VNC");

        let mut messages = set_encodings_message(&[ENCODING_TIGHT, -23, ENCODING_DESKTOP_SIZE]);
        messages.extend_from_slice(&update_request(true));
        client
            .write_all(&messages)
            .await
            .expect("failed to send initial messages");
    }

    #[test]
    fn version_parser_supports_and_downgrades_rfb_3x() {
        assert_eq!(
            parse_rfb_version(b"RFB 003.003\n").unwrap(),
            RfbVersion::V3_3
        );
        assert_eq!(
            parse_rfb_version(b"RFB 003.007\n").unwrap(),
            RfbVersion::V3_7
        );
        assert_eq!(
            parse_rfb_version(b"RFB 003.008\n").unwrap(),
            RfbVersion::V3_8
        );
        assert_eq!(
            parse_rfb_version(b"RFB 003.889\n").unwrap(),
            RfbVersion::V3_3
        );
        assert!(parse_rfb_version(b"RFB 004.008\n").is_err());
        assert!(parse_rfb_version(b"RFB 003.08x\n").is_err());
        assert!(parse_rfb_version(b"RFB 003-008\n").is_err());
    }

    #[tokio::test]
    async fn handshake_supports_all_versions_and_preserves_coalesced_request() {
        for version in [
            b"RFB 003.003\n",
            b"RFB 003.007\n",
            b"RFB 003.008\n",
            b"RFB 003.889\n",
        ] {
            let config = VncConfig {
                password: Some("secret".to_string()),
                ..VncConfig::default()
            };
            let (mut server, mut client) = tcp_pair_with_config(config).await;
            let (server_result, ()) = tokio::join!(server.handshake(), async {
                complete_client_handshake(&mut client, version).await;
            });
            server_result.expect("handshake should succeed");
            assert!(server.shared());
            assert!(matches!(
                server
                    .read_input_event()
                    .await
                    .expect("coalesced request was lost"),
                RfbInputEvent::FramebufferUpdateRequest(_)
            ));
        }
    }

    #[tokio::test]
    async fn auth_failure_reason_is_only_sent_for_rfb_3_8() {
        for (version, expected_tail) in [
            (RfbVersion::V3_3, Vec::new()),
            (RfbVersion::V3_7, Vec::new()),
            (
                RfbVersion::V3_8,
                [
                    &("Invalid VNC password".len() as u32).to_be_bytes()[..],
                    b"Invalid VNC password",
                ]
                .concat(),
            ),
        ] {
            let config = VncConfig {
                password: Some("secret".to_string()),
                ..VncConfig::default()
            };
            let (mut server, mut client) = tcp_pair_with_config(config).await;
            let task = tokio::spawn(async move { server.handle_vnc_auth(version).await });
            let mut challenge = [0u8; 16];
            client.read_exact(&mut challenge).await.unwrap();
            let mut wrong = encrypt_vnc_challenge(&challenge, "secret").unwrap();
            wrong[0] ^= 0xff;
            client.write_all(&wrong).await.unwrap();
            assert!(task.await.unwrap().is_err());
            let mut result = Vec::new();
            client.read_to_end(&mut result).await.unwrap();
            assert_eq!(&result[..4], &1u32.to_be_bytes());
            assert_eq!(&result[4..], expected_tail);
        }
    }

    #[tokio::test]
    async fn input_read_resumes_after_cancellation_mid_message() {
        let (mut server, mut client) = tcp_pair().await;
        client
            .write_all(&[3])
            .await
            .expect("failed to write message type");
        server
            .stream
            .readable()
            .await
            .expect("server stream did not become readable");

        let mut read = Box::pin(server.read_input_event());
        let state = poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx))).await;
        assert!(state.is_pending());
        drop(read);
        assert_eq!(&server.input_buffer[..], &[3]);

        client
            .write_all(&[
                1, 0, 0, 0, 0, 0x05, 0x00, 0x02, 0xd0, // update request body
                5, 1, 0, 10, 0, 20, // pointer event
                5, 0, 0, 11, 0, 21, // pointer release
            ])
            .await
            .expect("failed to write remaining messages");

        assert!(matches!(
            server.read_input_event().await.expect("update read failed"),
            RfbInputEvent::FramebufferUpdateRequest(FramebufferUpdateRequest {
                incremental: true,
                ..
            })
        ));
        match server
            .read_input_event()
            .await
            .expect("pointer read failed")
        {
            RfbInputEvent::Pointer(pointer) => {
                assert_eq!(pointer.button_mask, 1);
                assert_eq!(pointer.previous_button_mask, 0);
                assert_eq!(pointer.x, 10);
                assert_eq!(pointer.y, 20);
            }
            _ => panic!("expected pointer event"),
        }
        match server
            .read_input_event()
            .await
            .expect("pointer release read failed")
        {
            RfbInputEvent::Pointer(pointer) => {
                assert_eq!(pointer.button_mask, 0);
                assert_eq!(pointer.previous_button_mask, 1);
                assert_eq!(pointer.x, 11);
                assert_eq!(pointer.y, 21);
            }
            _ => panic!("expected pointer release event"),
        }
    }

    #[tokio::test]
    async fn coalesced_input_messages_keep_their_boundaries() {
        let (mut server, mut client) = tcp_pair().await;
        client
            .write_all(&[
                4, 1, 0, 0, 0, 0, 0, 0x61, // key down: a
                6, 0, 0, 0, 0, 0, 0, 3, b'a', b'b', b'c', // clipboard
            ])
            .await
            .expect("failed to write coalesced messages");

        match server.read_input_event().await.expect("key read failed") {
            RfbInputEvent::Key(key) => {
                assert!(key.down);
                assert_eq!(key.keysym, 0x61);
            }
            _ => panic!("expected key event"),
        }
        assert!(matches!(
            server
                .read_input_event()
                .await
                .expect("clipboard read failed"),
            RfbInputEvent::UnsupportedClientCutText
        ));
    }

    #[test]
    fn input_message_limits_are_enforced_before_allocation() {
        let encodings = input_message_len(&[2, 0, 0x04, 0x01]);
        assert!(matches!(encodings, Err(AppError::BadRequest(_))));

        let clipboard = input_message_len(&[6, 0, 0, 0, 0, 0x10, 0, 1]);
        assert!(matches!(clipboard, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn eof_distinguishes_disconnect_from_truncated_message() {
        let (mut clean_server, clean_client) = tcp_pair().await;
        drop(clean_client);
        assert!(matches!(
            clean_server
                .read_input_event()
                .await
                .expect("clean disconnect failed"),
            RfbInputEvent::Disconnected
        ));

        let (mut partial_server, mut partial_client) = tcp_pair().await;
        partial_client
            .write_all(&[4, 1])
            .await
            .expect("failed to write partial message");
        drop(partial_client);
        match partial_server.read_input_event().await {
            Err(AppError::Io(err)) => {
                assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof)
            }
            _ => panic!("expected truncated message error"),
        }
    }

    #[tokio::test]
    async fn set_encodings_and_pixel_format_control_tight_availability() {
        let (mut server, _client) = tcp_pair().await;
        server
            .input_buffer
            .extend_from_slice(&set_encodings_message(&[ENCODING_TIGHT, -23]));
        assert!(matches!(
            server.decode_buffered_input().unwrap(),
            Some(RfbInputEvent::SetEncodings {
                encoding_enabled: true,
                resumed: true,
            })
        ));

        let mut indexed_format = vec![0, 0, 0, 0];
        let mut format = RfbPixelFormat::default();
        format.true_colour = false;
        indexed_format.extend_from_slice(&pixel_format_bytes(format));
        server.input_buffer.extend_from_slice(&indexed_format);
        assert!(matches!(
            server.decode_buffered_input().unwrap(),
            Some(RfbInputEvent::SetPixelFormat(RfbPixelFormat {
                true_colour: false,
                ..
            }))
        ));
        assert!(!server.configured_encoding_enabled());

        let mut true_colour_16 = vec![0, 0, 0, 0];
        format.bits_per_pixel = 16;
        format.depth = 16;
        format.true_colour = true;
        true_colour_16.extend_from_slice(&pixel_format_bytes(format));
        server.input_buffer.extend_from_slice(&true_colour_16);
        server.decode_buffered_input().unwrap();
        assert!(server.configured_encoding_enabled());

        server
            .input_buffer
            .extend_from_slice(&set_encodings_message(&[]));
        assert!(matches!(
            server.decode_buffered_input().unwrap(),
            Some(RfbInputEvent::SetEncodings {
                encoding_enabled: false,
                resumed: false,
            })
        ));
        server
            .input_buffer
            .extend_from_slice(&set_encodings_message(&[ENCODING_TIGHT, -32]));
        assert!(matches!(
            server.decode_buffered_input().unwrap(),
            Some(RfbInputEvent::SetEncodings {
                encoding_enabled: true,
                resumed: true,
            })
        ));
    }

    #[tokio::test]
    async fn keyboard_tracks_modifiers_and_synthesizes_shift() {
        let (mut server, _client) = tcp_pair().await;
        let ctrl = server
            .key_event_to_hid(RfbKeyEvent {
                down: true,
                keysym: 0xffe3,
            })
            .unwrap();
        assert_eq!(ctrl.key, CanonicalKey::ControlLeft);
        let alt = server
            .key_event_to_hid(RfbKeyEvent {
                down: true,
                keysym: 0xffea,
            })
            .unwrap();
        assert!(alt.modifiers.left_ctrl);
        assert!(alt.modifiers.right_alt);
        let delete = server
            .key_event_to_hid(RfbKeyEvent {
                down: true,
                keysym: 0xffff,
            })
            .unwrap();
        assert_eq!(delete.key, CanonicalKey::Delete);
        assert!(delete.modifiers.left_ctrl);
        assert!(delete.modifiers.right_alt);

        let uppercase = server
            .key_event_to_hid(RfbKeyEvent {
                down: true,
                keysym: b'A' as u32,
            })
            .unwrap();
        assert!(uppercase.modifiers.left_shift);
        let uppercase_up = server
            .key_event_to_hid(RfbKeyEvent {
                down: false,
                keysym: b'A' as u32,
            })
            .unwrap();
        assert!(!uppercase_up.modifiers.left_shift);
        assert!(uppercase_up.modifiers.left_ctrl);
        assert!(uppercase_up.modifiers.right_alt);

        assert!(server
            .key_event_to_hid(RfbKeyEvent {
                down: true,
                keysym: 0x0101_f642,
            })
            .is_none());
        let plain = server
            .key_event_to_hid(RfbKeyEvent {
                down: true,
                keysym: b'a' as u32,
            })
            .unwrap();
        assert!(!plain.modifiers.left_shift);
        assert!(plain.modifiers.left_ctrl);
        assert!(plain.modifiers.right_alt);
    }

    #[test]
    fn shifted_symbols_and_extended_keys_map_to_canonical_keys() {
        for (keysym, expected) in [
            (b'!' as u32, CanonicalKey::Digit1),
            (b'@' as u32, CanonicalKey::Digit2),
            (b'#' as u32, CanonicalKey::Digit3),
            (b'$' as u32, CanonicalKey::Digit4),
            (b'%' as u32, CanonicalKey::Digit5),
            (b'^' as u32, CanonicalKey::Digit6),
            (b'&' as u32, CanonicalKey::Digit7),
            (b'*' as u32, CanonicalKey::Digit8),
            (b'(' as u32, CanonicalKey::Digit9),
            (b')' as u32, CanonicalKey::Digit0),
            (b'_' as u32, CanonicalKey::Minus),
            (b'+' as u32, CanonicalKey::Equal),
            (b'{' as u32, CanonicalKey::BracketLeft),
            (b'}' as u32, CanonicalKey::BracketRight),
            (b'|' as u32, CanonicalKey::Backslash),
            (b':' as u32, CanonicalKey::Semicolon),
            (b'"' as u32, CanonicalKey::Quote),
            (b'~' as u32, CanonicalKey::Backquote),
            (b'<' as u32, CanonicalKey::Comma),
            (b'>' as u32, CanonicalKey::Period),
            (b'?' as u32, CanonicalKey::Slash),
        ] {
            assert_eq!(keysym_to_key(keysym), Some((expected, true)));
        }

        assert_eq!(keysym_to_key(0xffe2).unwrap().0, CanonicalKey::ShiftRight);
        assert_eq!(keysym_to_key(0xff61).unwrap().0, CanonicalKey::PrintScreen);
        assert_eq!(keysym_to_key(0xff67).unwrap().0, CanonicalKey::ContextMenu);
        assert_eq!(keysym_to_key(0xffd5).unwrap().0, CanonicalKey::F24);
        assert_eq!(keysym_to_key(0xffb0).unwrap().0, CanonicalKey::Numpad0);
        assert_eq!(keysym_to_key(0xffb9).unwrap().0, CanonicalKey::Numpad9);
        assert_eq!(keysym_to_key(0xffaf).unwrap().0, CanonicalKey::NumpadDivide);
        assert_eq!(keysym_to_key(0xffe5).unwrap().0, CanonicalKey::CapsLock);
        assert_eq!(keysym_to_key(0xff7f).unwrap().0, CanonicalKey::NumLock);
    }

    #[test]
    fn pointer_coordinates_use_endpoint_mapping_and_clamp() {
        let event = |x, y| RfbPointerEvent {
            x,
            y,
            button_mask: 0,
            previous_button_mask: 0,
        };
        let top_left = pointer_event_to_hid(event(0, 0), 1920, 1080);
        assert_eq!((top_left[0].x, top_left[0].y), (0, 0));
        let bottom_right = pointer_event_to_hid(event(1919, 1079), 1920, 1080);
        assert_eq!((bottom_right[0].x, bottom_right[0].y), (32767, 32767));
        let clamped = pointer_event_to_hid(event(u16::MAX, u16::MAX), 1280, 720);
        assert_eq!((clamped[0].x, clamped[0].y), (32767, 32767));
        let resized = pointer_event_to_hid(event(1279, 719), 1280, 720);
        assert_eq!((resized[0].x, resized[0].y), (32767, 32767));
    }

    #[tokio::test]
    async fn framebuffer_requests_merge_and_jpeg_replay_obeys_sequence() {
        let (mut server, mut client) = tcp_pair().await;
        server.encodings = ClientEncodings {
            has_tight: true,
            has_jpeg_quality: true,
            has_resize: true,
            ..ClientEncodings::default()
        };
        let frame = RfbFrame::Jpeg {
            data: Bytes::from_static(b"jpeg"),
            width: 800,
            height: 600,
            sequence: 7,
        };

        assert_eq!(
            server.send_frame(&frame).await.unwrap(),
            FrameSendOutcome::NotSent
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), client.read_u8())
                .await
                .is_err()
        );

        server.input_buffer.extend_from_slice(&update_request(true));
        server
            .input_buffer
            .extend_from_slice(&update_request(false));
        server.decode_buffered_input().unwrap();
        server.decode_buffered_input().unwrap();
        assert_eq!(server.pending_request, Some(false));
        assert_eq!(
            server.send_frame(&frame).await.unwrap(),
            FrameSendOutcome::FrameSent
        );
        assert!(!server.has_pending_request());

        server.pending_request = Some(true);
        assert_eq!(
            server.send_frame(&frame).await.unwrap(),
            FrameSendOutcome::NotSent
        );
        assert!(server.has_pending_request());
        server.pending_request = Some(false);
        assert_eq!(
            server.send_frame(&frame).await.unwrap(),
            FrameSendOutcome::FrameSent
        );
    }

    #[tokio::test]
    async fn desktop_size_consumes_request_and_h264_waits_for_keyframe() {
        let config = VncConfig {
            encoding: VncEncoding::H264,
            ..VncConfig::default()
        };
        let (mut server, _client) = tcp_pair_with_config(config).await;
        server.encodings = ClientEncodings {
            has_h264: true,
            has_resize: true,
            ..ClientEncodings::default()
        };
        server.pending_request = Some(true);
        let non_key = RfbFrame::H264 {
            data: Bytes::from_static(b"p"),
            width: 1024,
            height: 768,
            key: false,
            sequence: 10,
        };
        assert_eq!(
            server.send_frame(&non_key).await.unwrap(),
            FrameSendOutcome::DesktopSizeSent
        );
        assert!(!server.has_pending_request());
        assert_eq!(server.last_sent_sequence, None);
        assert_eq!(server.framebuffer_size(), (1024, 768));

        server.pending_request = Some(true);
        assert_eq!(
            server.send_frame(&non_key).await.unwrap(),
            FrameSendOutcome::NotSent
        );
        assert!(server.has_pending_request());
        let key = RfbFrame::H264 {
            data: Bytes::from_static(b"i"),
            width: 1024,
            height: 768,
            key: true,
            sequence: 11,
        };
        assert_eq!(
            server.send_frame(&key).await.unwrap(),
            FrameSendOutcome::FrameSent
        );
        assert!(!server.has_pending_request());
        assert_eq!(server.last_sent_sequence, Some(11));
    }
}

//! Variable-length TCP framing (RustDesk wire format).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_PACKET_LENGTH: usize = 0x3FFFFFFF;

pub fn encode_frame(data: &[u8]) -> io::Result<Vec<u8>> {
    let len = data.len();
    let mut buf = Vec::with_capacity(len + 4);

    if len <= 0x3F {
        buf.push((len << 2) as u8);
    } else if len <= 0x3FFF {
        let h = ((len << 2) as u16) | 0x1;
        buf.extend_from_slice(&h.to_le_bytes());
    } else if len <= 0x3FFFFF {
        let h = ((len << 2) as u32) | 0x2;
        buf.push((h & 0xFF) as u8);
        buf.push(((h >> 8) & 0xFF) as u8);
        buf.push(((h >> 16) & 0xFF) as u8);
    } else if len <= MAX_PACKET_LENGTH {
        let h = ((len << 2) as u32) | 0x3;
        buf.extend_from_slice(&h.to_le_bytes());
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Message too large",
        ));
    }

    buf.extend_from_slice(data);
    Ok(buf)
}

fn decode_header(first_byte: u8, header_bytes: &[u8]) -> (usize, usize) {
    let head_len = ((first_byte & 0x3) + 1) as usize;

    let mut n = first_byte as usize;
    if head_len > 1 && !header_bytes.is_empty() {
        n |= (header_bytes[0] as usize) << 8;
    }
    if head_len > 2 && header_bytes.len() >= 2 {
        n |= (header_bytes[1] as usize) << 16;
    }
    if head_len > 3 && header_bytes.len() >= 3 {
        n |= (header_bytes[2] as usize) << 24;
    }

    let msg_len = n >> 2;
    (head_len, msg_len)
}

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<BytesMut> {
    let mut first_byte = [0u8; 1];
    reader.read_exact(&mut first_byte).await?;

    let head_len = ((first_byte[0] & 0x3) + 1) as usize;

    let mut header_rest = [0u8; 3];
    if head_len > 1 {
        reader.read_exact(&mut header_rest[..head_len - 1]).await?;
    }

    let (_, msg_len) = decode_header(first_byte[0], &header_rest);

    if msg_len > MAX_PACKET_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Message too large",
        ));
    }

    let mut buf = BytesMut::with_capacity(msg_len);
    buf.resize(msg_len, 0);
    reader.read_exact(&mut buf).await?;

    Ok(buf)
}

pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, data: &[u8]) -> io::Result<()> {
    let frame = encode_frame(data)?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn write_frame_buffered<W: AsyncWrite + Unpin>(
    writer: &mut W,
    data: &[u8],
    buf: &mut BytesMut,
) -> io::Result<()> {
    buf.clear();
    encode_frame_into(data, buf)?;
    writer.write_all(buf).await?;
    writer.flush().await?;
    Ok(())
}

pub fn encode_frame_into(data: &[u8], buf: &mut BytesMut) -> io::Result<()> {
    let len = data.len();

    buf.reserve(4 + len);

    if len <= 0x3F {
        buf.put_u8((len << 2) as u8);
    } else if len <= 0x3FFF {
        buf.put_u16_le(((len << 2) as u16) | 0x1);
    } else if len <= 0x3FFFFF {
        let h = ((len << 2) as u32) | 0x2;
        buf.put_u8((h & 0xFF) as u8);
        buf.put_u8(((h >> 8) & 0xFF) as u8);
        buf.put_u8(((h >> 16) & 0xFF) as u8);
    } else if len <= MAX_PACKET_LENGTH {
        buf.put_u32_le(((len << 2) as u32) | 0x3);
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Message too large",
        ));
    }

    buf.extend_from_slice(data);
    Ok(())
}

/// Stateful decoder for `Framed`.
#[derive(Debug, Clone, Copy)]
pub struct BytesCodec {
    state: DecodeState,
    max_packet_length: usize,
}

#[derive(Debug, Clone, Copy)]
enum DecodeState {
    Head,
    Data(usize),
}

impl Default for BytesCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl BytesCodec {
    pub fn new() -> Self {
        Self {
            state: DecodeState::Head,
            max_packet_length: MAX_PACKET_LENGTH,
        }
    }

    pub fn set_max_packet_length(&mut self, n: usize) {
        self.max_packet_length = n;
    }

    pub fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        let n = match self.state {
            DecodeState::Head => match self.decode_head(src)? {
                Some(n) => {
                    self.state = DecodeState::Data(n);
                    n
                }
                None => return Ok(None),
            },
            DecodeState::Data(n) => n,
        };

        match self.decode_data(n, src)? {
            Some(data) => {
                self.state = DecodeState::Head;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    fn decode_head(&mut self, src: &mut BytesMut) -> io::Result<Option<usize>> {
        if src.is_empty() {
            return Ok(None);
        }

        let head_len = ((src[0] & 0x3) + 1) as usize;
        if src.len() < head_len {
            return Ok(None);
        }

        let mut n = src[0] as usize;
        if head_len > 1 {
            n |= (src[1] as usize) << 8;
        }
        if head_len > 2 {
            n |= (src[2] as usize) << 16;
        }
        if head_len > 3 {
            n |= (src[3] as usize) << 24;
        }
        n >>= 2;

        if n > self.max_packet_length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Message too large",
            ));
        }

        src.advance(head_len);
        Ok(Some(n))
    }

    fn decode_data(&self, n: usize, src: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        if src.len() < n {
            return Ok(None);
        }
        Ok(Some(src.split_to(n)))
    }

    pub fn encode(&mut self, data: Bytes, buf: &mut BytesMut) -> io::Result<()> {
        let len = data.len();

        if len <= 0x3F {
            buf.put_u8((len << 2) as u8);
        } else if len <= 0x3FFF {
            buf.put_u16_le(((len << 2) as u16) | 0x1);
        } else if len <= 0x3FFFFF {
            let h = ((len << 2) as u32) | 0x2;
            buf.put_u16_le((h & 0xFFFF) as u16);
            buf.put_u8((h >> 16) as u8);
        } else if len <= MAX_PACKET_LENGTH {
            buf.put_u32_le(((len << 2) as u32) | 0x3);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Message too large",
            ));
        }

        buf.extend(data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_small() {
        let data = vec![1u8; 63];
        let encoded = encode_frame(&data).unwrap();
        assert_eq!(encoded.len(), 63 + 1);

        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::from(&encoded[..]);
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.len(), 63);
    }

    #[test]
    fn test_encode_decode_medium() {
        let data = vec![2u8; 1000];
        let encoded = encode_frame(&data).unwrap();
        assert_eq!(encoded.len(), 1000 + 2);

        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::from(&encoded[..]);
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.len(), 1000);
    }

    #[test]
    fn test_encode_decode_large() {
        let data = vec![3u8; 100000];
        let encoded = encode_frame(&data).unwrap();
        assert_eq!(encoded.len(), 100000 + 3);

        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::from(&encoded[..]);
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.len(), 100000);
    }
}

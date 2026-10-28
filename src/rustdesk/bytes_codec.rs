//! Variable-length TCP framing (RustDesk wire format).

use bytes::BytesMut;
use std::io::{self, IoSlice};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_PACKET_LENGTH: usize = 0x3FFFFFFF;

fn encode_header(len: usize) -> io::Result<([u8; 4], usize)> {
    let header_len = match len {
        0..=0x3F => 1,
        0x40..=0x3FFF => 2,
        0x4000..=0x3FFFFF => 3,
        0x400000..=MAX_PACKET_LENGTH => 4,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Message too large",
            ));
        }
    };
    let header = (((len as u32) << 2) | (header_len as u32 - 1)).to_le_bytes();
    Ok((header, header_len))
}

/// Read one framed message while enforcing a caller-selected allocation limit.
///
/// Network-facing protocol stages should use a substantially smaller limit than
/// the wire format's theoretical maximum so an untrusted peer cannot force a
/// huge allocation by sending only a length header.
pub async fn read_frame_with_limit<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_packet_length: usize,
) -> io::Result<BytesMut> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header[..1]).await?;
    let header_len = ((header[0] & 0x3) + 1) as usize;
    reader.read_exact(&mut header[1..header_len]).await?;
    let msg_len = (u32::from_le_bytes(header) >> 2) as usize;

    if msg_len > max_packet_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {msg_len} bytes exceeds {max_packet_length}-byte limit"),
        ));
    }

    let mut buf = BytesMut::with_capacity(msg_len);
    buf.resize(msg_len, 0);
    reader.read_exact(&mut buf).await?;

    Ok(buf)
}

pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, data: &[u8]) -> io::Result<()> {
    write_frame_vectored(writer, data).await?;
    writer.flush().await?;
    Ok(())
}

/// Write a frame without copying its payload into a second contiguous buffer.
/// TCP writers normally send the small header and payload in one vectored write.
pub async fn write_frame_vectored<W: AsyncWrite + Unpin>(
    writer: &mut W,
    data: &[u8],
) -> io::Result<()> {
    let (header, header_len) = encode_header(data.len())?;

    let slices = [IoSlice::new(&header[..header_len]), IoSlice::new(data)];
    let written = writer.write_vectored(&slices).await?;
    if written == 0 {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "failed to write RustDesk frame",
        ));
    }
    if written < header_len {
        writer.write_all(&header[written..header_len]).await?;
        writer.write_all(data).await?;
    } else {
        writer.write_all(&data[written - header_len..]).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct ShortWriter {
        bytes: Vec<u8>,
        max_write: usize,
    }

    impl AsyncWrite for ShortWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let len = buf.len().min(self.max_write);
            self.bytes.extend_from_slice(&buf[..len]);
            Poll::Ready(Ok(len))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            let mut written = 0;
            for buf in bufs {
                let len = buf.len().min(self.max_write - written);
                self.bytes.extend_from_slice(&buf[..len]);
                written += len;
            }
            Poll::Ready(Ok(written))
        }

        fn is_write_vectored(&self) -> bool {
            true
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn header_boundaries_match_wire_format() {
        for (len, expected) in [
            (0, &[0x00][..]),
            (63, &[0xfc][..]),
            (64, &[0x01, 0x01][..]),
            (16383, &[0xfd, 0xff][..]),
            (16384, &[0x02, 0x00, 0x01][..]),
            (4194303, &[0xfe, 0xff, 0xff][..]),
            (4194304, &[0x03, 0x00, 0x00, 0x01][..]),
            (MAX_PACKET_LENGTH, &[0xff, 0xff, 0xff, 0xff][..]),
        ] {
            let (header, header_len) = encode_header(len).unwrap();
            assert_eq!(&header[..header_len], expected, "payload length {len}");
        }
        assert_eq!(
            encode_header(MAX_PACKET_LENGTH + 1).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[tokio::test]
    async fn frame_round_trips_at_header_boundaries() {
        for len in [0, 63, 64, 16383, 16384, 4194303, 4194304] {
            let payload = vec![0x5a; len];
            let mut encoded = Vec::new();
            write_frame(&mut encoded, &payload).await.unwrap();
            let mut reader = encoded.as_slice();
            let decoded = read_frame_with_limit(&mut reader, len).await.unwrap();
            assert_eq!(decoded.as_ref(), payload.as_slice());
            assert!(reader.is_empty());
        }
    }

    #[tokio::test]
    async fn read_limit_rejects_length_before_allocating_payload() {
        // A 1024-byte length header with no payload must fail the limit check,
        // rather than attempt to read the missing payload and return EOF.
        let mut reader = &[0x01, 0x10][..];
        let error = read_frame_with_limit(&mut reader, 128).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn truncated_frames_return_unexpected_eof() {
        for bytes in [
            &[0x03][..],
            &[0x03, 0x00][..],
            &[0x03, 0x00, 0x00][..],
            &[0x08, 0x5a][..],
        ] {
            let mut reader = bytes;
            let error = read_frame_with_limit(&mut reader, 1024).await.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        }
    }

    #[tokio::test]
    async fn consecutive_frames_preserve_message_boundaries() {
        let mut reader = &[0x08, 0x11, 0x22, 0x00, 0x04, 0x33][..];
        for expected in [&[0x11, 0x22][..], &[][..], &[0x33][..]] {
            let frame = read_frame_with_limit(&mut reader, 2).await.unwrap();
            assert_eq!(frame.as_ref(), expected);
        }
        assert!(reader.is_empty());
    }

    #[tokio::test]
    async fn vectored_writer_completes_partial_header_and_payload_writes() {
        let payload = vec![0x5a; 16384];
        for max_write in [1, 2, 3, 4, 1024] {
            let mut writer = ShortWriter {
                bytes: Vec::new(),
                max_write,
            };
            write_frame_vectored(&mut writer, &payload).await.unwrap();
            assert_eq!(&writer.bytes[..3], &[0x02, 0x00, 0x01]);
            assert_eq!(&writer.bytes[3..], payload.as_slice());
        }
    }

    #[tokio::test]
    async fn vectored_writer_reports_zero_writes() {
        let mut writer = ShortWriter {
            bytes: Vec::new(),
            max_write: 0,
        };
        let error = write_frame_vectored(&mut writer, b"payload")
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }

    #[tokio::test]
    async fn vectored_writer_round_trips() {
        let payload = vec![0x5a; 100_000];
        let (mut writer, mut reader) = tokio::io::duplex(payload.len() + 4);
        let expected = payload.clone();
        let send = tokio::spawn(async move {
            write_frame_vectored(&mut writer, &payload).await.unwrap();
        });

        let decoded = read_frame_with_limit(&mut reader, expected.len())
            .await
            .unwrap();
        send.await.unwrap();
        assert_eq!(decoded.as_ref(), expected.as_slice());
    }
}

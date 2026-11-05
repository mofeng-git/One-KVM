use rtsp_types as rtsp;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::error::{AppError, Result};

use super::types::RtspRequest;

async fn serialize_and_write<W: AsyncWrite + Unpin>(
    stream: &mut W,
    response: rtsp::Response<Vec<u8>>,
) -> Result<()> {
    let mut data = Vec::new();
    response
        .write(&mut data)
        .map_err(|e| AppError::BadRequest(format!("failed to serialize RTSP response: {}", e)))?;
    stream.write_all(&data).await?;
    Ok(())
}

pub(crate) async fn send_simple_response<W: AsyncWrite + Unpin>(
    stream: &mut W,
    code: u16,
    _reason: &str,
    cseq: Option<&str>,
    body: &str,
) -> Result<()> {
    let mut builder = rtsp::Response::builder(rtsp::Version::V1_0, status_code_from_u16(code));
    if let Some(cseq) = cseq {
        builder = builder.header(rtsp::headers::CSEQ, cseq);
    }

    let response = builder.build(body.as_bytes().to_vec());
    serialize_and_write(stream, response).await
}

pub(crate) async fn send_response<W: AsyncWrite + Unpin>(
    stream: &mut W,
    req: &RtspRequest,
    code: u16,
    _reason: &str,
    extra_headers: Vec<(String, String)>,
    body: &str,
    session_id: &str,
) -> Result<()> {
    let cseq = req
        .headers
        .get("cseq")
        .cloned()
        .unwrap_or_else(|| "1".to_string());

    let mut builder = rtsp::Response::builder(req.version, status_code_from_u16(code))
        .header(rtsp::headers::CSEQ, cseq.as_str());

    if !session_id.is_empty() {
        builder = builder.header(rtsp::headers::SESSION, session_id);
    }

    for (name, value) in extra_headers {
        let header_name = rtsp::HeaderName::try_from(name.as_str()).map_err(|e| {
            AppError::BadRequest(format!("invalid RTSP header name {}: {}", name, e))
        })?;
        builder = builder.header(header_name, value);
    }

    let response = builder.build(body.as_bytes().to_vec());
    serialize_and_write(stream, response).await
}

pub(crate) fn status_code_from_u16(code: u16) -> rtsp::StatusCode {
    match code {
        200 => rtsp::StatusCode::Ok,
        400 => rtsp::StatusCode::BadRequest,
        401 => rtsp::StatusCode::Unauthorized,
        404 => rtsp::StatusCode::NotFound,
        405 => rtsp::StatusCode::MethodNotAllowed,
        453 => rtsp::StatusCode::NotEnoughBandwidth,
        455 => rtsp::StatusCode::MethodNotValidInThisState,
        461 => rtsp::StatusCode::UnsupportedTransport,
        _ => rtsp::StatusCode::InternalServerError,
    }
}

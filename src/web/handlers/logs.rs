//! Runtime log reporting to the project feedback endpoint.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::{http::header::CONTENT_TYPE, Json};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::{AppError, Result};
use crate::logging;

/// Feedback endpoint that accepts a problem description plus a log attachment.
pub const REPORT_ENDPOINT: &str =
    "http://api.mrcong.cn:300/apps/one-kvm/report.php?app=one-kvm";
const REPORT_DESCRIPTION_FIELD: &str = "description";
const REPORT_LOG_FIELD: &str = "log";
const REPORT_LOG_FILENAME: &str = "one-kvm.log";
const REPORT_LOG_LINES: usize = 1000;
const MAX_DESCRIPTION_CHARS: usize = 2000;
const MIN_REPORT_INTERVAL: Duration = Duration::from_secs(10);
const REPORT_TIMEOUT: Duration = Duration::from_secs(30);

static LAST_REPORT: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Debug, Deserialize)]
pub struct LogReportRequest {
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct LogReportResponse {
    pub success: bool,
    pub status: u16,
}

/// Submit the recent runtime log together with a user-supplied description.
pub async fn report_runtime_logs(
    Json(request): Json<LogReportRequest>,
) -> Result<Json<LogReportResponse>> {
    let description = normalize_description(&request.description)?;

    if !allow_report(Instant::now()) {
        return Err(AppError::RateLimited(
            "a log report was submitted recently, please try again later".to_string(),
        ));
    }

    let lines = logging::hub().snapshot(REPORT_LOG_LINES);
    if lines.is_empty() {
        return Err(AppError::ServiceUnavailable(
            "no runtime logs are available yet".to_string(),
        ));
    }

    let log_content = format!("{}\n", lines.join("\n"));
    let boundary = format!("----OneKvmBoundary{}", uuid::Uuid::new_v4().simple());
    let body = build_multipart_body(&boundary, &description, &log_content);

    let client = reqwest::Client::builder()
        .timeout(REPORT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("one-kvm/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| {
            AppError::Internal(format!("failed to create the report client: {error}"))
        })?;

    let response = client
        .post(REPORT_ENDPOINT)
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .map_err(|error| {
            AppError::ServiceUnavailable(format!("failed to submit the log report: {error}"))
        })?;

    let status = response.status();
    // The endpoint redirects after processing, so treat redirects as accepted.
    if !(status.is_success() || status.is_redirection()) {
        return Err(AppError::ServiceUnavailable(format!(
            "the report endpoint returned HTTP {}",
            status.as_u16()
        )));
    }

    info!(
        lines = lines.len(),
        status = status.as_u16(),
        "Runtime log report submitted"
    );

    Ok(Json(LogReportResponse {
        success: true,
        status: status.as_u16(),
    }))
}

fn normalize_description(raw: &str) -> Result<String> {
    let description = raw.trim();
    if description.is_empty() {
        return Err(AppError::BadRequest(
            "a problem description is required".to_string(),
        ));
    }
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(AppError::BadRequest(format!(
            "the problem description must not exceed {MAX_DESCRIPTION_CHARS} characters"
        )));
    }
    Ok(description.to_string())
}

fn allow_report(now: Instant) -> bool {
    let mut last = LAST_REPORT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    allow_report_at(&mut last, now)
}

fn allow_report_at(last: &mut Option<Instant>, now: Instant) -> bool {
    match *last {
        Some(previous) if now.duration_since(previous) < MIN_REPORT_INTERVAL => false,
        _ => {
            *last = Some(now);
            true
        }
    }
}

fn build_multipart_body(boundary: &str, description: &str, log: &str) -> Vec<u8> {
    let mut body = String::with_capacity(description.len() + log.len() + 512);
    body.push_str(&format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"{REPORT_DESCRIPTION_FIELD}\"\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n\
         {description}\r\n"
    ));
    body.push_str(&format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"{REPORT_LOG_FIELD}\"; filename=\"{REPORT_LOG_FILENAME}\"\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n\
         {log}\r\n"
    ));
    body.push_str(&format!("--{boundary}--\r\n"));
    body.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_is_required_and_trimmed() {
        assert!(normalize_description("   ").is_err());
        assert_eq!(normalize_description("  broken video ").unwrap(), "broken video");
    }

    #[test]
    fn description_length_is_bounded() {
        let too_long = "x".repeat(MAX_DESCRIPTION_CHARS + 1);
        assert!(normalize_description(&too_long).is_err());
        let at_limit = "x".repeat(MAX_DESCRIPTION_CHARS);
        assert!(normalize_description(&at_limit).is_ok());
    }

    #[test]
    fn multipart_body_contains_description_and_log_attachment() {
        let body = build_multipart_body("BOUNDARY", "no signal", "line one\nline two\n");
        let text = String::from_utf8(body).unwrap();

        assert!(text.starts_with("--BOUNDARY\r\n"));
        assert!(text.contains("name=\"description\""));
        assert!(text.contains("no signal"));
        assert!(text.contains("name=\"log\"; filename=\"one-kvm.log\""));
        assert!(text.contains("line one\nline two"));
        assert!(text.ends_with("--BOUNDARY--\r\n"));
    }

    #[test]
    fn report_cooldown_rejects_immediate_repeats() {
        let start = Instant::now();
        let mut last = None;
        assert!(allow_report_at(&mut last, start));
        assert!(!allow_report_at(&mut last, start + Duration::from_secs(1)));
        assert!(allow_report_at(&mut last, start + MIN_REPORT_INTERVAL));
    }
}

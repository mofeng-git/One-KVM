//! Host power controls are separate from restarting the One-KVM process.
use super::*;

#[derive(Deserialize)]
pub struct HostPowerRequest {
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Clone, Copy)]
enum HostPowerAction {
    Reboot,
    Poweroff,
}

impl HostPowerAction {
    fn command(self) -> &'static str {
        match self {
            Self::Reboot => "reboot",
            Self::Poweroff => "poweroff",
        }
    }
}

fn confirmed_request(
    request: HostPowerRequest,
    action: HostPowerAction,
    execute: impl FnOnce(HostPowerAction) -> Result<()>,
) -> Result<Json<LoginResponse>> {
    if !request.confirmed {
        return Err(AppError::BadRequest(
            "Explicit confirmation is required for host power control.".into(),
        ));
    }
    execute(action)?;
    Ok(Json(LoginResponse {
        success: true,
        message: Some("Host power request accepted.".into()),
    }))
}

#[cfg(target_os = "linux")]
fn schedule_action(action: HostPowerAction) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    static PENDING: AtomicBool = AtomicBool::new(false);

    // These are system commands; do not run them through a user-controlled shell.
    if unsafe { libc::geteuid() } != 0 {
        return Err(AppError::BadRequest(
            "Host power control requires the One-KVM service to run as root.".into(),
        ));
    }
    let command = action.command();
    let executable = ["/sbin", "/usr/sbin", "/bin", "/usr/bin"]
        .into_iter()
        .map(|directory| std::path::Path::new(directory).join(command))
        .find(|path| {
            std::fs::metadata(path)
                .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        })
        .ok_or_else(|| AppError::NotFound(format!("Host command {command} is unavailable.")))?;
    if PENDING.swap(true, Ordering::AcqRel) {
        return Err(AppError::Conflict(
            "A host power request is already pending.".into(),
        ));
    }
    tokio::spawn(async move {
        // Let the accepted response reach the browser before the connection ends.
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        let result = tokio::process::Command::new(executable)
            .stdin(std::process::Stdio::null())
            .output()
            .await;
        match result {
            Ok(output) if output.status.success() => info!("Host {command} command accepted"),
            Ok(output) => {
                PENDING.store(false, Ordering::Release);
                warn!(
                    "Host {command} failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(error) => {
                PENDING.store(false, Ordering::Release);
                warn!("Could not execute host {command}: {error}");
            }
        }
    });
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn schedule_action(_action: HostPowerAction) -> Result<()> {
    Err(AppError::BadRequest(
        "Host power control is only available on Linux.".into(),
    ))
}

pub async fn host_reboot(Json(request): Json<HostPowerRequest>) -> Result<Json<LoginResponse>> {
    confirmed_request(request, HostPowerAction::Reboot, schedule_action)
}

pub async fn host_poweroff(Json(request): Json<HostPowerRequest>) -> Result<Json<LoginResponse>> {
    confirmed_request(request, HostPowerAction::Poweroff, schedule_action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_false_confirmation_cannot_reach_the_executor() {
        for body in ["{}", r#"{"confirmed":false}"#] {
            let request = serde_json::from_str(body).unwrap();
            let result = confirmed_request(request, HostPowerAction::Poweroff, |_| {
                panic!("Unconfirmed request must never execute a system command")
            });
            assert!(matches!(result, Err(AppError::BadRequest(_))));
        }
    }

    #[test]
    fn confirmation_dispatches_only_the_selected_host_command() {
        for (action, expected) in [
            (HostPowerAction::Reboot, "reboot"),
            (HostPowerAction::Poweroff, "poweroff"),
        ] {
            let mut dispatched = None;
            let Json(response) =
                confirmed_request(HostPowerRequest { confirmed: true }, action, |a| {
                    dispatched = Some(a.command());
                    Ok(())
                })
                .unwrap();
            assert!(response.success);
            assert_eq!(dispatched, Some(expected));
        }
    }

    #[test]
    fn executor_errors_are_not_reported_as_accepted() {
        let result = confirmed_request(
            HostPowerRequest { confirmed: true },
            HostPowerAction::Reboot,
            |_| Err(AppError::Conflict("pending".into())),
        );
        assert!(matches!(result, Err(AppError::Conflict(_))));
    }
}

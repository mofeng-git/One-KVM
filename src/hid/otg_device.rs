#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::unix::io::AsFd;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
#[cfg(unix)]
use tracing::trace;

#[cfg(unix)]
pub struct OtgDeviceIo;

#[cfg(unix)]
impl OtgDeviceIo {
    pub fn write_with_timeout(
        file: &mut File,
        data: &[u8],
        timeout_ms: i32,
    ) -> std::io::Result<bool> {
        write_with_wait(file, data, timeout_ms as u16, |file, timeout| {
            let mut pollfd = [PollFd::new(file.as_fd(), PollFlags::POLLOUT)];
            match poll(&mut pollfd, PollTimeout::from(timeout)) {
                Ok(1) => {
                    if let Some(revents) = pollfd[0].revents() {
                        if revents.contains(PollFlags::POLLERR)
                            || revents.contains(PollFlags::POLLHUP)
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::BrokenPipe,
                                "Device error or hangup",
                            ));
                        }
                    }
                    Ok(true)
                }
                Ok(0) => {
                    trace!("HID write timeout, dropping data");
                    Ok(false)
                }
                Ok(_) => Ok(false),
                Err(e) => Err(std::io::Error::other(e)),
            }
        })
    }
}

#[cfg(unix)]
fn write_with_wait<Writer: Write>(
    writer: &mut Writer,
    mut data: &[u8],
    timeout_ms: u16,
    mut wait: impl FnMut(&Writer, u16) -> std::io::Result<bool>,
) -> std::io::Result<bool> {
    let mut deadline = None;
    while !data.is_empty() {
        match writer.write(data) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(written) => data = &data[written..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let deadline = deadline.get_or_insert_with(|| {
                    Instant::now() + Duration::from_millis(u64::from(timeout_ms))
                });
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero()
                    || !wait(
                        writer,
                        remaining
                            .as_nanos()
                            .div_ceil(1_000_000)
                            .min(u16::MAX as u128) as u16,
                    )?
                {
                    return Ok(false);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::{ErrorKind, Result};

    struct ScriptedWriter {
        steps: VecDeque<Result<usize>>,
        data: Vec<u8>,
    }

    impl Write for ScriptedWriter {
        fn write(&mut self, input: &[u8]) -> Result<usize> {
            let count = self.steps.pop_front().unwrap_or(Ok(input.len()))?;
            self.data.extend_from_slice(&input[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ready_device_skips_poll_and_preserves_report_bytes() {
        let mut writer = ScriptedWriter {
            steps: VecDeque::new(),
            data: Vec::new(),
        };
        assert!(
            write_with_wait(&mut writer, &[1, 2, 3, 4], 100, |_, _| panic!(
                "unexpected poll"
            ))
            .unwrap()
        );
        assert_eq!(writer.data, [1, 2, 3, 4]);
    }

    #[test]
    fn partial_and_interrupted_writes_do_not_duplicate_bytes() {
        let mut writer = ScriptedWriter {
            steps: [
                Ok(2),
                Err(ErrorKind::Interrupted.into()),
                Err(ErrorKind::WouldBlock.into()),
            ]
            .into(),
            data: Vec::new(),
        };
        let mut waits = 0;
        assert!(write_with_wait(&mut writer, &[1, 2, 3, 4], 100, |_, _| {
            waits += 1;
            Ok(true)
        })
        .unwrap());
        assert_eq!(waits, 1);
        assert_eq!(writer.data, [1, 2, 3, 4]);
    }

    #[test]
    fn blocked_device_retains_bounded_timeout_and_error_handling() {
        let mut writer = ScriptedWriter {
            steps: [Err(ErrorKind::WouldBlock.into())].into(),
            data: Vec::new(),
        };
        assert!(!write_with_wait(&mut writer, &[1], 100, |_, _| Ok(false)).unwrap());
        assert!(writer.data.is_empty());
        writer.steps.push_back(Err(ErrorKind::BrokenPipe.into()));
        assert_eq!(
            write_with_wait(&mut writer, &[1], 100, |_, _| Ok(true))
                .unwrap_err()
                .kind(),
            ErrorKind::BrokenPipe
        );
    }
}

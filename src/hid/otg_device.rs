#[cfg(unix)]
use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::io::AsFd;
#[cfg(unix)]
use std::path::PathBuf;

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
        let mut pollfd = [PollFd::new(file.as_fd(), PollFlags::POLLOUT)];
        match poll(&mut pollfd, PollTimeout::from(timeout_ms as u16)) {
            Ok(1) => {
                if let Some(revents) = pollfd[0].revents() {
                    if revents.contains(PollFlags::POLLERR) || revents.contains(PollFlags::POLLHUP)
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Device error or hangup",
                        ));
                    }
                }
                file.write_all(data)?;
                Ok(true)
            }
            Ok(0) => {
                trace!("HID write timeout, dropping data");
                Ok(false)
            }
            Ok(_) => Ok(false),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }
}

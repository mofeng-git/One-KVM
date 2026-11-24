//! Lifecycle for an upstream executable bundled inside a plugin executable.
use std::{io::Write, path::Path, process::Stdio};

use anyhow::{bail, Result};
use serde_json::json;
use tokio::{
    process::{Child, Command},
    sync::Mutex,
};

use crate::{runtime::Service, Status};

pub struct EmbeddedProcess {
    child: Mutex<Child>,
    #[cfg(windows)]
    _job: Job,
    _runtime: tempfile::TempDir,
}

impl EmbeddedProcess {
    pub fn launch(
        data: &Path,
        executable: &[u8],
        configure: impl FnOnce(&mut Command, &Path) -> Result<()>,
    ) -> Result<Self> {
        if executable.is_empty() {
            bail!("This plugin was built without its upstream executable");
        }

        let runtime = tempfile::tempdir_in(data)?;
        let path = runtime
            .path()
            .join(format!("client{}", std::env::consts::EXE_SUFFIX));
        let mut file = std::fs::File::create_new(&path)?;
        file.write_all(executable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o700))?;
        }
        drop(file);

        let mut command = Command::new(path);
        command.stdin(Stdio::null()).kill_on_drop(true);
        for key in [
            "ONE_KVM_TOKEN",
            "ONE_KVM_IPC",
            "ONE_KVM_CONFIG",
            "ONE_KVM_DATA",
        ] {
            command.env_remove(key);
        }
        configure(&mut command, runtime.path())?;

        #[cfg(target_os = "linux")]
        {
            let parent = std::process::id() as libc::pid_t;
            // Terminate the embedded program if its adapter exits unexpectedly.
            unsafe {
                command.pre_exec(move || {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
                    }
                    Ok(())
                });
            }
        }

        #[cfg(windows)]
        let job = Job::new()?;
        let child = command.spawn()?;
        #[cfg(windows)]
        job.assign(&child)?;
        Ok(Self {
            child: Mutex::new(child),
            #[cfg(windows)]
            _job: job,
            _runtime: runtime,
        })
    }

    pub fn write_config(directory: &Path, name: &str, source: &str) -> Result<std::path::PathBuf> {
        let path = directory.join(name);
        let mut file = std::fs::OpenOptions::new();
        file.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            file.mode(0o600);
        }
        file.open(&path)?.write_all(source.as_bytes())?;
        Ok(path)
    }
}

impl Service for EmbeddedProcess {
    async fn status(&self) -> Result<Status> {
        let mut child = self.child.lock().await;
        if let Some(exit) = child.try_wait()? {
            bail!("Upstream client exited with {exit}");
        }
        Ok(Status {
            fields: [("pid".into(), json!(child.id()))].into_iter().collect(),
            error: None,
        })
    }

    async fn shutdown(&self) {
        let _ = self.child.lock().await.kill().await;
    }
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for Job {}
#[cfg(windows)]
unsafe impl Sync for Job {}

#[cfg(windows)]
impl Job {
    fn new() -> std::io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let job = Self(handle);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(job)
    }

    fn assign(&self, child: &Child) -> std::io::Result<()> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        let handle = child
            .raw_handle()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
        if unsafe { AssignProcessToJobObject(self.0, handle) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::StreamExt;
use one_kvm_plugin_sdk::{Manifest, Status, MAX_MANIFEST};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::{
    fs,
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    capabilities::{Capabilities, Session},
    package::{self, valid_id, write_json, EXECUTABLE},
};
use crate::{state::AppState, AppError, Result};

#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
struct Settings {
    autostart: bool,
    config: Map<String, Value>,
}

#[derive(Clone, Default, Serialize)]
struct Runtime {
    phase: &'static str,
    pid: Option<u32>,
    #[serde(flatten)]
    status: Status,
}

#[derive(Serialize)]
pub struct PluginView {
    manifest: Manifest,
    enabled: bool,
    autostart: bool,
    config: Map<String, Value>,
    runtime: Runtime,
}

struct Installed {
    manifest: Manifest,
    generation: Uuid,
    settings: Settings,
    enabled: bool,
    offline_status: Map<String, Value>,
    runtime: Arc<RwLock<Runtime>>,
    logs: Arc<Mutex<VecDeque<String>>>,
    worker: Option<Worker>,
}

struct Worker {
    cancel: CancellationToken,
    task: JoinHandle<()>,
    session: Arc<RwLock<Option<Arc<Session>>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

pub struct PluginManager {
    root: PathBuf,
    plugins: Mutex<BTreeMap<String, Installed>>,
    capabilities: Arc<Capabilities>,
    http: reqwest::Client,
    server: Mutex<Option<(String, JoinHandle<()>)>>,
}

impl PluginManager {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            plugins: Mutex::new(BTreeMap::new()),
            capabilities: Arc::new(Capabilities::default()),
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .expect("initialize plugin control HTTP client"),
            server: Mutex::new(None),
        }
    }

    pub async fn start(&self, app: &Arc<AppState>) -> Result<()> {
        let url = self.capabilities.serve(Arc::downgrade(app)).await?;
        *self.server.lock().await = Some(url);
        self.prepare_storage().await?;

        let mut plugins = self.plugins.lock().await;
        let mut directories = fs::read_dir(&self.root).await?;
        while let Some(dir) = directories.next_entry().await? {
            let name = dir.file_name();
            if name
                .to_str()
                .is_some_and(|name| name.starts_with(".removed-"))
                && dir.file_type().await?.is_dir()
            {
                if let Err(error) = fs::remove_dir_all(dir.path()).await {
                    tracing::warn!(path = %dir.path().display(), %error, "Could not finish removing plugin files");
                }
                continue;
            }
            let Some(id) = name.to_str().filter(|id| valid_id(id)) else {
                continue;
            };
            if !dir.file_type().await?.is_dir() {
                continue;
            }
            match load_plugin(&dir.path(), id).await {
                Ok(mut plugin) => {
                    self.start_worker(&mut plugin).await;
                    plugins.insert(id.into(), plugin);
                }
                Err(error) => {
                    tracing::warn!(plugin = id, %error, "Could not load installed plugin")
                }
            }
        }
        Ok(())
    }

    async fn prepare_storage(&self) -> Result<()> {
        fs::create_dir_all(&self.root).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700)).await?;
        }

        Ok(())
    }

    /// Provision a local package before the host service starts.
    pub async fn provision(&self, package: PathBuf) -> Result<String> {
        self.prepare_storage().await?;
        let plugin = self.install(package).await?;
        Ok(plugin.manifest.id)
    }

    pub async fn shutdown(&self) {
        let mut plugins = self.plugins.lock().await;
        futures::future::join_all(plugins.values_mut().map(stop_worker)).await;
        if let Some((_, task)) = self.server.lock().await.take() {
            task.abort();
        }
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) async fn list(&self) -> Vec<PluginView> {
        let plugins = self.plugins.lock().await;
        let mut views = Vec::with_capacity(plugins.len());
        for plugin in plugins.values() {
            views.push(view(plugin).await);
        }
        views
    }

    pub(super) async fn install(&self, source: PathBuf) -> Result<PluginView> {
        let stage = tempfile::tempdir_in(&self.root)?;
        let manifest = package::unpack(&source, stage.path()).await?;
        let generation = Uuid::new_v4();
        let mut plugins = self.plugins.lock().await;
        let directory = self.root.join(&manifest.id);
        let active = directory.join("active.json");
        let versions = directory.join("versions");
        let destination = versions.join(generation.to_string());
        fs::create_dir_all(directory.join("data")).await?;
        fs::create_dir_all(&versions).await?;
        let settings_path = directory.join("settings.json");
        let settings = match plugins.get(&manifest.id) {
            Some(previous) => previous.settings.clone(),
            None => match fs::read(&settings_path).await {
                Ok(bytes) => serde_json::from_slice(&bytes)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let settings = Settings {
                        autostart: manifest.default_enabled,
                        config: manifest
                            .config_schema
                            .iter()
                            .map(|field| (field.key.clone(), field.default.clone()))
                            .collect(),
                    };
                    write_json(&settings_path, &settings).await?;
                    settings
                }
                Err(error) => return Err(error.into()),
            },
        };

        fs::rename(stage.path(), &destination).await?;
        if let Err(error) = write_json(&active, &generation).await {
            let _ = fs::remove_dir_all(&destination).await;
            return Err(error);
        }
        let id = manifest.id.clone();
        let enabled = plugins
            .get(&id)
            .map_or(settings.autostart, |previous| previous.enabled);
        let mut plugin = Installed {
            manifest,
            generation,
            settings,
            enabled,
            offline_status: Map::new(),
            runtime: Arc::default(),
            logs: Arc::default(),
            worker: None,
        };
        if let Some(mut old) = plugins.remove(&id) {
            stop_worker(&mut old).await;
        }
        prune_versions(&versions, generation).await;
        self.start_worker(&mut plugin).await;
        let result = view(&plugin).await;
        plugins.insert(id, plugin);
        Ok(result)
    }

    pub(super) async fn uninstall(&self, id: &str) -> Result<()> {
        let mut plugins = self.plugins.lock().await;
        let plugin = plugins.get_mut(id).ok_or_else(not_found)?;
        stop_worker(plugin).await;
        // Rename first so a failed deletion can't reload a partially deleted plugin.
        let removed = self.root.join(format!(".removed-{}", Uuid::new_v4()));
        if let Err(error) = fs::rename(self.root.join(id), &removed).await {
            self.start_worker(plugin).await;
            return Err(error.into());
        }
        plugins.remove(id);
        fs::remove_dir_all(removed).await?;
        Ok(())
    }

    pub(super) async fn configure(
        &self,
        id: &str,
        enabled: Option<bool>,
        autostart: Option<bool>,
        config: Option<Map<String, Value>>,
    ) -> Result<PluginView> {
        let mut plugins = self.plugins.lock().await;
        let plugin = plugins.get_mut(id).ok_or_else(not_found)?;
        let mut settings = plugin.settings.clone();
        if let Some(autostart) = autostart {
            settings.autostart = autostart;
        }
        if let Some(config) = config {
            for (key, value) in config {
                let field = plugin
                    .manifest
                    .config_schema
                    .iter()
                    .find(|field| field.key == key)
                    .ok_or_else(|| {
                        AppError::BadRequest(format!("Unknown configuration field: {key}"))
                    })?;
                if !field.accepts(&value) {
                    return Err(AppError::BadRequest(format!("Invalid value for {key}")));
                }
                settings.config.insert(key, value);
            }
        }
        let enabled = enabled.unwrap_or(plugin.enabled);
        let restart = enabled != plugin.enabled || settings.config != plugin.settings.config;
        if settings != plugin.settings {
            write_json(&self.root.join(id).join("settings.json"), &settings).await?;
            plugin.settings = settings;
        }
        if restart {
            plugin.enabled = enabled;
            stop_worker(plugin).await;
            self.start_worker(plugin).await;
        }
        Ok(view(plugin).await)
    }

    pub(super) async fn logs(&self, id: &str) -> Result<Vec<String>> {
        let logs = self
            .plugins
            .lock()
            .await
            .get(id)
            .ok_or_else(not_found)?
            .logs
            .clone();
        let result = logs.lock().await.iter().cloned().collect();
        Ok(result)
    }

    pub(super) async fn action(&self, id: &str, action: &str) -> Result<Value> {
        let session = {
            let plugins = self.plugins.lock().await;
            let plugin = plugins.get(id).ok_or_else(not_found)?;
            if !plugin.manifest.actions.iter().any(|item| item.id == action) {
                return Err(AppError::NotFound("Unknown plugin action".into()));
            }
            let worker = plugin.worker.as_ref().ok_or_else(unavailable)?;
            let session = worker
                .session
                .read()
                .await
                .clone()
                .ok_or_else(unavailable)?;
            session
        };
        let port = session.port().ok_or_else(unavailable)?;
        let response = self
            .http
            .post(format!("http://127.0.0.1:{port}/actions/{action}"))
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(control_error)?;
        let status = response.status();
        let value: Value = read_response(response).await?;
        if !status.is_success() {
            return Err(AppError::BadRequest(
                value["error"]
                    .as_str()
                    .unwrap_or("Plugin action failed")
                    .into(),
            ));
        }
        Ok(value)
    }

    pub(super) async fn open_file(&self, id: &str, name: &str) -> Result<fs::File> {
        let plugins = self.plugins.lock().await;
        let plugin = plugins.get(id).ok_or_else(not_found)?;
        Ok(fs::File::open(self.file_path(plugin, name).await?).await?)
    }

    pub(super) async fn remove_file(&self, id: &str, name: &str) -> Result<()> {
        let plugins = self.plugins.lock().await;
        let plugin = plugins.get(id).ok_or_else(not_found)?;
        fs::remove_file(self.file_path(plugin, name).await?).await?;
        Ok(())
    }

    async fn file_path(&self, plugin: &Installed, name: &str) -> Result<PathBuf> {
        let missing = || AppError::NotFound("Plugin file not found".into());
        if !plugin.manifest.files || !super::capabilities::valid_filename(name) {
            return Err(missing());
        }
        let base = self
            .root
            .join(&plugin.manifest.id)
            .join("data")
            .join("files");
        let path = base.join(name);
        if !fs::symlink_metadata(&base)
            .await
            .is_ok_and(|metadata| metadata.is_dir())
            || !fs::symlink_metadata(&path)
                .await
                .is_ok_and(|metadata| metadata.is_file())
        {
            return Err(missing());
        }
        Ok(path)
    }

    async fn start_worker(&self, plugin: &mut Installed) {
        *plugin.runtime.write().await = Runtime {
            phase: "stopped",
            ..Runtime::default()
        };
        let working = self.root.join(&plugin.manifest.id);
        let binary = working
            .join("versions")
            .join(plugin.generation.to_string())
            .join(EXECUTABLE);
        let config = effective_config(plugin);
        if plugin
            .manifest
            .status_fields
            .iter()
            .any(|field| field.available_offline)
        {
            match inspect(&working, &binary, &config).await {
                Ok(status) => plugin.offline_status = status.fields,
                Err(error) => {
                    tracing::warn!(plugin = plugin.manifest.id, %error, "Plugin status inspection failed")
                }
            }
        }
        if !plugin.enabled {
            return;
        }
        let Some((url, _)) = self
            .server
            .lock()
            .await
            .as_ref()
            .map(|(url, task)| (url.clone(), task.is_finished()))
            .filter(|(_, closed)| !closed)
        else {
            plugin.runtime.write().await.status.error =
                Some("Plugin capability server is unavailable".into());
            return;
        };
        let launch = Launch {
            id: plugin.manifest.id.clone(),
            url,
            dir: working,
            binary,
            config,
            capabilities: plugin.manifest.capabilities.clone(),
        };
        let cancel = CancellationToken::new();
        let session = Arc::new(RwLock::new(None));
        let supervisor = Supervisor {
            runtime: plugin.runtime.clone(),
            logs: plugin.logs.clone(),
            cancel: cancel.clone(),
            session: session.clone(),
            api: self.capabilities.clone(),
            http: self.http.clone(),
        };
        plugin.runtime.write().await.phase = "starting";
        let task = tokio::spawn(supervisor.run(launch));
        plugin.worker = Some(Worker {
            cancel,
            task,
            session,
        });
    }
}

async fn inspect(
    working: &Path,
    binary: &Path,
    config: &Map<String, Value>,
) -> anyhow::Result<Status> {
    let working = fs::canonicalize(working).await?;
    let data = working.join("data");
    let config_path = working.join("runtime-config.json");
    write_json(&config_path, config).await?;
    let mut child = Command::new(fs::canonicalize(binary).await?)
        .arg(one_kvm_plugin_sdk::INSPECT_ARG)
        .current_dir(&data)
        .env("ONE_KVM_CONFIG", config_path)
        .env("ONE_KVM_DATA", data)
        .env_remove("ONE_KVM_IPC")
        .env_remove("ONE_KVM_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child.stdout.take().expect("inspection stdout is piped");
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut output = Vec::new();
        stdout
            .take((MAX_MANIFEST + 1) as u64)
            .read_to_end(&mut output)
            .await?;
        anyhow::ensure!(
            output.len() <= MAX_MANIFEST,
            "Inspection output is too large"
        );
        anyhow::ensure!(child.wait().await?.success(), "Inspection command failed");
        Ok(serde_json::from_slice(&output)?)
    })
    .await?
}

async fn stop_worker(plugin: &mut Installed) {
    if let Some(mut worker) = plugin.worker.take() {
        worker.cancel.cancel();
        if let Err(error) = (&mut worker.task).await {
            tracing::warn!(plugin = plugin.manifest.id, %error, "Plugin supervisor ended unexpectedly");
        }
    }
}

async fn load_plugin(directory: &Path, id: &str) -> Result<Installed> {
    let generation: Uuid = serde_json::from_slice(&fs::read(directory.join("active.json")).await?)?;
    let versions = directory.join("versions");
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(versions.join(generation.to_string()).join("manifest.json")).await?,
    )?;
    package::validate(&manifest)?;
    if manifest.id != id {
        return Err(AppError::BadRequest(
            "Plugin ID does not match install directory".into(),
        ));
    }
    let settings: Settings =
        serde_json::from_slice(&fs::read(directory.join("settings.json")).await?)?;
    prune_versions(&versions, generation).await;
    Ok(Installed {
        manifest,
        generation,
        enabled: settings.autostart,
        settings,
        offline_status: Map::new(),
        runtime: Arc::default(),
        logs: Arc::default(),
        worker: None,
    })
}

async fn prune_versions(directory: &Path, active: Uuid) {
    if let Ok(mut entries) = fs::read_dir(directory).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            if name
                .to_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .is_some_and(|id| id != active)
            {
                if let Err(error) = fs::remove_dir_all(entry.path()).await {
                    tracing::warn!(path = %entry.path().display(), %error, "Could not remove old plugin version");
                }
            }
        }
    }
}

fn effective_config(plugin: &Installed) -> Map<String, Value> {
    plugin
        .manifest
        .config_schema
        .iter()
        .map(|field| {
            let saved = plugin
                .settings
                .config
                .get(&field.key)
                .filter(|value| field.accepts(value));
            (field.key.clone(), saved.unwrap_or(&field.default).clone())
        })
        .collect()
}

async fn view(plugin: &Installed) -> PluginView {
    let mut runtime = plugin.runtime.read().await.clone();
    runtime.status.fields.retain(|key, value| {
        (plugin.manifest.files && key == "files" && value.is_array())
            || plugin
                .manifest
                .status_fields
                .iter()
                .any(|field| field.key == *key)
    });
    for field in plugin
        .manifest
        .status_fields
        .iter()
        .filter(|field| field.available_offline)
    {
        if let Some(value) = plugin.offline_status.get(&field.key) {
            runtime
                .status
                .fields
                .entry(field.key.clone())
                .or_insert_with(|| value.clone());
        }
    }
    PluginView {
        manifest: plugin.manifest.clone(),
        enabled: plugin.enabled,
        autostart: plugin.settings.autostart,
        config: effective_config(plugin),
        runtime,
    }
}

struct Launch {
    id: String,
    url: String,
    dir: PathBuf,
    binary: PathBuf,
    config: Map<String, Value>,
    capabilities: Vec<one_kvm_plugin_sdk::Capability>,
}

struct Supervisor {
    runtime: Arc<RwLock<Runtime>>,
    logs: Arc<Mutex<VecDeque<String>>>,
    cancel: CancellationToken,
    session: Arc<RwLock<Option<Arc<Session>>>>,
    api: Arc<Capabilities>,
    http: reqwest::Client,
}

impl Supervisor {
    async fn run(self, launch: Launch) {
        let mut delay = Duration::from_secs(1);
        while !self.cancel.is_cancelled() {
            let start = Instant::now();
            *self.runtime.write().await = Runtime {
                phase: "starting",
                ..Runtime::default()
            };
            let session = self.api.register(&launch.id, &launch.capabilities).await;
            *self.session.write().await = Some(session.clone());
            let result = self.spawn(&launch, &session).await;
            self.api.revoke(&session).await;
            *self.session.write().await = None;
            if self.cancel.is_cancelled() {
                break;
            }
            if start.elapsed() >= Duration::from_secs(60) {
                delay = Duration::from_secs(1);
            }
            let error = result
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "Plugin exited".into());
            tracing::warn!(plugin = launch.id, %error, ?delay, "Plugin will restart");
            *self.runtime.write().await = Runtime {
                phase: "backoff",
                status: Status {
                    error: Some(error),
                    ..Status::default()
                },
                ..Runtime::default()
            };
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                _ = tokio::time::sleep(delay) => {}
            }
            delay = (delay * 2).min(Duration::from_secs(60));
        }
        *self.runtime.write().await = Runtime {
            phase: "stopped",
            ..Runtime::default()
        };
    }

    async fn spawn(&self, launch: &Launch, session: &Session) -> anyhow::Result<()> {
        let dir = fs::canonicalize(&launch.dir).await?;
        let data = dir.join("data");
        fs::create_dir_all(&data).await?;
        let config = dir.join("runtime-config.json");
        write_json(&config, &launch.config).await?;
        let mut command = Command::new(fs::canonicalize(&launch.binary).await?);
        command
            .current_dir(&data)
            .env("ONE_KVM_IPC", &launch.url)
            .env("ONE_KVM_TOKEN", &session.token)
            .env("ONE_KVM_CONFIG", &config)
            .env("ONE_KVM_DATA", &data)
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        // Tokio closes the child's owned stdin on wait(). Keep the lifetime pipe
        // open through startup and health monitoring, then ask the child to exit.
        let stdin = child.stdin.take();
        let stdout = tokio::spawn(collect_logs(
            child.stdout.take().unwrap(),
            self.logs.clone(),
            "stdout",
        ));
        let stderr = tokio::spawn(collect_logs(
            child.stderr.take().unwrap(),
            self.logs.clone(),
            "stderr",
        ));
        let result = self.monitor(&mut child, session).await;
        drop(stdin);
        if tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .is_err()
        {
            child.kill().await?;
        }
        for mut task in [stdout, stderr] {
            if tokio::time::timeout(Duration::from_millis(200), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
        result
    }

    async fn monitor(&self, child: &mut Child, session: &Session) -> anyhow::Result<()> {
        let pid = child.id();
        let port = tokio::select! {
            _ = self.cancel.cancelled() => return Ok(()),
            exit = child.wait() => anyhow::bail!("Plugin exited with {}", exit?),
            ready = tokio::time::timeout(Duration::from_secs(30), session.ready()) => ready?,
        };
        let mut timer = tokio::time::interval(Duration::from_secs(2));
        let mut failed = 0;
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                exit = child.wait() => anyhow::bail!("Plugin exited with {}", exit?),
                _ = timer.tick() => {}
            }
            let probe = self
                .http
                .get(format!("http://127.0.0.1:{port}/health"))
                .bearer_auth(&session.token)
                .send();
            let status = tokio::select! {
                _ = self.cancel.cancelled() => return Ok(()),
                response = probe => match response.and_then(reqwest::Response::error_for_status) {
                    Ok(response) => read_response::<Status>(response).await,
                    Err(error) => Err(control_error(error)),
                }
            };
            match status {
                Ok(status) if status.error.is_none() => {
                    failed = 0;
                    *self.runtime.write().await = Runtime {
                        phase: "running",
                        pid,
                        status,
                    };
                }
                result => {
                    failed += 1;
                    let error = match result {
                        Ok(status) => status.error.unwrap(),
                        Err(error) => error.to_string(),
                    };
                    self.runtime.write().await.status.error = Some(error.clone());
                    if failed >= 3 {
                        anyhow::bail!("{error}");
                    }
                }
            }
        }
    }
}

async fn collect_logs(
    mut input: impl AsyncRead + Unpin,
    logs: Arc<Mutex<VecDeque<String>>>,
    source: &str,
) {
    let mut bytes = [0; 2048];
    let mut line = Vec::new();
    while let Ok(size @ 1..) = input.read(&mut bytes).await {
        for &byte in &bytes[..size] {
            if byte == b'\n' || line.len() == 4096 {
                push_log(&logs, source, &line).await;
                line.clear();
            }
            if byte != b'\n' {
                line.push(byte);
            }
        }
    }
    if !line.is_empty() {
        push_log(&logs, source, &line).await;
    }
}

async fn push_log(logs: &Mutex<VecDeque<String>>, source: &str, line: &[u8]) {
    let mut logs = logs.lock().await;
    if logs.len() >= 200 {
        logs.pop_front();
    }
    logs.push_back(format!("[{source}] {}", String::from_utf8_lossy(line)));
}

async fn read_response<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let mut buffer = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(bytes) = stream.next().await {
        let bytes = bytes.map_err(control_error)?;
        if buffer.len() + bytes.len() > MAX_MANIFEST {
            return Err(AppError::ServiceUnavailable(
                "Plugin response exceeds the size limit".into(),
            ));
        }
        buffer.extend_from_slice(&bytes);
    }
    serde_json::from_slice(&buffer).map_err(control_error)
}

fn control_error(error: impl std::fmt::Display) -> AppError {
    AppError::ServiceUnavailable(format!("Plugin control failed: {error}"))
}

fn not_found() -> AppError {
    AppError::NotFound("Plugin not installed".into())
}
fn unavailable() -> AppError {
    AppError::ServiceUnavailable("Plugin is not running".into())
}

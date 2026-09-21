//! Classic Bluetooth HID peripheral: SDP + L2CAP. Linux/BlueZ only.
mod agent;
pub mod bonds;
mod controller;
mod protocol;

use bluer::{
    l2cap::{Security, SecurityLevel, SeqPacket, SeqPacketListener, Socket, SocketAddr},
    Adapter, Address, AddressType, Session,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch, Mutex as AsyncMutex};

#[derive(Debug, Clone)]
pub struct Config {
    pub adapter: String,
    pub name: String,
    pub peer: Option<String>,
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !self
            .adapter
            .strip_prefix("hci")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
        {
            return Err("Bluetooth adapter must be hci followed by an index".into());
        }
        if self.name.is_empty() || self.name.len() > 64 || self.name.chars().any(char::is_control) {
            return Err(
                "Bluetooth name must contain 1–64 UTF-8 bytes without control characters".into(),
            );
        }
        if let Some(peer) = &self.peer {
            peer.parse::<Address>()
                .map_err(|_| "Invalid Bluetooth peer address")?;
        }
        Ok(())
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub initialized: bool,
    pub connected: bool,
    pub ready: bool,
    pub adapter: String,
    pub adapter_address: String,
    pub peer: Option<String>,
    pub pairing_seconds: u32,
    pub control_connected: bool,
    pub interrupt_connected: bool,
    pub leds: u8,
    pub generation: u64,
    pub error: Option<String>,
    pub devices: Vec<Device>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub address: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}
#[derive(Debug, Clone, Copy)]
pub enum Report {
    Keyboard,
    Mouse,
    Consumer,
}
impl Report {
    fn len(self) -> usize {
        match self {
            Self::Keyboard => 8,
            Self::Mouse => 4,
            Self::Consumer => 2,
        }
    }
    fn id(self) -> u8 {
        match self {
            Self::Keyboard => 1,
            Self::Mouse => 2,
            Self::Consumer => 3,
        }
    }
}
struct State {
    peer: Option<String>,
    pairing_until: Option<Instant>,
    bonded_before_pairing: HashSet<String>,
}
impl State {
    fn observe_bond(&mut self, address: &str, paired: bool) {
        // Just Works may complete without an Agent1 authorization callback.
        // Adopt only a new bond from our explicit window, respecting a pinned peer.
        if paired
            && self.peer.is_none()
            && self
                .pairing_until
                .is_some_and(|until| until > Instant::now())
            && !self.bonded_before_pairing.contains(address)
        {
            self.peer = Some(address.to_owned());
            tracing::info!(
                peer = address,
                "Bluetooth HID selected newly bonded computer"
            );
        }
    }
    fn close_pairing(&mut self, paired: bool, fallback: Option<String>) {
        self.pairing_until = None;
        if !paired {
            self.peer = fallback;
        }
    }
    fn pairing_completed(&self, selected_paired: bool) -> bool {
        selected_paired
            && self
                .peer
                .as_ref()
                .is_some_and(|peer| !self.bonded_before_pairing.contains(peer))
    }
}
struct Shared {
    state: Mutex<State>,
}
impl Shared {
    fn new(peer: Option<String>) -> Self {
        Self {
            state: Mutex::new(State {
                peer,
                pairing_until: None,
                bonded_before_pairing: HashSet::new(),
            }),
        }
    }
    fn allowed(&self, address: Address) -> bool {
        self.state.lock().unwrap().peer.as_deref() == Some(address.to_string().as_str())
    }
}
pub enum Action {
    Pair(u32),
    ClosePairing,
    Forget,
    Disconnect,
    Reset,
}
enum Command {
    Report(Report, Vec<u8>),
    Action(Action),
    Stop,
}
struct Request {
    command: Command,
    result: oneshot::Sender<Result<(), String>>,
    expires: Instant,
}
pub struct Peripheral {
    tx: mpsc::Sender<Request>,
    status: watch::Receiver<Status>,
    task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
}
impl Peripheral {
    pub fn start(config: Config) -> Result<Self, String> {
        Self::start_with_store(config, None)
    }
    pub fn start_with_store(
        config: Config,
        bonds: Option<Arc<dyn bonds::BondStore>>,
    ) -> Result<Self, String> {
        config.validate()?;
        let (tx, rx) = mpsc::channel(64);
        let (state_tx, status) = watch::channel(Status {
            adapter: config.adapter.clone(),
            ..Default::default()
        });
        let task = tokio::spawn(supervise(config, bonds, rx, state_tx));
        Ok(Self {
            tx,
            status,
            task: AsyncMutex::new(Some(task)),
        })
    }
    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }
    async fn request(&self, command: Command, duration: Duration) -> Result<(), String> {
        let (result, rx) = oneshot::channel();
        let request = Request {
            command,
            result,
            expires: Instant::now() + duration,
        };
        self.tx
            .try_send(request)
            .map_err(|_| "Bluetooth input queue unavailable or full".to_string())?;
        tokio::time::timeout(duration, rx)
            .await
            .map_err(|_| "Bluetooth operation timed out".to_string())?
            .map_err(|_| "Bluetooth worker stopped".to_string())?
    }
    pub async fn send(&self, report: Report, value: Vec<u8>) -> Result<(), String> {
        if value.len() != report.len() {
            return Err("Invalid HID report size".into());
        }
        self.request(Command::Report(report, value), Duration::from_millis(500))
            .await
    }
    pub async fn action(&self, action: Action) -> Result<(), String> {
        self.request(Command::Action(action), Duration::from_secs(15))
            .await
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        if let Some(task) = self.task.lock().await.take() {
            let result = self.request(Command::Stop, Duration::from_secs(45)).await;
            // Do not abort: the worker must restore the radio before a backend switch.
            task.await.map_err(|e| e.to_string())?;
            result
        } else {
            Ok(())
        }
    }
}

fn listen(address: Address, psm: u16) -> Result<SeqPacketListener, String> {
    let socket = Socket::<SeqPacket>::new_seq_packet().map_err(|e| e.to_string())?;
    socket
        .set_security(Security {
            level: SecurityLevel::Medium,
            key_size: 0,
        })
        .map_err(|e| e.to_string())?;
    socket.bind(SocketAddr::new(address,AddressType::BrEdr,psm))
        .map_err(|e|format!("Cannot bind classic HID PSM 0x{psm:02x}: {e}. Disable BlueZ input plugin; see Bluetooth HID setup."))?;
    socket.listen(2).map_err(|e| e.to_string())
}
struct Runtime {
    adapter: Adapter,
    _session: Session,
    shared: Arc<Shared>,
    agent: Option<agent::Agent>,
    controller: controller::Controller,
    listeners: [SeqPacketListener; 2],
    channels: [Option<Arc<SeqPacket>>; 2],
    channel_peer: Option<Address>,
    partial_since: Option<Instant>,
    hid: protocol::HidProtocol,
    generation: u64,
    old_pairable: bool,
    old_powered: bool,
    old_alias: String,
    old_discoverable: bool,
    old_discoverable_timeout: u32,
    old_pairable_timeout: u32,
    config: Config,
    bonds: Option<Arc<dyn bonds::BondStore>>,
    recorded_peer: Option<String>,
}
impl Runtime {
    async fn open(
        config: &Config,
        bonds: Option<Arc<dyn bonds::BondStore>>,
    ) -> Result<Self, String> {
        let controller = controller::Controller::acquire(&config.adapter).await?;
        let session = Session::new().await.map_err(|e| e.to_string())?;
        let adapter = session
            .adapter(&config.adapter)
            .map_err(|e| e.to_string())?;
        let address = adapter.address().await.map_err(|e| e.to_string())?;
        let mut config = config.clone();
        if let Some(store) = &bonds {
            bonds::clean_adapter(store.as_ref(), &adapter).await?;
            if config.peer.is_none() {
                config.peer = store
                    .list()
                    .await?
                    .into_iter()
                    .find(|bond| bond.adapter == address.to_string() && !bond.pending)
                    .map(|bond| bond.peer);
            }
        }
        // Acquire BOTH channels before changing adapter state. Conflict is a startup error.
        let listeners = [listen(address, 0x11)?, listen(address, 0x13)?];
        let old_powered = adapter.is_powered().await.map_err(|e| e.to_string())?;
        let old_pairable = adapter.is_pairable().await.map_err(|e| e.to_string())?;
        let old_alias = adapter.alias().await.map_err(|e| e.to_string())?;
        let old_discoverable = adapter.is_discoverable().await.map_err(|e| e.to_string())?;
        let old_discoverable_timeout = adapter
            .discoverable_timeout()
            .await
            .map_err(|e| e.to_string())?;
        let old_pairable_timeout = adapter
            .pairable_timeout()
            .await
            .map_err(|e| e.to_string())?;
        let shared = Arc::new(Shared::new(
            config.peer.as_ref().map(|p| p.to_ascii_uppercase()),
        ));
        let mut runtime = Self {
            adapter,
            _session: session,
            shared,
            agent: None,
            controller,
            listeners,
            channels: [None, None],
            channel_peer: None,
            partial_since: None,
            hid: Default::default(),
            generation: 0,
            old_pairable,
            old_powered,
            old_alias,
            old_discoverable,
            old_discoverable_timeout,
            old_pairable_timeout,
            config: config.clone(),
            bonds,
            recorded_peer: None,
        };
        if let Err(e) = runtime.setup().await {
            let restore = runtime.close().await;
            return Err(format!("{e}; cleanup: {restore:?}"));
        }
        Ok(runtime)
    }
    async fn setup(&mut self) -> Result<(), String> {
        self.adapter
            .set_powered(true)
            .await
            .map_err(|e| e.to_string())?;
        self.adapter
            .set_discoverable(false)
            .await
            .map_err(|e| e.to_string())?;
        self.adapter
            .set_pairable(false)
            .await
            .map_err(|e| e.to_string())?;
        self.adapter
            .set_alias(self.config.name.clone())
            .await
            .map_err(|e| e.to_string())?;
        self.agent =
            Some(agent::Agent::register(self.shared.clone(), self.config.adapter.clone()).await?);
        self.controller.configure().await?;
        self.public(None).await?;
        Ok(())
    }
    async fn public(&self, seconds: Option<u32>) -> Result<(), String> {
        if let Some(seconds) = seconds {
            self.adapter
                .set_pairable_timeout(seconds)
                .await
                .map_err(|e| e.to_string())?;
            self.adapter
                .set_discoverable_timeout(seconds)
                .await
                .map_err(|e| e.to_string())?;
            self.adapter
                .set_pairable(true)
                .await
                .map_err(|e| e.to_string())?;
            if let Err(e) = self.adapter.set_discoverable(true).await {
                let _ = self.adapter.set_pairable(false).await;
                return Err(e.to_string());
            }
        } else {
            self.adapter
                .set_discoverable(false)
                .await
                .map_err(|e| e.to_string())?;
            self.adapter
                .set_pairable(false)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn drop_channels(&mut self) {
        for socket in self.channels.iter_mut().filter_map(Option::take) {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
        self.channel_peer = None;
        self.partial_since = None;
        self.hid = Default::default();
        self.generation = self.generation.wrapping_add(1);
    }
    async fn packet(&self, index: usize, data: &[u8]) -> Result<(), String> {
        let socket = self.channels[index]
            .as_ref()
            .ok_or("Classic HID channel not connected")?;
        let sent = tokio::time::timeout(Duration::from_millis(250), socket.send(data))
            .await
            .map_err(|_| "HID send timeout")?
            .map_err(|e| e.to_string())?;
        if sent != data.len() {
            return Err("Short HID packet write".into());
        }
        Ok(())
    }
    async fn release(&mut self) {
        for packet in self.hid.release() {
            if self.channels[1].is_some() && self.packet(1, &packet).await.is_err() {
                self.drop_channels();
                break;
            }
        }
        self.generation = self.generation.wrapping_add(1);
    }
    async fn accept(
        &mut self,
        index: usize,
        socket: SeqPacket,
        peer: SocketAddr,
    ) -> Result<(), String> {
        if peer.addr_type != AddressType::BrEdr {
            return Ok(());
        }
        // A host can open HID channels before the next status poll.
        let paired = self
            .adapter
            .device(peer.addr)
            .map_err(|e| e.to_string())?
            .is_paired()
            .await
            .map_err(|e| e.to_string())?;
        self.shared
            .state
            .lock()
            .unwrap()
            .observe_bond(&peer.addr.to_string(), paired);
        if !self.shared.allowed(peer.addr) {
            return Ok(());
        }
        if self.channel_peer.is_some_and(|p| p != peer.addr) || self.channels[index].is_some() {
            return Ok(());
        }
        // Kernel BT_SECURITY_MEDIUM negotiates encryption before accept completes.
        if socket
            .as_ref()
            .security()
            .map_err(|e| e.to_string())?
            .key_size
            < 7
        {
            return Ok(());
        }
        self.channel_peer = Some(peer.addr);
        self.channels[index] = Some(Arc::new(socket));
        if self.channels.iter().all(Option::is_some) {
            self.partial_since = None;
            self.release().await;
        } else {
            self.partial_since = Some(Instant::now());
        }
        Ok(())
    }
    async fn status(&mut self) -> Result<Status, String> {
        if !self.adapter.is_powered().await.map_err(|e| e.to_string())? {
            return Err("Bluetooth adapter powered off".into());
        }
        // A daemon restart loses the SDP registration even if the adapter comes back powered.
        if !self
            .adapter
            .uuids()
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
            .iter()
            .any(|id| id.to_string() == protocol::HID_UUID)
        {
            return Err("Classic HID SDP registration lost; restarting Bluetooth backend".into());
        }
        let mut status = Status {
            initialized: true,
            adapter: self.config.adapter.clone(),
            adapter_address: self
                .adapter
                .address()
                .await
                .map_err(|e| e.to_string())?
                .to_string(),
            ..Default::default()
        };
        for address in self
            .adapter
            .device_addresses()
            .await
            .map_err(|e| e.to_string())?
        {
            let device = self.adapter.device(address).map_err(|e| e.to_string())?;
            let connected = device.is_connected().await.unwrap_or(false);
            let paired = device.is_paired().await.unwrap_or(false);
            self.shared
                .state
                .lock()
                .unwrap()
                .observe_bond(&address.to_string(), paired);
            if paired || connected {
                status.devices.push(Device {
                    address: address.to_string(),
                    name: device.alias().await.unwrap_or_default(),
                    paired,
                    connected,
                });
            }
        }
        status.devices.sort_by(|a, b| a.address.cmp(&b.address));
        let (peer, until) = {
            let state = self.shared.state.lock().unwrap();
            (state.peer.clone(), state.pairing_until)
        };
        status.peer = peer.clone();
        let selected = status
            .devices
            .iter()
            .find(|device| Some(&device.address) == peer.as_ref());
        let selected_paired = selected.is_some_and(|device| device.paired);
        if selected_paired && self.recorded_peer != peer {
            if let (Some(store), Some(peer)) = (&self.bonds, &peer) {
                store
                    .save(bonds::Bond {
                        adapter: status.adapter_address.clone(),
                        peer: peer.clone(),
                        pending: false,
                    })
                    .await?;
                self.recorded_peer = Some(peer.clone());
            }
        }
        status.connected = selected.is_some_and(|device| device.paired && device.connected);
        if self.channels.iter().any(Option::is_some)
            && (!status.connected
                || self
                    .partial_since
                    .is_some_and(|time| time.elapsed() > Duration::from_secs(10)))
        {
            self.drop_channels();
        }
        if let Some(until) = until {
            let completed = self
                .shared
                .state
                .lock()
                .unwrap()
                .pairing_completed(selected_paired);
            if until <= Instant::now() || completed {
                {
                    let mut state = self.shared.state.lock().unwrap();
                    state.close_pairing(
                        selected_paired,
                        self.config.peer.as_ref().map(|p| p.to_ascii_uppercase()),
                    );
                    status.peer = state.peer.clone();
                }
                self.public(None).await?;
                tracing::info!(reason = if completed { "bonded" } else { "expired" }, peer = ?status.peer, "Bluetooth HID pairing window closed");
            } else {
                status.pairing_seconds =
                    until.saturating_duration_since(Instant::now()).as_secs() as u32 + 1;
            }
        }
        status.control_connected = self.channels[0].is_some();
        status.interrupt_connected = self.channels[1].is_some();
        status.ready = status.connected
            && status.control_connected
            && status.interrupt_connected
            && !self.hid.suspended;
        status.leds = self.hid.leds;
        status.generation = self.generation;
        Ok(status)
    }
    async fn forget(&mut self) -> Result<(), String> {
        self.release().await;
        self.drop_channels();
        let peer = self.shared.state.lock().unwrap().peer.clone();
        if let Some(peer) = &peer {
            let address = peer.parse().map_err(|_| "Invalid peer")?;
            if self
                .adapter
                .device_addresses()
                .await
                .map_err(|e| e.to_string())?
                .contains(&address)
            {
                self.adapter
                    .remove_device(address)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            self.shared
                .state
                .lock()
                .unwrap()
                .bonded_before_pairing
                .remove(peer);
            tracing::info!(%peer, "Bluetooth HID forgot computer");
        }
        if let Some(store) = &self.bonds {
            let address = self
                .adapter
                .address()
                .await
                .map_err(|e| e.to_string())?
                .to_string();
            for bond in store
                .list()
                .await?
                .into_iter()
                .filter(|b| b.adapter == address && Some(&b.peer) == peer.as_ref())
            {
                store.remove(bond).await?;
            }
        }
        self.recorded_peer = None;
        self.config.peer = None;
        self.shared.state.lock().unwrap().peer = None;
        Ok(())
    }
    async fn incoming(&mut self, index: usize, data: &[u8]) -> Result<(), String> {
        if index == 1 {
            self.hid.output(data);
            return Ok(());
        }
        let result = self.hid.control(data);
        if let Some(reply) = result.reply {
            self.packet(0, &reply).await?;
        }
        if result.reset {
            self.release().await;
        }
        if result.unplug {
            self.forget().await?;
        }
        Ok(())
    }
    async fn execute(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Report(kind, value) => {
                let peer = self.channel_peer.ok_or("Classic HID is not connected")?;
                if !self.shared.allowed(peer) || self.channels.iter().any(Option::is_none) {
                    return Err("Classic HID channels are not ready".into());
                }
                let device = self.adapter.device(peer).map_err(|e| e.to_string())?;
                if !device.is_paired().await.unwrap_or(false)
                    || !device.is_connected().await.unwrap_or(false)
                {
                    self.drop_channels();
                    return Err("Selected Bluetooth computer disconnected".into());
                }
                let packet = self.hid.input(kind, &value)?;
                if let Err(e) = self.packet(1, &packet).await {
                    self.drop_channels();
                    return Err(e);
                }
            }
            Command::Action(Action::Pair(seconds)) => {
                if !(10..=300).contains(&seconds) {
                    return Err("Pairing window must be 10–300 seconds".into());
                }
                let mut bonded = HashSet::new();
                for address in self
                    .adapter
                    .device_addresses()
                    .await
                    .map_err(|e| e.to_string())?
                {
                    if self
                        .adapter
                        .device(address)
                        .map_err(|e| e.to_string())?
                        .is_paired()
                        .await
                        .map_err(|e| e.to_string())?
                    {
                        bonded.insert(address.to_string());
                    }
                }
                {
                    let mut state = self.shared.state.lock().unwrap();
                    // Preserve the original baseline when extending an open window.
                    if !state
                        .pairing_until
                        .is_some_and(|until| until > Instant::now())
                    {
                        state.bonded_before_pairing = bonded;
                    }
                    state.pairing_until =
                        Some(Instant::now() + Duration::from_secs(seconds.into()));
                }
                if let Err(e) = self.public(Some(seconds)).await {
                    self.shared.state.lock().unwrap().pairing_until = None;
                    return Err(e);
                }
                tracing::info!(seconds, "Bluetooth HID pairing window opened");
            }
            Command::Action(Action::ClosePairing) => {
                self.public(None).await?;
                let peer = self.shared.state.lock().unwrap().peer.clone();
                let paired = if let Some(peer) = peer {
                    self.adapter
                        .device(peer.parse().map_err(|_| "Invalid HID peer")?)
                        .map_err(|e| e.to_string())?
                        .is_paired()
                        .await
                        .map_err(|e| e.to_string())?
                } else {
                    false
                };
                self.shared
                    .state
                    .lock()
                    .unwrap()
                    .close_pairing(paired, self.config.peer.clone());
                tracing::info!(reason = "manual", "Bluetooth HID pairing window closed");
            }
            Command::Action(Action::Forget) => self.forget().await?,
            Command::Action(Action::Disconnect) => {
                self.release().await;
                self.drop_channels();
                let peer = self.shared.state.lock().unwrap().peer.clone();
                if let Some(peer) = peer {
                    self.adapter
                        .device(peer.parse().map_err(|_| "Invalid peer")?)
                        .map_err(|e| e.to_string())?
                        .disconnect()
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            Command::Action(Action::Reset) => self.release().await,
            Command::Stop => {}
        }
        Ok(())
    }
    async fn close(&mut self) -> Result<(), String> {
        self.release().await;
        self.drop_channels();
        self.agent.take();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut errors = vec![];
        if let Err(e) = self.adapter.set_discoverable(false).await {
            errors.push(e.to_string());
        }
        if let Err(e) = self.controller.restore().await {
            errors.push(e);
        }
        if let Err(e) = self.adapter.set_alias(self.old_alias.clone()).await {
            errors.push(e.to_string());
        }
        if let Err(e) = self
            .adapter
            .set_discoverable_timeout(self.old_discoverable_timeout)
            .await
        {
            errors.push(e.to_string());
        }
        if let Err(e) = self
            .adapter
            .set_pairable_timeout(self.old_pairable_timeout)
            .await
        {
            errors.push(e.to_string());
        }
        if let Err(e) = self.adapter.set_pairable(self.old_pairable).await {
            errors.push(e.to_string());
        }
        if let Err(e) = self.adapter.set_discoverable(self.old_discoverable).await {
            errors.push(e.to_string());
        }
        if let Err(e) = self.adapter.set_powered(self.old_powered).await {
            errors.push(e.to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}
async fn receive(socket: Option<Arc<SeqPacket>>, buffer: &mut [u8]) -> std::io::Result<usize> {
    match socket {
        Some(socket) => socket.recv(buffer).await,
        None => std::future::pending().await,
    }
}
async fn supervise(
    config: Config,
    bonds: Option<Arc<dyn bonds::BondStore>>,
    mut rx: mpsc::Receiver<Request>,
    tx: watch::Sender<Status>,
) {
    let mut generation = 0;
    loop {
        let mut runtime = match Runtime::open(&config, bonds.clone()).await {
            Ok(runtime) => runtime,
            Err(error) => {
                tx.send_replace(Status {
                    adapter: config.adapter.clone(),
                    error: Some(error.clone()),
                    generation,
                    ..Default::default()
                });
                let delay = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(delay);
                loop {
                    tokio::select! {_=&mut delay=>break,request=rx.recv()=>{
                        let Some(request)=request else{return;};
                        if matches!(request.command,Command::Stop){let _=request.result.send(Ok(()));return;}
                        let _=request.result.send(Err(error.clone()));
                    }}
                }
                continue;
            }
        };
        runtime.generation = generation;
        let mut timer = tokio::time::interval(Duration::from_millis(500));
        let failure = loop {
            let ctl = runtime.channels[0].clone();
            let intr = runtime.channels[1].clone();
            let mut ctl_buffer = [0; 1024];
            let mut intr_buffer = [0; 1024];
            tokio::select! {
                accepted=runtime.listeners[0].accept()=>{match accepted{Ok((socket,peer))=>{if let Err(e)=runtime.accept(0,socket,peer).await{break e;}},Err(e)=>break e.to_string()}}
                accepted=runtime.listeners[1].accept()=>{match accepted{Ok((socket,peer))=>{if let Err(e)=runtime.accept(1,socket,peer).await{break e;}},Err(e)=>break e.to_string()}}
                read=receive(ctl,&mut ctl_buffer)=>{match read{Ok(n)if n>0=>{if runtime.incoming(0,&ctl_buffer[..n]).await.is_err(){runtime.drop_channels();}},_=>runtime.drop_channels()}}
                read=receive(intr,&mut intr_buffer)=>{match read{Ok(n)if n>0=>{if runtime.incoming(1,&intr_buffer[..n]).await.is_err(){runtime.drop_channels();}},_=>runtime.drop_channels()}}
                _=timer.tick()=>{match runtime.status().await{Ok(status)=>{tx.send_if_modified(|old|{if *old!=status{*old=status;true}else{false}});},Err(e)=>break e}}
                request=rx.recv()=>{
                    let Some(request)=request else{let _=runtime.close().await;return;};
                    if matches!(request.command,Command::Stop){let result=runtime.close().await;tx.send_replace(Status{adapter:config.adapter.clone(),..Default::default()});let _=request.result.send(result);return;}
                    if request.expires<=Instant::now()||request.result.is_closed(){runtime.release().await;continue;}
                    let action = matches!(&request.command, Command::Action(_));
                    let result=runtime.execute(request.command).await;
                    if result.is_err(){runtime.release().await;}
                    if action {
                        if let Ok(status) = runtime.status().await { tx.send_replace(status); }
                    }
                    let _=request.result.send(result);
                }
            }
        };
        let restore = runtime.close().await;
        generation = runtime.generation.wrapping_add(1);
        tx.send_replace(Status {
            adapter: config.adapter.clone(),
            error: Some(format!("{failure}; cleanup: {restore:?}")),
            generation,
            ..Default::default()
        });
        drop(runtime);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_pairing_preserves_completed_bonds_but_releases_unpaired_targets() {
        let shared = Shared::new(Some("10:6F:D9:66:97:88".into()));
        let mut state = shared.state.lock().unwrap();
        state.pairing_until = Some(Instant::now() + Duration::from_secs(120));
        state.close_pairing(true, None);
        assert_eq!(state.peer.as_deref(), Some("10:6F:D9:66:97:88"));
        assert!(state.pairing_until.is_none());
        state.close_pairing(false, None);
        assert!(state.peer.is_none());
    }
    #[test]
    fn configuration_validation() {
        let mut c = Config {
            adapter: "hci0".into(),
            name: "One-KVM".into(),
            peer: None,
        };
        assert!(c.validate().is_ok());
        c.adapter = "hci0;reboot".into();
        assert!(c.validate().is_err());
        c.adapter = "hci1".into();
        c.peer = Some("bogus".into());
        assert!(c.validate().is_err());
    }
    #[test]
    fn peer_isolation() {
        let state = Shared::new(Some("10:6F:D9:66:97:88".into()));
        assert!(state.allowed("10:6F:D9:66:97:88".parse().unwrap()));
        assert!(!state.allowed("10:6F:D9:66:97:89".parse().unwrap()));
    }
    #[test]
    fn new_bond_without_agent_callback_completes_pairing() {
        let shared = Shared::new(None);
        let mut state = shared.state.lock().unwrap();
        state.pairing_until = Some(Instant::now() + Duration::from_secs(120));
        let peer = "10:6F:D9:66:97:88";
        state.observe_bond(peer, false);
        assert_eq!(state.peer, None);
        state.observe_bond(peer, true);
        assert_eq!(state.peer.as_deref(), Some(peer));
        assert!(state.pairing_completed(true));
        // A second computer cannot replace the one selected in this window.
        state.observe_bond("10:6F:D9:66:97:89", true);
        assert_eq!(state.peer.as_deref(), Some(peer));
    }
    #[test]
    fn old_bond_does_not_finish_a_new_window() {
        let peer = "10:6F:D9:66:97:88";
        let shared = Shared::new(Some(peer.into()));
        let mut state = shared.state.lock().unwrap();
        state.pairing_until = Some(Instant::now() + Duration::from_secs(120));
        state.bonded_before_pairing.insert(peer.into());
        state.observe_bond(peer, true);
        assert!(!state.pairing_completed(true));
        state.peer = None;
        state.observe_bond(peer, true);
        assert_eq!(state.peer, None);
    }
    #[test]
    fn bond_observation_respects_closed_window_and_pinned_peer() {
        let peer = "10:6F:D9:66:97:88";
        let shared = Shared::new(None);
        let mut state = shared.state.lock().unwrap();
        state.observe_bond(peer, true);
        assert_eq!(state.peer, None);
        state.pairing_until = Some(Instant::now() - Duration::from_secs(1));
        state.observe_bond(peer, true);
        assert_eq!(state.peer, None);
        state.pairing_until = Some(Instant::now() + Duration::from_secs(120));
        state.peer = Some("10:6F:D9:66:97:89".into());
        state.observe_bond(peer, true);
        assert_eq!(state.peer.as_deref(), Some("10:6F:D9:66:97:89"));
    }
}

#[derive(Serialize)]
pub struct AdapterInfo {
    pub name: String,
    pub address: String,
    pub powered: bool,
}
pub async fn adapters() -> Result<Vec<AdapterInfo>, String> {
    let session = Session::new().await.map_err(|e| e.to_string())?;
    let mut output = vec![];
    for name in session.adapter_names().await.map_err(|e| e.to_string())? {
        let adapter = session.adapter(&name).map_err(|e| e.to_string())?;
        output.push(AdapterInfo {
            name,
            address: adapter
                .address()
                .await
                .map_err(|e| e.to_string())?
                .to_string(),
            powered: adapter.is_powered().await.map_err(|e| e.to_string())?,
        });
    }
    Ok(output)
}

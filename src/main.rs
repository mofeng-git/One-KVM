use std::collections::HashSet;
use std::future::Future;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use axum_server::tls_rustls::RustlsConfig;
use clap::{Args, Parser, Subcommand, ValueEnum};
use futures::{stream::FuturesUnordered, StreamExt};
use rustls::crypto::{ring, CryptoProvider};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use one_kvm::auth::{SessionStore, TwoFactorService, UserStore};
use one_kvm::config;
use one_kvm::db::open_database_pool;
use one_kvm::platform::PlatformCapabilities;
use one_kvm::runtime::{RuntimeBuilder, WebConfigOverrides};
use one_kvm::state::ShutdownAction;
use one_kvm::utils::bind_tcp_listener;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

#[derive(Parser, Debug)]
#[command(name = "one-kvm")]
#[command(version, about = "A  open and lightweight IP-KVM solution", long_about = None)]
struct CliArgs {
    /// User management commands
    #[command(subcommand)]
    command: Option<CliCommand>,

    /// Listen address (overrides database config)
    #[arg(short = 'a', long, value_name = "ADDRESS")]
    address: Option<String>,

    /// HTTP port (overrides database config)
    #[arg(
        short = 'p',
        long = "port",
        visible_alias = "http-port",
        value_name = "PORT"
    )]
    http_port: Option<u16>,

    /// HTTPS port (overrides database config)
    #[arg(long, value_name = "PORT")]
    https_port: Option<u16>,

    /// Enable HTTPS (overrides database config)
    #[arg(long)]
    enable_https: bool,

    /// Path to SSL certificate file (generates self-signed if not provided)
    #[arg(long, value_name = "FILE", requires = "ssl_key")]
    ssl_cert: Option<PathBuf>,

    /// Path to SSL private key file
    #[arg(long, value_name = "FILE", requires = "ssl_cert")]
    ssl_key: Option<PathBuf>,

    /// Data directory path
    #[arg(short = 'd', long, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(short = 'l', long, value_name = "LEVEL", default_value = "info")]
    log_level: LogLevel,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Manage local users
    User(UserCommand),
}

#[derive(Args, Debug)]
struct UserCommand {
    #[command(subcommand)]
    action: UserAction,
}

#[derive(Subcommand, Debug)]
enum UserAction {
    /// Set password for the single local user (interactive terminal prompt)
    SetPassword,
    /// Disable TOTP for the single local user
    DisableTotp,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    init_logging(args.log_level);

    CryptoProvider::install_default(ring::default_provider())
        .expect("Failed to install rustls crypto provider");

    tracing::info!("Starting One-KVM v{}", env!("CARGO_PKG_VERSION"));
    let platform = PlatformCapabilities::current();
    tracing::info!(
        "Platform mode: {:?} ({})",
        platform.mode,
        platform.mode_label
    );

    let data_dir = args.data_dir.clone().unwrap_or_else(get_data_dir);
    tracing::info!("Data directory: {}", data_dir.display());

    if let Some(command) = args.command {
        run_cli_command(command, data_dir).await?;
        return Ok(());
    }

    let overrides = WebConfigOverrides {
        address: args.address,
        http_port: args.http_port,
        https_port: args.https_port,
        enable_https: args.enable_https,
        ssl_cert: args.ssl_cert,
        ssl_key: args.ssl_key,
    };
    let mut runtime = RuntimeBuilder::new(data_dir.clone())
        .with_web_overrides(overrides)
        .build()
        .await?;
    let config = runtime.config();
    let state = runtime.state().clone();

    let bind_ips = resolve_bind_addresses(&config.web)?;
    let scheme = if config.web.https_enabled {
        "https"
    } else {
        "http"
    };
    let bind_port = if config.web.https_enabled {
        config.web.https_port
    } else {
        config.web.http_port
    };

    for ip in &bind_ips {
        let addr = SocketAddr::new(*ip, bind_port);
        tracing::info!("Server will listen on: {}://{}", scheme, addr);
    }

    let app = runtime.router();

    let listeners = bind_tcp_listeners(&bind_ips, bind_port)?;

    let shutdown_signal = {
        let shutdown_tx = state.shutdown_tx.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();
        async move {
            tokio::select! {
                result = shutdown_signal() => {
                    if let Err(e) = result {
                        tracing::error!("Failed while waiting for shutdown signal: {}", e);
                    }
                    tracing::info!("SIGINT or SIGTERM received");
                    ShutdownAction::Exit
                }
                request = shutdown_rx.recv() => {
                    match request {
                        Ok(action) => {
                            tracing::info!("Shutdown request received: {:?}", action);
                            action
                        }
                        Err(e) => {
                            tracing::warn!("Shutdown request channel closed: {}", e);
                            ShutdownAction::Exit
                        }
                    }
                }
            }
        }
    };

    let shutdown_action = if config.web.https_enabled {
        let tls_config = if let (Some(cert_path), Some(key_path)) =
            (&config.web.ssl_cert_path, &config.web.ssl_key_path)
        {
            RustlsConfig::from_pem_file(cert_path, key_path).await?
        } else {
            let cert_dir = data_dir.join("certs");
            let cert_path = cert_dir.join("server.crt");
            let key_path = cert_dir.join("server.key");

            if !cert_path.exists() || !key_path.exists() {
                tracing::info!("Generating new self-signed TLS certificate");
                let cert = generate_self_signed_cert()?;
                tokio::fs::create_dir_all(&cert_dir).await?;
                tokio::fs::write(&cert_path, cert.cert.pem()).await?;
                tokio::fs::write(&key_path, cert.signing_key.serialize_pem()).await?;
            } else {
                tracing::info!("Using existing TLS certificate from {}", cert_dir.display());
            }

            RustlsConfig::from_pem_file(&cert_path, &key_path).await?
        };

        let servers = FuturesUnordered::new();
        for listener in listeners {
            let local_addr = listener.local_addr()?;
            tracing::info!("Starting HTTPS server on {}", local_addr);

            let server = axum_server::from_tcp_rustls(listener, tls_config.clone())?
                .serve(app.clone().into_make_service());
            servers.push(server);
        }

        run_servers_until_shutdown(servers, shutdown_signal, "HTTPS").await
    } else {
        let servers = FuturesUnordered::new();
        for listener in listeners {
            let local_addr = listener.local_addr()?;
            tracing::info!("Starting HTTP server on {}", local_addr);

            let listener = tokio::net::TcpListener::from_std(listener)?;
            let server = axum::serve(listener, app.clone());
            servers.push(async move { server.await });
        }

        run_servers_until_shutdown(servers, shutdown_signal, "HTTP").await
    };

    runtime.shutdown().await;
    tracing::info!("Server shutdown complete");
    if let ShutdownAction::Restart { exe_path } = shutdown_action {
        restart_current_process(exe_path)?;
    }
    Ok(())
}

fn init_logging(level: LogLevel) {
    let app_level = match level {
        LogLevel::Error => "error",
        LogLevel::Warn => "warn",
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
        LogLevel::Trace => "trace",
    };
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| app_level.into());

    if let Err(err) = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
    {
        eprintln!("failed to initialize tracing: {}", err);
    }
}

fn get_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("ONE_KVM_DATA_DIR") {
        return PathBuf::from(path);
    }

    #[cfg(windows)]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                return exe_dir.join("one-kvm");
            }
        }
        return std::env::current_dir()
            .map(|dir| dir.join("one-kvm"))
            .unwrap_or_else(|_| PathBuf::from("one-kvm"));
    }

    #[cfg(not(windows))]
    PathBuf::from("/etc/one-kvm")
}

#[cfg(unix)]
async fn shutdown_signal() -> anyhow::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = terminate.recv() => {},
    }
    Ok(())
}

#[cfg(not(unix))]
async fn shutdown_signal() -> anyhow::Result<()> {
    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn run_servers_until_shutdown<F, E>(
    mut servers: FuturesUnordered<F>,
    shutdown_signal: impl Future<Output = ShutdownAction>,
    protocol: &'static str,
) -> ShutdownAction
where
    F: Future<Output = Result<(), E>> + Send,
    E: std::fmt::Display,
{
    tokio::select! {
        action = shutdown_signal => {
            action
        }
        result = servers.next() => {
            if let Some(Err(e)) = result {
                tracing::error!("{} server error: {}", protocol, e);
            }
            ShutdownAction::Exit
        }
    }
}

fn restart_current_process(exe_path: Option<PathBuf>) -> anyhow::Result<()> {
    let exe = exe_path.unwrap_or(std::env::current_exe()?);
    let args: Vec<String> = std::env::args().skip(1).collect();

    tracing::info!("Restarting: {:?} {:?}", exe, args);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&exe).args(&args).exec();
        Err(anyhow::anyhow!("Failed to restart: {}", err))
    }

    #[cfg(not(unix))]
    {
        std::process::Command::new(&exe).args(&args).spawn()?;
        std::process::exit(0);
    }
}

async fn run_cli_command(command: CliCommand, data_dir: PathBuf) -> anyhow::Result<()> {
    let db = open_database_pool(&data_dir).await?;
    let users = UserStore::new(db.clone_pool());
    let two_factor = TwoFactorService::new(db.clone_pool());
    let sessions = SessionStore::new(0);

    match command {
        CliCommand::User(user) => {
            run_user_action(user.action, &users, &sessions, &two_factor).await
        }
    }
}

async fn run_user_action(
    action: UserAction,
    users: &UserStore,
    sessions: &SessionStore,
    two_factor: &TwoFactorService,
) -> anyhow::Result<()> {
    match action {
        UserAction::SetPassword => set_user_password(users, sessions).await,
        UserAction::DisableTotp => disable_user_totp(users, two_factor).await,
    }
}

async fn disable_user_totp(users: &UserStore, two_factor: &TwoFactorService) -> anyhow::Result<()> {
    let user = users.single_user().await?.ok_or_else(|| {
        anyhow::anyhow!("No local user exists yet; complete setup in the web UI first.")
    })?;
    if two_factor.disable_without_code(&user.id).await? {
        println!("TOTP disabled for user '{}'.", user.username);
    } else {
        println!("TOTP is already disabled for user '{}'.", user.username);
    }
    Ok(())
}

async fn set_user_password(users: &UserStore, sessions: &SessionStore) -> anyhow::Result<()> {
    let user = users.single_user().await?.ok_or_else(|| {
        anyhow::anyhow!("No local user exists yet; complete setup in the web UI first.")
    })?;

    let new_password = read_new_password_interactive()?;
    if new_password.len() < 4 {
        anyhow::bail!("Password must be at least 4 characters");
    }

    users.update_password(&user.id, &new_password).await?;
    let revoked = sessions.delete_all().await?;

    tracing::info!(
        "Password updated for user '{}' and {} sessions revoked",
        user.username,
        revoked
    );
    println!(
        "Password updated for user '{}' (revoked {} sessions).",
        user.username, revoked
    );
    Ok(())
}

fn read_new_password_interactive() -> anyhow::Result<String> {
    let once = |label: &str| -> anyhow::Result<String> {
        print!("{}", label);
        std::io::stdout().flush()?;

        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        let s = line.trim_end_matches(['\r', '\n']).to_string();
        if s.is_empty() {
            anyhow::bail!("Password cannot be empty");
        }
        Ok(s)
    };

    let a = once("New password: ")?;
    let b = once("Confirm password: ")?;
    if a != b {
        anyhow::bail!("Passwords do not match");
    }
    Ok(a)
}

fn resolve_bind_addresses(web: &config::WebConfig) -> anyhow::Result<Vec<IpAddr>> {
    let raw_addrs = if !web.bind_addresses.is_empty() {
        web.bind_addresses.as_slice()
    } else {
        std::slice::from_ref(&web.bind_address)
    };

    let mut seen = HashSet::new();
    let mut addrs = Vec::new();
    for addr in raw_addrs {
        let ip: IpAddr = addr
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid bind address: {}", addr))?;
        if seen.insert(ip) {
            addrs.push(ip);
        }
    }

    Ok(addrs)
}

fn bind_tcp_listeners(addrs: &[IpAddr], port: u16) -> anyhow::Result<Vec<std::net::TcpListener>> {
    let mut listeners = Vec::new();
    for ip in addrs {
        let addr = SocketAddr::new(*ip, port);
        match bind_tcp_listener(addr) {
            Ok(listener) => listeners.push(listener),
            Err(err) => {
                tracing::warn!("Failed to bind {}: {}", addr, err);
            }
        }
    }

    if listeners.is_empty() {
        anyhow::bail!("Failed to bind any addresses on port {}", port);
    }

    Ok(listeners)
}

fn generate_self_signed_cert() -> anyhow::Result<rcgen::CertifiedKey<rcgen::KeyPair>> {
    use rcgen::generate_simple_self_signed;

    let subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];

    let certified_key = generate_simple_self_signed(subject_alt_names)?;
    Ok(certified_key)
}

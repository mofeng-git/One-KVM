use one_kvm::runtime::android::{self, AndroidRuntimeConfig};

fn main() {
    let mut args = std::env::args().skip(1);
    let data_dir = args
        .next()
        .unwrap_or_else(|| "/data/local/tmp/one-kvm".to_string());
    let bind_address = args.next().unwrap_or_else(|| "0.0.0.0".to_string());
    let port = args
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);

    one_kvm::runtime::android::init_rustls_provider();

    if let Err(err) = android::run_foreground(AndroidRuntimeConfig {
        data_dir,
        bind_address,
        port,
    }) {
        eprintln!("one-kvm android host failed: {err}");
        std::process::exit(1);
    }
}

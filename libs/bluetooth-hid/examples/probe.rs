//! Native hardware smoke check. Registers for 12 seconds, sends no input,
//! opens no pairing window, then restores the adapter configuration.
use one_kvm_bluetooth_hid::{Config, Peripheral};
use std::time::Duration;
#[tokio::main]
async fn main() -> Result<(), String> {
    let peripheral = Peripheral::start(Config {
        adapter: "hci0".into(),
        name: "One-KVM HID".into(),
        peer: None,
    })?;
    let mut initialized = false;
    for _ in 0..12 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status = peripheral.status();
        initialized |= status.initialized;
        println!("{}", serde_json::to_string(&status).unwrap());
    }
    peripheral.shutdown().await?;
    if !initialized {
        return Err("Native BlueZ peripheral never initialized".into());
    }
    Ok(())
}

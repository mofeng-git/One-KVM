//! Explicit hardware regression: opens a 10-second pairing window without
//! removing existing bonds or sending input. Stop the production HID first.
use one_kvm_bluetooth_hid::{Action, Config, Peripheral};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), String> {
    let peripheral = Peripheral::start(Config {
        adapter: "hci0".into(),
        name: "One-KVM HID".into(),
        peer: None,
    })?;
    let result = async {
        for _ in 0..50 {
            if peripheral.status().initialized {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let before = peripheral.status();
        if !before.initialized {
            return Err(format!("Initialization failed: {:?}", before.error));
        }
        if !before
            .devices
            .iter()
            .any(|d| Some(&d.address) == before.peer.as_ref() && d.paired)
        {
            return Err("This regression needs an existing bonded computer".into());
        }
        peripheral.action(Action::Pair(10)).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let open = peripheral.status();
        println!("open: {}", serde_json::to_string(&open).unwrap());
        if open.pairing_seconds == 0 {
            return Err("Existing bond prematurely closed pairing".into());
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
        let expired = peripheral.status();
        println!("expired: {}", serde_json::to_string(&expired).unwrap());
        if expired.pairing_seconds != 0 || expired.peer != before.peer {
            return Err("Window did not expire while preserving the bonded peer".into());
        }
        Ok(())
    }
    .await;
    let cleanup = peripheral.shutdown().await;
    result.and(cleanup)
}

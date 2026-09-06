use crate::Shared;
use dbus::{channel::MatchingReceiver, message::MatchRule, nonblock::Proxy, Path};
use dbus_crossroads::Crossroads;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const PATH: &str = "/org/onekvm/bluetooth/agent";
pub struct Agent {
    connection: Arc<dbus::nonblock::SyncConnection>,
    task: tokio::task::JoinHandle<()>,
}
impl Agent {
    pub async fn register(shared: Arc<Shared>, adapter: String) -> Result<Self, String> {
        let (resource, connection) =
            dbus_tokio::connection::new_system_sync().map_err(|e| e.to_string())?;
        let task = tokio::spawn(async move {
            let _ = resource.await;
        });
        let mut cr = Crossroads::new();
        let iface = cr.register("org.bluez.Agent1", |b| {
            b.method("Release", (), (), |_, _: &mut (), ()| Ok(()));
            b.method("Cancel", (), (), |_, _: &mut (), ()| Ok(()));
            let state = shared.clone();
            let name = adapter.clone();
            b.method(
                "RequestAuthorization",
                ("device",),
                (),
                move |_, _: &mut (), (path,): (Path<'static>,)| {
                    authorize(&state, &name, &path, true)
                },
            );
            let state = shared.clone();
            let name = adapter.clone();
            b.method(
                "RequestConfirmation",
                ("device", "passkey"),
                (),
                move |_, _: &mut (), (path, _): (Path<'static>, u32)| {
                    authorize(&state, &name, &path, true)
                },
            );
            b.method(
                "AuthorizeService",
                ("device", "uuid"),
                (),
                move |_, _: &mut (), (path, uuid): (Path<'static>, String)| {
                    if !matches!(uuid.as_str(), "00001124-0000-1000-8000-00805f9b34fb") {
                        return Err(dbus::MethodErr::from((
                            "org.bluez.Error.Rejected",
                            "Not an HID service",
                        )));
                    }
                    authorize(&shared, &adapter, &path, false)
                },
            );
        });
        cr.insert(PATH, &[iface], ());
        let profile = cr.register("org.bluez.Profile1", |b| {
            b.method("Release", (), (), |_, _: &mut (), ()| Ok(()));
            b.method(
                "RequestDisconnection",
                ("device",),
                (),
                |_, _: &mut (), (_device,): (Path<'static>,)| Ok(()),
            );
            b.method(
                "NewConnection",
                ("device", "fd", "properties"),
                (),
                |_,
                 _: &mut (),
                 (_device, _fd, _props): (
                    Path<'static>,
                    dbus::arg::OwnedFd,
                    dbus::arg::PropMap,
                )| {
                    // This profile only publishes SDP. Our L2CAP sockets own both HID channels.
                    Err::<(), _>(dbus::MethodErr::from((
                        "org.bluez.Error.Rejected",
                        "Unexpected profile connection",
                    )))
                },
            );
        });
        cr.insert("/org/onekvm/bluetooth/profile", &[profile], ());
        connection.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |msg, conn| {
                let _ = cr.handle_message(msg, conn);
                true
            }),
        );
        let agent = Self { connection, task };
        let proxy = Proxy::new(
            "org.bluez",
            "/org/bluez",
            Duration::from_secs(5),
            agent.connection.clone(),
        );
        let result: Result<(), dbus::Error> = proxy
            .method_call(
                "org.bluez.AgentManager1",
                "RegisterAgent",
                (Path::from(PATH), "NoInputNoOutput"),
            )
            .await;
        if let Err(error) = result {
            return Err(error.to_string());
        }
        let result: Result<(), dbus::Error> = proxy
            .method_call(
                "org.bluez.AgentManager1",
                "RequestDefaultAgent",
                (Path::from(PATH),),
            )
            .await;
        result.map_err(|e| e.to_string())?;
        let mut options: dbus::arg::PropMap = std::collections::HashMap::new();
        options.insert(
            "ServiceRecord".into(),
            dbus::arg::Variant(Box::new(crate::protocol::sdp())),
        );
        options.insert(
            "Role".into(),
            dbus::arg::Variant(Box::new("server".to_string())),
        );
        options.insert(
            "RequireAuthentication".into(),
            dbus::arg::Variant(Box::new(true)),
        );
        options.insert(
            "RequireAuthorization".into(),
            dbus::arg::Variant(Box::new(false)),
        );
        let result: Result<(), dbus::Error> = proxy
            .method_call(
                "org.bluez.ProfileManager1",
                "RegisterProfile",
                (
                    Path::from("/org/onekvm/bluetooth/profile"),
                    crate::protocol::HID_UUID,
                    options,
                ),
            )
            .await;
        result.map_err(|e| format!("Register HID SDP: {e}"))?;
        Ok(agent)
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn authorize(
    shared: &Shared,
    adapter: &str,
    path: &str,
    pairing: bool,
) -> Result<(), dbus::MethodErr> {
    let rejected = || {
        dbus::MethodErr::from((
            "org.bluez.Error.Rejected",
            "Open pairing in One-KVM or select the paired computer",
        ))
    };
    let prefix = format!("/org/bluez/{adapter}/dev_");
    let address = path
        .strip_prefix(&prefix)
        .ok_or_else(rejected)?
        .replace('_', ":");
    let mut state = shared.state.lock().unwrap();
    if pairing
        && !state
            .pairing_until
            .is_some_and(|until| until > Instant::now())
    {
        return Err(rejected());
    }
    if state.peer.as_ref().is_some_and(|peer| peer != &address) {
        return Err(rejected());
    }
    if state.peer.is_none() {
        if !pairing || state.bonded_before_pairing.contains(&address) {
            return Err(rejected());
        }
        state.peer = Some(address);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_unrelated_bond_cannot_be_adopted_by_agent_callback() {
        let shared = Shared::new(None);
        {
            let mut state = shared.state.lock().unwrap();
            state.pairing_until = Some(Instant::now() + Duration::from_secs(120));
            state
                .bonded_before_pairing
                .insert("10:6F:D9:66:97:88".into());
        }
        assert!(authorize(
            &shared,
            "hci0",
            "/org/bluez/hci0/dev_10_6F_D9_66_97_88",
            true
        )
        .is_err());
        assert!(shared.state.lock().unwrap().peer.is_none());
    }
    #[test]
    fn only_pair_during_window_and_pin_first_peer() {
        let state = Shared::new(None);
        let first = "/org/bluez/hci0/dev_10_6F_D9_66_97_88";
        assert!(authorize(&state, "hci0", first, true).is_err());
        state.state.lock().unwrap().pairing_until = Some(Instant::now() + Duration::from_secs(60));
        assert!(authorize(&state, "hci0", first, true).is_ok());
        assert!(authorize(
            &state,
            "hci0",
            "/org/bluez/hci0/dev_10_6F_D9_66_97_89",
            true
        )
        .is_err());
        assert!(authorize(&state, "hci1", first, true).is_err());
        state.state.lock().unwrap().pairing_until = None;
        assert!(authorize(&state, "hci0", first, false).is_ok());
        assert!(authorize(&state, "hci0", first, true).is_err());
    }
}

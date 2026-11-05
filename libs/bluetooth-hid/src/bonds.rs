//! Only bonds explicitly owned by this HID peripheral may be removed.
use bluer::{Adapter, Session};
use std::{future::Future, pin::Pin};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bond {
    pub adapter: String,
    pub peer: String,
    pub pending: bool,
}
pub type Operation<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;
pub trait BondStore: Send + Sync {
    fn list(&self) -> Operation<'_, Vec<Bond>>;
    fn save(&self, bond: Bond) -> Operation<'_, ()>;
    fn remove(&self, bond: Bond) -> Operation<'_, ()>;
}

pub async fn clean_adapter(store: &dyn BondStore, adapter: &Adapter) -> Result<(), String> {
    let address = adapter
        .address()
        .await
        .map_err(|e| e.to_string())?
        .to_string();
    clean_owned(store, &address, |peer| async move {
        let peer = peer.parse().map_err(|_| "Invalid stored HID peer")?;
        if adapter
            .device_addresses()
            .await
            .map_err(|e| e.to_string())?
            .contains(&peer)
        {
            adapter
                .remove_device(peer)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
}

async fn clean_owned<F, Fut>(
    store: &dyn BondStore,
    address: &str,
    mut remove: F,
) -> Result<(), String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    for bond in store
        .list()
        .await?
        .into_iter()
        .filter(|b| b.pending && b.adapter == address)
    {
        remove(bond.peer.clone()).await?;
        // Keep the tombstone until BlueZ confirms removal; retry safely after interruption.
        store.remove(bond).await?;
    }
    Ok(())
}

pub async fn reset(store: &dyn BondStore, explicit: Vec<Bond>) -> Result<(), String> {
    let session = Session::new().await.map_err(|e| e.to_string())?;
    let mut owned = store.list().await?;
    owned.extend(explicit);
    for mut bond in owned {
        bond.pending = true;
        store.save(bond).await?;
    }
    for name in session.adapter_names().await.map_err(|e| e.to_string())? {
        clean_adapter(store, &session.adapter(&name).map_err(|e| e.to_string())?).await?;
    }
    // Records for absent adapters remain pending and are cleaned before their next use.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Memory(Mutex<Vec<Bond>>);
    impl BondStore for Memory {
        fn list(&self) -> Operation<'_, Vec<Bond>> {
            Box::pin(async { Ok(self.0.lock().unwrap().clone()) })
        }
        fn save(&self, bond: Bond) -> Operation<'_, ()> {
            Box::pin(async move {
                let mut rows = self.0.lock().unwrap();
                rows.retain(|b| b.adapter != bond.adapter || b.peer != bond.peer);
                rows.push(bond);
                Ok(())
            })
        }
        fn remove(&self, bond: Bond) -> Operation<'_, ()> {
            Box::pin(async move {
                self.0.lock().unwrap().retain(|b| b != &bond);
                Ok(())
            })
        }
    }
    fn bond(adapter: &str, peer: &str, pending: bool) -> Bond {
        Bond {
            adapter: adapter.into(),
            peer: peer.into(),
            pending,
        }
    }
    #[tokio::test]
    async fn cleanup_only_removes_owned_pending_bonds_on_the_selected_hardware() {
        let current = bond("adapter-A", "host-1", true);
        let absent = bond("adapter-B", "host-2", true);
        let retained = bond("adapter-A", "host-3", false);
        let store = Memory(Mutex::new(vec![current, absent.clone(), retained.clone()]));
        let removed = Mutex::new(Vec::new());
        clean_owned(&store, "adapter-A", |peer| {
            removed.lock().unwrap().push(peer);
            async { Ok(()) }
        })
        .await
        .unwrap();
        assert_eq!(*removed.lock().unwrap(), vec!["host-1"]);
        assert_eq!(store.list().await.unwrap(), vec![absent, retained]);
    }
    #[tokio::test]
    async fn cleanup_failure_keeps_tombstone_and_is_reported() {
        let record = bond("adapter-A", "host-1", true);
        let store = Memory(Mutex::new(vec![record.clone()]));
        assert_eq!(
            clean_owned(&store, "adapter-A", |_| async {
                Err("BlueZ denied removal".into())
            })
            .await
            .unwrap_err(),
            "BlueZ denied removal"
        );
        assert_eq!(store.list().await.unwrap(), vec![record]);
        clean_owned(&store, "adapter-A", |_| async { Ok(()) })
            .await
            .unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }
}

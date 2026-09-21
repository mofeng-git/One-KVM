use one_kvm_bluetooth_hid::bonds::{Bond, BondStore, Operation};
use sqlx::{Pool, Sqlite};

#[derive(Clone)]
pub struct HidBondStore(pub Pool<Sqlite>);
impl BondStore for HidBondStore {
    fn list(&self) -> Operation<'_, Vec<Bond>> {
        Box::pin(async move {
            let rows: Vec<(String, String, bool)> = sqlx::query_as(
                "SELECT adapter, peer, pending FROM hid_bonds ORDER BY adapter, peer",
            )
            .fetch_all(&self.0)
            .await
            .map_err(|e| e.to_string())?;
            Ok(rows
                .into_iter()
                .map(|(adapter, peer, pending)| Bond {
                    adapter,
                    peer,
                    pending,
                })
                .collect())
        })
    }
    fn save(&self, bond: Bond) -> Operation<'_, ()> {
        Box::pin(async move {
            sqlx::query("INSERT INTO hid_bonds(adapter, peer, pending) VALUES (?, ?, ?) ON CONFLICT(adapter, peer) DO UPDATE SET pending = MAX(pending, excluded.pending)")
                .bind(bond.adapter.to_ascii_uppercase()).bind(bond.peer.to_ascii_uppercase()).bind(bond.pending)
                .execute(&self.0).await.map_err(|e| e.to_string())?;
            Ok(())
        })
    }
    fn remove(&self, bond: Bond) -> Operation<'_, ()> {
        Box::pin(async move {
            sqlx::query("DELETE FROM hid_bonds WHERE adapter = ? AND peer = ?")
                .bind(bond.adapter)
                .bind(bond.peer)
                .execute(&self.0)
                .await
                .map_err(|e| e.to_string())?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn ownership_and_pending_cleanup_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db = super::super::open_database_pool(dir.path()).await.unwrap();
        let store = HidBondStore(db.clone_pool());
        let bond = Bond {
            adapter: "AA:BB:CC:DD:EE:FF".into(),
            peer: "11:22:33:44:55:66".into(),
            pending: false,
        };
        store.save(bond.clone()).await.unwrap();
        store
            .save(Bond {
                pending: true,
                ..bond.clone()
            })
            .await
            .unwrap();
        store.save(bond.clone()).await.unwrap(); // Late status cannot undo a pending reset.
        let reopened = super::super::open_database_pool(dir.path()).await.unwrap();
        let records = HidBondStore(reopened.clone_pool()).list().await.unwrap();
        assert_eq!(
            records,
            vec![Bond {
                pending: true,
                ..bond.clone()
            }]
        );
        store.remove(bond).await.unwrap();
        assert!(store.list().await.unwrap().is_empty());
    }
}

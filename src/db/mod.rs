mod pool;
mod wol_history;

use std::path::Path;

use crate::error::Result;

pub use pool::DatabasePool;
pub use wol_history::WolHistoryStore;

/// Open the application database stored in `data_dir` and ensure its schema exists.
pub async fn open_database_pool(data_dir: &Path) -> Result<DatabasePool> {
    let db = DatabasePool::new(&data_dir.join("one-kvm.db")).await?;
    db.init_schema().await?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_database_pool_creates_data_dir_and_initializes_schema() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("nested").join("data");

        let db = open_database_pool(&data_dir).await.unwrap();

        assert!(data_dir.join("one-kvm.db").is_file());
        let users_table: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'users'",
        )
        .fetch_optional(db.pool())
        .await
        .unwrap();
        assert_eq!(users_table.as_deref(), Some("users"));
    }
}

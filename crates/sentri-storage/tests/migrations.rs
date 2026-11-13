use sentri_storage::Store;

#[tokio::test]
async fn opens_file_database_and_applies_migrations_twice() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/sentri.db");
    let store = Store::open(&path).await.unwrap();
    store.close().await;
    // Re-opening must be idempotent.
    let store = Store::open(&path).await.unwrap();
    store.close().await;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[tokio::test]
async fn backup_writes_snapshot_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("sentri.db")).await.unwrap();
    let target = dir.path().join("backup.db");
    store.backup(&target).await.unwrap();
    assert!(target.exists());
    assert!(store.backup(&target).await.is_err());
    Store::open(&target).await.unwrap();
}

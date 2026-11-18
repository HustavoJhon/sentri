mod common;

use common::opp;
use sentri_core::Category;
use sentri_storage::Store;

/// Parallel collection writes from several connections at once. Deferred
/// transactions used to fail with "database is locked" (`SQLITE_BUSY_SNAPSHOT`).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_do_not_fail_with_database_locked() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("sentri.db")).await.unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for writer in 0..8 {
        let store = store.clone();
        tasks.spawn(async move {
            for i in 0..25 {
                let url = format!("https://example.com/{writer}/{i}");
                store
                    .upsert_opportunity(&opp(&url, "Item", Category::Event))
                    .await?;
            }
            Ok::<_, sentri_storage::StorageError>(())
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    assert_eq!(
        store
            .stats(chrono::Utc::now(), chrono::Utc::now())
            .await
            .unwrap()
            .total,
        200
    );
}

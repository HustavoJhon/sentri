mod common;

use chrono::Duration;
use common::{now, opp};
use sentri_core::{Action, Category, Flag};
use sentri_storage::{RunStatus, Store};

#[tokio::test]
async fn records_latest_run_per_source() {
    let store = Store::in_memory().await.unwrap();
    let first = store.start_run("rss", now()).await.unwrap();
    store
        .finish_run(first, now(), Err("timeout"))
        .await
        .unwrap();
    let second = store.start_run("rss", now()).await.unwrap();
    store.finish_run(second, now(), Ok((10, 3))).await.unwrap();
    let gh = store.start_run("github", now()).await.unwrap();

    let runs = store.latest_runs().await.unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].source_name, "github");
    assert_eq!(runs[0].status, RunStatus::Running);
    assert_eq!(runs[0].id, gh);
    assert_eq!(runs[1].status, RunStatus::Succeeded);
    assert_eq!((runs[1].items_seen, runs[1].items_new), (10, 3));
}

#[tokio::test]
async fn computes_dashboard_stats() {
    let store = Store::in_memory().await.unwrap();
    let mut a = opp("https://a.example/", "A", Category::Hackathon);
    a.dates.deadline = Some(now() + Duration::days(2));
    let id = store.upsert_opportunity(&a).await.unwrap().id();
    store
        .upsert_opportunity(&opp("https://b.example/", "B", Category::Job))
        .await
        .unwrap();
    store
        .apply_action(id, Action::Set(Flag::Saved), now())
        .await
        .unwrap();

    let stats = store.stats(now() - Duration::days(1), now()).await.unwrap();
    assert_eq!(stats.total, 2);
    assert_eq!(stats.new_since, 2);
    assert_eq!(stats.saved, 1);
    assert_eq!(stats.upcoming_deadlines, 1);
}

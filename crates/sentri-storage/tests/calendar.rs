mod common;

use chrono::Duration;
use common::{now, opp};
use sentri_core::{Action, Category, EntryKind};
use sentri_storage::{NewEntry, StorageError, Store};

#[tokio::test]
async fn scheduling_creates_one_entry_per_date_kind() {
    let store = Store::in_memory().await.unwrap();
    let mut o = opp("https://event.example/", "DevFest", Category::Conference);
    o.dates.starts_at = Some(now() + Duration::days(10));
    o.dates.deadline = Some(now() + Duration::days(3));
    let id = store.upsert_opportunity(&o).await.unwrap().id();

    let entries = store.schedule_opportunity(id, now()).await.unwrap();
    let kinds: Vec<EntryKind> = entries.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EntryKind::Deadline, EntryKind::Event]);
    // Again: no duplicates, and history logs it once.
    assert_eq!(
        store.schedule_opportunity(id, now()).await.unwrap().len(),
        2
    );
    let scheduled = store
        .history(10)
        .await
        .unwrap()
        .iter()
        .filter(|h| h.action == Action::Scheduled)
        .count();
    assert_eq!(scheduled, 1);

    let undated = store
        .upsert_opportunity(&opp("https://x.example/", "X", Category::Other))
        .await
        .unwrap()
        .id();
    assert!(matches!(
        store.schedule_opportunity(undated, now()).await,
        Err(StorageError::Invalid(_))
    ));
}

#[tokio::test]
async fn personal_entries_crud_and_range_queries() {
    let store = Store::in_memory().await.unwrap();
    let new = NewEntry {
        kind: EntryKind::Personal,
        title: "Study Rust".into(),
        starts_at: now(),
        ends_at: Some(now() + Duration::hours(2)),
        all_day: false,
        notes: None,
    };
    let entry = store.create_entry(&new, now()).await.unwrap();
    let edited = store
        .update_entry(
            entry.id,
            &NewEntry {
                title: "Study Rust ch. 8".into(),
                ..new.clone()
            },
            now(),
        )
        .await
        .unwrap();
    assert_eq!(edited.title, "Study Rust ch. 8");
    let bad = NewEntry {
        ends_at: Some(now() - Duration::hours(1)),
        ..new.clone()
    };
    assert!(store.create_entry(&bad, now()).await.is_err());

    let range = store
        .entries_between(now() - Duration::days(1), now() + Duration::days(1))
        .await
        .unwrap();
    assert_eq!(range.len(), 1);
    store.delete_entry(entry.id).await.unwrap();
    assert!(store.delete_entry(entry.id).await.is_err());

    let mut dated = opp("https://d.example/", "Deadline", Category::Scholarship);
    dated.dates.deadline = Some(now() + Duration::hours(5));
    store.upsert_opportunity(&dated).await.unwrap();
    let found = store
        .dated_between(now(), now() + Duration::days(1))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
}

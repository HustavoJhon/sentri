mod common;

use chrono::{Duration, NaiveDate};
use common::{now, opp};
use sentri_core::{Action, Category, Flag};
use sentri_storage::Store;

#[tokio::test]
async fn aggregates_for_dashboard() {
    let store = Store::in_memory().await.unwrap();
    let a = store
        .upsert_opportunity(&opp("https://a.example/", "A", Category::Course))
        .await
        .unwrap()
        .id();
    store
        .upsert_opportunity(&opp("https://b.example/", "B", Category::Course))
        .await
        .unwrap();
    store
        .upsert_opportunity(&opp("https://c.example/", "C", Category::Job))
        .await
        .unwrap();
    store
        .apply_action(a, Action::Set(Flag::Saved), now())
        .await
        .unwrap();
    store
        .apply_action(a, Action::Set(Flag::Liked), now())
        .await
        .unwrap();
    // Repeated toggles the same day count once.
    store
        .apply_action(a, Action::Clear(Flag::Liked), now())
        .await
        .unwrap();
    store
        .apply_action(a, Action::Set(Flag::Liked), now())
        .await
        .unwrap();

    let i = store.insights(now(), 30, 0).await.unwrap();
    let today = now().date_naive();
    assert_eq!(i.discovered_per_day.get(&today), Some(&3));
    assert_eq!(i.likes_per_day.get(&today), Some(&1));
    assert_eq!(i.saves_per_day.get(&today), Some(&1));
    assert_eq!(i.by_category[0], (Category::Course, 2));
    assert_eq!(i.pending_courses, 1);
}

#[tokio::test]
async fn activity_is_deduplicated_and_uses_local_days() {
    let store = Store::in_memory().await.unwrap();
    let mut manual = opp("https://m.example/", "Manual", Category::Event);
    manual.source_name = "manual".into();
    let id = store.upsert_opportunity(&manual).await.unwrap().id();
    for _ in 0..3 {
        store
            .apply_action(id, Action::Set(Flag::Saved), now())
            .await
            .unwrap();
    }
    store.apply_action(id, Action::Viewed, now()).await.unwrap();

    let since = now() - Duration::days(7);
    let days = store.activity(since, 0).await.unwrap();
    let today = days.get(&now().date_naive()).unwrap();
    assert_eq!(today.added_manually, 1);
    assert_eq!(today.actions.get("saved"), Some(&1));
    assert_eq!(today.total(), 2, "views are not counted");

    // now() is 12:00 UTC; at UTC-13 it is still the previous day.
    let shifted = store.activity(since, -13 * 3600).await.unwrap();
    assert!(shifted.contains_key(&NaiveDate::from_ymd_opt(2026, 11, 12).unwrap()));

    let entries = store.day_entries(now().date_naive(), 0).await.unwrap();
    let actions: Vec<&str> = entries.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(actions, ["added", "saved", "viewed"]);
}

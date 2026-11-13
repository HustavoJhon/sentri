mod common;

use common::{now, opp};
use sentri_core::{Action, Category, FeedQuery, FeedView, Flag, OpportunityId};
use sentri_storage::{StorageError, Store};

async fn store_with_two() -> (Store, OpportunityId, OpportunityId) {
    let store = Store::in_memory().await.unwrap();
    let a = store
        .upsert_opportunity(&opp("https://a.example/", "A", Category::Course))
        .await
        .unwrap()
        .id();
    let b = store
        .upsert_opportunity(&opp("https://b.example/", "B", Category::Job))
        .await
        .unwrap()
        .id();
    (store, a, b)
}

async fn ids(store: &Store, view: FeedView) -> Vec<OpportunityId> {
    store
        .list(&FeedQuery::view(view), now())
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.id)
        .collect()
}

#[tokio::test]
async fn flags_are_independent_and_persisted() {
    let (store, a, _) = store_with_two().await;
    store
        .apply_action(a, Action::Set(Flag::Liked), now())
        .await
        .unwrap();
    let state = store
        .apply_action(a, Action::Set(Flag::Saved), now())
        .await
        .unwrap();
    assert!(state.liked && state.saved);
    let state = store.toggle_flag(a, Flag::Liked, now()).await.unwrap();
    assert!(!state.liked && state.saved);
    assert_eq!(store.get(a).await.unwrap().state, state);
}

#[tokio::test]
async fn views_reflect_flags_and_dismissed_items_leave_the_feed() {
    let (store, a, b) = store_with_two().await;
    store
        .apply_action(a, Action::Set(Flag::Saved), now())
        .await
        .unwrap();
    store
        .apply_action(b, Action::Set(Flag::Dismissed), now())
        .await
        .unwrap();

    assert_eq!(ids(&store, FeedView::Saved).await, [a]);
    assert_eq!(ids(&store, FeedView::Dismissed).await, [b]);
    assert_eq!(ids(&store, FeedView::Discover).await, [a]);

    store
        .apply_action(b, Action::Clear(Flag::Dismissed), now())
        .await
        .unwrap();
    assert_eq!(ids(&store, FeedView::Discover).await.len(), 2);
}

#[tokio::test]
async fn history_records_every_action() {
    let (store, a, b) = store_with_two().await;
    store.apply_action(a, Action::Viewed, now()).await.unwrap();
    store
        .apply_action(
            b,
            Action::Set(Flag::Applied),
            now() + chrono::Duration::seconds(1),
        )
        .await
        .unwrap();
    let history = store.history(10).await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].action, Action::Set(Flag::Applied));
    assert_eq!(history[1].title, "A");
    assert_eq!(ids(&store, FeedView::History).await, [a]);
    assert_eq!(ids(&store, FeedView::Applied).await, [b]);
}

#[tokio::test]
async fn interacted_returns_signals_only() {
    let (store, a, b) = store_with_two().await;
    store.apply_action(a, Action::Viewed, now()).await.unwrap();
    store
        .apply_action(b, Action::Set(Flag::Liked), now())
        .await
        .unwrap();
    let items = store.interacted().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, b);
}

#[tokio::test]
async fn unknown_opportunity_is_an_error() {
    let (store, _, _) = store_with_two().await;
    assert!(matches!(
        store
            .apply_action(OpportunityId(77), Action::Viewed, now())
            .await,
        Err(StorageError::NotFound(77))
    ));
}

#[tokio::test]
async fn news_lives_apart_from_discover() {
    let store = Store::in_memory().await.unwrap();
    let mut news = opp(
        "https://news.example/1",
        "Linux 7.0 released",
        Category::Other,
    );
    news.content = sentri_core::ContentKind::News;
    let news = store.upsert_opportunity(&news).await.unwrap().id();
    let job = store
        .upsert_opportunity(&opp("https://jobs.example/1", "Job", Category::Job))
        .await
        .unwrap()
        .id();
    assert_eq!(ids(&store, FeedView::News).await, [news]);
    assert_eq!(ids(&store, FeedView::Discover).await, [job]);
    store
        .apply_action(news, Action::Set(Flag::Saved), now())
        .await
        .unwrap();
    assert_eq!(
        ids(&store, FeedView::Saved).await,
        [news],
        "personal lists keep both"
    );
}

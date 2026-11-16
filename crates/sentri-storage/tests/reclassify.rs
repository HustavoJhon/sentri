mod common;

use common::{now, opp};
use sentri_core::{
    Action, Annotations, Category, ContentKind, FeedQuery, FeedView, Field, Flag, Stance,
};
use sentri_storage::Store;

async fn news(store: &Store, url: &str, title: &str, stance: Stance) -> sentri_core::OpportunityId {
    let mut o = opp(url, title, Category::Other);
    o.content = ContentKind::News;
    o.stance = Some(stance);
    store.upsert_opportunity(&o).await.unwrap().id()
}

#[tokio::test]
async fn reclassifies_reversibly_and_keeps_user_data() {
    let store = Store::in_memory().await.unwrap();
    // Stored by the old heuristic as a rumor ("leaks").
    let leak = news(
        &store,
        "https://a.example/1",
        "App leaks user data of millions",
        Stance::Speculation,
    )
    .await;
    let edited = news(
        &store,
        "https://a.example/2",
        "Startup leaks roadmap to press today",
        Stance::Speculation,
    )
    .await;
    store
        .set_overrides(edited, &[(Field::Stance, "opinion".into())], now())
        .await
        .unwrap();
    store
        .apply_action(leak, Action::Set(Flag::Saved), now())
        .await
        .unwrap();
    store
        .set_annotations(
            leak,
            &Annotations {
                notes: Some("keep".into()),
                ..Annotations::default()
            },
            now(),
        )
        .await
        .unwrap();

    let dry = store
        .reclassify_news("b1", |_| None, false, now())
        .await
        .unwrap();
    assert_eq!(dry.len(), 1, "manual stance is skipped");
    assert_eq!(
        store.get(leak).await.unwrap().opportunity.stance,
        Some(Stance::Speculation),
        "dry run writes nothing"
    );

    let applied = store
        .reclassify_news("b1", |_| None, true, now())
        .await
        .unwrap();
    assert_eq!(applied[0].new, Stance::Report);
    let item = store.get(leak).await.unwrap();
    assert_eq!(item.opportunity.stance, Some(Stance::Report));
    assert!(item.state.saved, "likes and saves untouched");
    assert_eq!(
        store
            .edit_state(leak)
            .await
            .unwrap()
            .annotations
            .notes
            .as_deref(),
        Some("keep")
    );
    assert_eq!(
        store.get(edited).await.unwrap().opportunity.stance,
        Some(Stance::Opinion)
    );

    assert_eq!(store.undo_reclassification("b1").await.unwrap(), 1);
    assert_eq!(
        store.get(leak).await.unwrap().opportunity.stance,
        Some(Stance::Speculation)
    );
    assert!(
        store.undo_reclassification("b1").await.is_err(),
        "batch is gone"
    );
    assert_eq!(
        store
            .list(&FeedQuery::view(FeedView::Saved), now())
            .await
            .unwrap()
            .len(),
        1
    );
}

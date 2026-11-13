mod common;

use common::{now, opp};
use sentri_core::{Action, Annotations, Category, FeedQuery, FeedView, Field, Priority};
use sentri_storage::{StorageError, Store};

#[tokio::test]
async fn manual_edits_survive_new_collections_and_can_be_reset() {
    let store = Store::in_memory().await.unwrap();
    let id = store
        .upsert_opportunity(&opp("https://ig.example/p/1", "Post", Category::Other))
        .await
        .unwrap()
        .id();
    store
        .set_overrides(
            id,
            &[
                (Field::Title, "Beca en Japón".into()),
                (Field::Category, "scholarship".into()),
                (Field::AuthorHandle, "@becas".into()),
            ],
            now(),
        )
        .await
        .unwrap();

    // The source shows up again with different data.
    let mut again = opp("https://ig.example/p/1", "Instagram post", Category::Event);
    again.summary = Some("Full scholarship".into());
    store.upsert_opportunity(&again).await.unwrap();

    let state = store.edit_state(id).await.unwrap();
    assert_eq!(state.effective.title, "Beca en Japón");
    assert_eq!(state.effective.category, Category::Scholarship);
    assert_eq!(state.effective.summary.as_deref(), Some("Full scholarship"));
    assert_eq!(state.detected.title, "Post");
    assert_eq!(
        state.detected.category,
        Category::Event,
        "unknown completed"
    );
    assert!(state.is_overridden(Field::Title));

    // Queries see the effective (edited) values.
    let scholarships = store
        .list(&FeedQuery::view(FeedView::Scholarships), now())
        .await
        .unwrap();
    assert_eq!(scholarships.len(), 1);
    assert_eq!(
        scholarships[0].opportunity.author.handle.as_deref(),
        Some("@becas")
    );

    store.reset_overrides(id, &[Field::Title]).await.unwrap();
    assert_eq!(store.get(id).await.unwrap().opportunity.title, "Post");
    assert_eq!(
        store.get(id).await.unwrap().opportunity.category,
        Category::Scholarship
    );

    let history = store.history(5).await.unwrap();
    assert_eq!(history[0].action, Action::Edited);
}

#[tokio::test]
async fn invalid_edits_write_nothing() {
    let store = Store::in_memory().await.unwrap();
    let id = store
        .upsert_opportunity(&opp("https://a.example/", "A", Category::Course))
        .await
        .unwrap()
        .id();
    let result = store
        .set_overrides(
            id,
            &[
                (Field::Title, "B".into()),
                (Field::Deadline, "tomorrow".into()),
            ],
            now(),
        )
        .await;
    assert!(matches!(result, Err(StorageError::Invalid(_))));
    assert_eq!(store.get(id).await.unwrap().opportunity.title, "A");
    assert_eq!(store.edit_state(id).await.unwrap().overrides, Vec::new());
}

#[tokio::test]
async fn annotations_round_trip() {
    let store = Store::in_memory().await.unwrap();
    let id = store
        .upsert_opportunity(&opp("https://a.example/", "A", Category::Course))
        .await
        .unwrap()
        .id();
    let annotations = Annotations {
        notes: Some("Ask about the certificate".into()),
        priority: Some(Priority::High),
        custom: vec![
            ("mentor".into(), "Rosa".into()),
            (" ".into(), "dropped".into()),
        ],
    };
    store
        .set_annotations(id, &annotations, now())
        .await
        .unwrap();
    let stored = store.edit_state(id).await.unwrap().annotations;
    assert_eq!(stored.notes.as_deref(), Some("Ask about the certificate"));
    assert_eq!(stored.priority, Some(Priority::High));
    assert_eq!(
        stored.custom,
        vec![("mentor".to_owned(), "Rosa".to_owned())]
    );
}

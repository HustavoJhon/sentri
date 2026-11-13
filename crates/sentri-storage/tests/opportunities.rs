mod common;

use chrono::Duration;
use common::{now, opp};
use sentri_core::{
    Category, Cost, FeedQuery, FeedView, Modality, OpportunityId, PriceFilter, SortOrder,
    SourceKind,
};
use sentri_storage::{StorageError, Store, Upsert};

#[tokio::test]
async fn round_trips_every_field() {
    let store = Store::in_memory().await.unwrap();
    let mut o = opp("https://example.com/a", "Rust Course", Category::Course);
    o.summary = Some("Learn Rust".into());
    o.tags = vec!["rust".into(), "backend".into()];
    o.modality = Modality::Online;
    o.cost = Cost::Paid(Some("USD 20".into()));
    o.perks.certificate = Some(true);
    o.perks.travel_covered = Some(false);
    o.dates.deadline = Some(now() + Duration::days(3));
    o.external_id = Some("guid-1".into());
    o.author = sentri_core::Author {
        name: Some("Ana".into()),
        handle: Some("@ana".into()),
        profile_url: Some("https://social.example/@ana".into()),
        avatar_url: Some("https://social.example/ana.png".into()),
    };
    o.content = sentri_core::ContentKind::News;
    o.stance = Some(sentri_core::Stance::Opinion);

    let id = store.upsert_opportunity(&o).await.unwrap();
    assert!(id.is_new());
    let item = store.get(id.id()).await.unwrap();
    let mut stored = item.opportunity;
    stored.tags.sort();
    o.tags.sort();
    assert_eq!(stored, o);
    assert!(!item.state.liked);
}

#[tokio::test]
async fn deduplicates_by_canonical_url_across_sources() {
    let store = Store::in_memory().await.unwrap();
    let first = opp(
        "https://example.com/hack?utm_source=x",
        "Hack",
        Category::Other,
    );
    let a = store.upsert_opportunity(&first).await.unwrap();

    let mut second = opp(
        "http://www.example.com/hack/",
        "Hack (copy)",
        Category::Hackathon,
    );
    second.source = SourceKind::Manual;
    second.source_name = "manual".into();
    second.location = Some("Cusco".into());
    let b = store.upsert_opportunity(&second).await.unwrap();

    assert_eq!(b, Upsert::Merged(a.id()));
    assert_eq!(store.count().await.unwrap(), 1);
    let merged = store.get(a.id()).await.unwrap().opportunity;
    // Unknown fields are completed, known ones are kept.
    assert_eq!(merged.title, "Hack");
    assert_eq!(merged.category, Category::Hackathon);
    assert_eq!(merged.location.as_deref(), Some("Cusco"));
}

#[tokio::test]
async fn deduplicates_by_source_external_id() {
    let store = Store::in_memory().await.unwrap();
    let mut a = opp("https://example.com/v1", "Item", Category::Event);
    a.external_id = Some("42".into());
    let mut b = opp("https://example.com/v2", "Item moved", Category::Event);
    b.external_id = Some("42".into());
    let first = store.upsert_opportunity(&a).await.unwrap();
    assert_eq!(
        store.upsert_opportunity(&b).await.unwrap(),
        Upsert::Merged(first.id())
    );
}

#[tokio::test]
async fn update_replaces_editable_fields() {
    let store = Store::in_memory().await.unwrap();
    let o = opp("https://example.com/x", "Draft", Category::Other);
    let id = store.upsert_opportunity(&o).await.unwrap().id();
    let mut edited = o.clone();
    edited.title = "Final".into();
    edited.category = Category::Scholarship;
    edited.tags = vec!["travel".into()];
    store.update_opportunity(id, &edited).await.unwrap();
    let stored = store.get(id).await.unwrap().opportunity;
    assert_eq!(stored.title, "Final");
    assert_eq!(stored.tags, vec!["travel"]);

    assert!(matches!(
        store.update_opportunity(OpportunityId(999), &edited).await,
        Err(StorageError::NotFound(999))
    ));
}

async fn seeded() -> Store {
    let store = Store::in_memory().await.unwrap();
    let mut job = opp("https://jobs.example/1", "Backend Developer", Category::Job);
    job.modality = Modality::Online;
    job.dates.deadline = Some(now() + Duration::days(10));
    let mut course = opp("https://learn.example/rust", "Rust 100%", Category::Course);
    course.cost = Cost::Free;
    course.perks.certificate = Some(true);
    course.tags = vec!["rust".into()];
    let mut fellowship = opp(
        "https://fellow.example/",
        "Global Fellowship",
        Category::Fellowship,
    );
    fellowship.perks.travel_covered = Some(true);
    fellowship.location = Some("Berlin, Germany".into());
    fellowship.dates.deadline = Some(now() - Duration::days(1));
    for o in [job, course, fellowship] {
        store.upsert_opportunity(&o).await.unwrap();
    }
    store
}

async fn titles(store: &Store, q: FeedQuery) -> Vec<String> {
    store
        .list(&q, now())
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.opportunity.title)
        .collect()
}

#[tokio::test]
async fn filters_by_view_and_attributes() {
    let store = seeded().await;
    assert_eq!(
        titles(&store, FeedQuery::view(FeedView::Jobs)).await,
        ["Backend Developer"]
    );
    assert_eq!(
        titles(&store, FeedQuery::view(FeedView::Travel)).await,
        ["Global Fellowship"]
    );

    let free = FeedQuery {
        price: Some(PriceFilter::Free),
        with_certificate: true,
        ..FeedQuery::default()
    };
    assert_eq!(titles(&store, free).await, ["Rust 100%"]);

    let upcoming = FeedQuery {
        only_upcoming: true,
        ..FeedQuery::default()
    };
    assert_eq!(titles(&store, upcoming).await.len(), 2);

    let germany = FeedQuery {
        location: Some("germany".into()),
        ..FeedQuery::default()
    };
    assert_eq!(titles(&store, germany).await, ["Global Fellowship"]);
}

#[tokio::test]
async fn searches_text_tags_and_escapes_wildcards() {
    let store = seeded().await;
    let search = |text: &str| FeedQuery {
        text: Some(text.into()),
        ..FeedQuery::default()
    };
    assert_eq!(
        titles(&store, search("backend")).await,
        ["Backend Developer"]
    );
    assert_eq!(titles(&store, search("RUST")).await, ["Rust 100%"]);
    assert_eq!(titles(&store, search("100%")).await, ["Rust 100%"]);
    assert_eq!(titles(&store, search("%")).await, ["Rust 100%"]);
    assert_eq!(
        titles(&store, search("backend rust")).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn sorts_by_deadline_with_unknowns_last() {
    let store = seeded().await;
    let q = FeedQuery {
        sort: SortOrder::Deadline,
        ..FeedQuery::default()
    };
    assert_eq!(
        titles(&store, q).await,
        ["Global Fellowship", "Backend Developer", "Rust 100%"]
    );
}

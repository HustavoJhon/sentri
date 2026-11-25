//! The onboarding profile flows into the real feed ranking: location scope,
//! chosen interests and public GitHub languages, with explanations, and
//! without hiding anything.

use std::path::Path;

use chrono::Utc;
use sentri_app::{Config, Sentri};
use sentri_core::{CanonicalUrl, Category, FeedQuery, Opportunity, SourceKind};
use sentri_storage::Store;

fn opportunity(slug: &str, title: &str, category: Category, location: Option<&str>) -> Opportunity {
    let url = CanonicalUrl::parse(&format!("https://example.com/{slug}")).unwrap();
    let mut o = Opportunity::new(
        SourceKind::Manual,
        "test",
        url.as_str(),
        &url,
        title,
        Utc::now(),
    );
    o.category = category;
    o.location = location.map(str::to_owned);
    o
}

async fn app(config: &str, cache: &Path) -> Sentri {
    let mut config = Config::parse(config, Path::new("test")).unwrap();
    config.ui.cache_dir = Some(cache.join("images"));
    let store = Store::in_memory().await.unwrap();
    for o in [
        opportunity(
            "local",
            "Community night",
            Category::Meetup,
            Some("Arequipa, Perú"),
        ),
        opportunity(
            "abroad",
            "Summit",
            Category::Conference,
            Some("Berlin, Germany"),
        ),
        opportunity("hack", "Build weekend", Category::Hackathon, None),
        opportunity(
            "rust",
            "Contribute to a rust CLI",
            Category::OpenSource,
            None,
        ),
        opportunity("job", "Office manager", Category::Job, None),
    ] {
        store.upsert_opportunity(&o).await.unwrap();
    }
    Sentri::with_store(config, store, true).unwrap()
}

async fn ranked(app: &Sentri) -> Vec<(String, i32, Vec<String>)> {
    app.feed(&FeedQuery::default())
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            let reasons = r.relevance.reasons.iter().map(|x| x.text.clone()).collect();
            (r.item.opportunity.title, r.relevance.score, reasons)
        })
        .collect()
}

fn find<'a>(list: &'a [(String, i32, Vec<String>)], title: &str) -> &'a (String, i32, Vec<String>) {
    list.iter().find(|(t, _, _)| t == title).unwrap()
}

const PROFILE: &str = "[profile]\ncountry_code = \"PE\"\nhome_city = \"Arequipa\"\n";

#[tokio::test]
async fn location_and_interests_reorder_but_never_hide() {
    let dir = tempfile::tempdir().unwrap();
    let config = format!("{PROFILE}[discovery]\ntopics = [\"hackathons\"]\n");
    let list = ranked(&app(&config, dir.path()).await).await;
    assert_eq!(list.len(), 5, "nothing is filtered out");
    assert_eq!(list[0].0, "Community night", "your city first");
    assert!(
        find(&list, "Community night")
            .2
            .contains(&"Takes place in your city".to_owned())
    );
    let hack = find(&list, "Build weekend");
    assert!(
        hack.2
            .contains(&"Matches your interest in Hackathons".to_owned())
    );
    let abroad = find(&list, "Summit");
    assert!(abroad.1 > 0, "international stays visible and gets a bonus");
    assert!(abroad.2.iter().any(|r| r.starts_with("International")));
    assert_eq!(find(&list, "Office manager").1, 0);
}

#[tokio::test]
async fn switching_scopes_off_only_withholds_bonuses() {
    let dir = tempfile::tempdir().unwrap();
    let config =
        format!("{PROFILE}[discovery]\nlocal = false\nnational = false\ninternational = false\n");
    let list = ranked(&app(&config, dir.path()).await).await;
    assert_eq!(list.len(), 5);
    assert!(list.iter().all(|(_, score, _)| *score == 0), "{list:?}");
}

#[tokio::test]
async fn cached_github_languages_favor_matching_open_source() {
    let dir = tempfile::tempdir().unwrap();
    let config = format!("{PROFILE}[github]\nusername = \"octocat\"\n");
    let app = app(&config, dir.path()).await;
    let before = find(&ranked(&app).await, "Contribute to a rust CLI").1;
    // What `refresh_github_signals` stores after reading public repos.
    std::fs::write(
        dir.path().join("github-octocat-signals.json"),
        r#"{"languages":["rust","go"],"topics":["cli"]}"#,
    )
    .unwrap();
    let list = ranked(&app).await;
    let rust = find(&list, "Contribute to a rust CLI");
    assert!(rust.1 > before, "{before} -> {}", rust.1);
    assert!(
        rust.2
            .iter()
            .any(|r| r.contains("your GitHub languages: rust, go")),
        "{:?}",
        rust.2
    );
    assert_eq!(list[0].0, "Contribute to a rust CLI");
}

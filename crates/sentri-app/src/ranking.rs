//! Orders feed items by relevance and attaches the explanation.

use chrono::{DateTime, Utc};
use sentri_core::{
    Affinity, Discovery, FeedItem, FeedQuery, Profile, Relevance, Scorer, SortOrder, Weights,
};

/// A feed item with its relevance and the reasons behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    pub item: FeedItem,
    pub relevance: Relevance,
}

/// Scores every item, applies `min_score`, sorts (when the query asks for
/// relevance) and truncates to `limit`.
///
/// Low scores only move items down; they are never dropped unless the user
/// explicitly set `min_score`.
pub fn rank(
    items: Vec<FeedItem>,
    query: &FeedQuery,
    profile: &Profile,
    discovery: &Discovery,
    weights: &Weights,
    affinity: &Affinity,
    now: DateTime<Utc>,
) -> Vec<Ranked> {
    let scorer = Scorer {
        profile,
        discovery,
        weights,
        affinity,
        now,
    };
    let mut ranked: Vec<Ranked> = items
        .into_iter()
        .map(|item| Ranked {
            relevance: scorer.score(&item.opportunity),
            item,
        })
        .filter(|r| query.min_score.is_none_or(|min| r.relevance.score >= min))
        .collect();
    if query.sort == SortOrder::Relevance {
        // Stable sort keeps the newest-first order from storage among ties.
        ranked.sort_by_key(|r| std::cmp::Reverse(r.relevance.score));
    }
    if let Some(limit) = query.limit {
        ranked.truncate(limit as usize);
    }
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentri_core::{
        CanonicalUrl, Category, InteractionState, Interest, Opportunity, OpportunityId, Priority,
        SourceKind,
    };

    fn item(id: i64, title: &str) -> FeedItem {
        let url = CanonicalUrl::parse(&format!("https://x.example/{id}")).unwrap();
        let mut o = Opportunity::new(SourceKind::Rss, "f", url.as_str(), &url, title, Utc::now());
        o.category = Category::Other;
        FeedItem {
            id: OpportunityId(id),
            opportunity: o,
            state: InteractionState::default(),
        }
    }

    fn profile() -> Profile {
        Profile {
            interests: vec![Interest {
                name: "Rust".into(),
                keywords: vec!["rust".into()],
                categories: vec![],
                priority: Priority::High,
            }],
            ..Profile::default()
        }
    }

    #[test]
    fn sorts_by_score_and_keeps_low_scores() {
        let items = vec![
            item(1, "Cooking"),
            item(2, "Rust meetup"),
            item(3, "Gardening"),
        ];
        let ranked = rank(
            items,
            &FeedQuery::default(),
            &profile(),
            &Discovery::default(),
            &Weights::default(),
            &Affinity::default(),
            Utc::now(),
        );
        let ids: Vec<i64> = ranked.iter().map(|r| r.item.id.0).collect();
        assert_eq!(ids, [2, 1, 3]);
    }

    #[test]
    fn honors_min_score_limit_and_explicit_sort() {
        let items = vec![item(1, "Cooking"), item(2, "Rust meetup")];
        let query = FeedQuery {
            min_score: Some(1),
            ..FeedQuery::default()
        };
        let ranked = rank(
            items.clone(),
            &query,
            &profile(),
            &Discovery::default(),
            &Weights::default(),
            &Affinity::default(),
            Utc::now(),
        );
        assert_eq!(ranked.len(), 1);

        let newest = FeedQuery {
            sort: SortOrder::Newest,
            limit: Some(1),
            ..FeedQuery::default()
        };
        let ranked = rank(
            items,
            &newest,
            &profile(),
            &Discovery::default(),
            &Weights::default(),
            &Affinity::default(),
            Utc::now(),
        );
        assert_eq!(ranked[0].item.id.0, 1);
    }
}

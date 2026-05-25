use std::collections::HashMap;

use duty_core::Review;

const BOT_MARKERS: &[&str] = &["<!-- looper:", "Powered by <a", "[bot]"];

pub(crate) fn is_bot_authored(author: Option<&str>, body: &str) -> bool {
    if author
        .map(|login| login.to_ascii_lowercase().ends_with("[bot]"))
        .unwrap_or(false)
    {
        return true;
    }
    let lower = body.to_ascii_lowercase();
    BOT_MARKERS
        .iter()
        .any(|marker| lower.contains(&marker.to_ascii_lowercase()))
}

pub(crate) fn is_bot_only_approval(review_decision: &str, reviews: &[Review]) -> bool {
    if review_decision != "APPROVED" {
        return false;
    }
    let approved = reviews
        .iter()
        .filter(|review| review.state == "APPROVED")
        .collect::<Vec<_>>();
    !approved.is_empty()
        && approved
            .iter()
            .all(|review| is_bot_authored(review.author.as_deref(), review.body.as_str()))
}

pub(crate) fn latest_reviews_by_author(reviews: &[Review]) -> Vec<Review> {
    let mut by_author = HashMap::<String, Review>::new();
    for review in reviews {
        let Some(author) = &review.author else {
            continue;
        };
        let replace = by_author
            .get(author)
            .map(|existing| {
                existing.submitted_at.as_deref().unwrap_or("")
                    < review.submitted_at.as_deref().unwrap_or("")
            })
            .unwrap_or(true);
        if replace {
            by_author.insert(author.clone(), review.clone());
        }
    }
    by_author.into_values().collect()
}

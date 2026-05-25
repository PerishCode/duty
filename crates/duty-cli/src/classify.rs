use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use duty_core::{Comment, Commit, FactSnapshot, RateLimitSnapshot, Review};
use serde::Serialize;

use crate::{
    bot::{is_bot_authored, is_bot_only_approval, latest_reviews_by_author},
    cli::{ClassifyOptions, OutputFormat},
    lane::{derive_forbidden, derive_lane, Lane},
};

const AWAITING_THRESHOLD_HOURS: u64 = 24;
const KNOWN_TAGS: &[&str] = &[
    "bot-only-approval",
    "needs-rebase",
    "forbidden-surface",
    "unlabeled",
    "duplicate-title",
    "non-ascii-slug",
    "maintainer-edits-disabled",
    "org-member",
    "unresolved-changes-requested",
    "stale-approval",
    "awaiting-author-response-24h",
    "awaiting-reviewer-response-24h",
    "awaiting-first-review-24h",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tag {
    pub(crate) name: String,
    pub(crate) reason: String,
    pub(crate) source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) awaiting_hours: Option<u64>,
}

#[derive(Debug, Clone)]
struct PrFacts {
    number: u64,
    author: String,
    title: String,
    created_at: String,
    review_decision: String,
    merge_state_status: String,
    maintainer_can_modify: Option<bool>,
    is_org_member: bool,
    head_ref_oid: String,
    labels: Vec<String>,
    file_paths: Vec<String>,
    reviews: Vec<Review>,
    comments: Vec<Comment>,
    commits: Vec<Commit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClassifyReport {
    generated_at: String,
    open_pr_total: usize,
    classified_count: usize,
    by_tag: BTreeMap<String, Vec<u64>>,
    by_number: BTreeMap<String, Vec<Tag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate: Option<RateReport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateReport {
    pub(crate) before: RateLimitSnapshot,
    pub(crate) after: RateLimitSnapshot,
    pub(crate) cost: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ClassifyRunContext {
    pub(crate) org_members: HashSet<String>,
    pub(crate) rate: Option<RateReport>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug)]
struct ClassifyContext {
    title_index_by_author: HashMap<String, Vec<u64>>,
}

pub(crate) fn run_classify(
    snapshot: &FactSnapshot,
    options: &ClassifyOptions,
    run_context: &ClassifyRunContext,
) -> Result<(), String> {
    let facts = facts_from_snapshot(snapshot, &run_context.org_members);
    if options.all {
        let report = build_report(
            &facts,
            run_context.rate.clone(),
            run_context.warnings.clone(),
        );
        let path = write_report(&report, options.name.as_deref())?;
        let summary = report
            .by_tag
            .iter()
            .filter(|(_, nums)| !nums.is_empty())
            .map(|(name, nums)| format!("{name}={}", nums.len()))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "wrote {} entries to {}  [{}]{}{}",
            report.open_pr_total,
            path.display(),
            if summary.is_empty() {
                "no tags matched".to_string()
            } else {
                summary
            },
            rate_summary(report.rate.as_ref()),
            warning_summary(&report.warnings)
        );
        if options.print || options.queue.format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .map_err(|error| format!("failed to serialize classify report: {error}"))?
            );
        }
        return Ok(());
    }

    let Some(number) = options.number else {
        return Err("classify needs a PR number or --all".to_string());
    };
    let tags = tags_for_number_with_org_members(snapshot, number, &run_context.org_members)?;
    match options.queue.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "number": number,
                    "tags": tags,
                }))
                .map_err(|error| format!("failed to serialize classify tags: {error}"))?
            );
        }
        OutputFormat::Text => {
            println!("{}", format_single_pr(number, &tags));
        }
    }
    Ok(())
}

pub(crate) fn tags_for_number_with_org_members(
    snapshot: &FactSnapshot,
    number: u64,
    org_members: &HashSet<String>,
) -> Result<Vec<Tag>, String> {
    let facts = facts_from_snapshot(snapshot, org_members);
    let ctx = build_context(&facts);
    let facts = facts
        .into_iter()
        .find(|facts| facts.number == number)
        .ok_or_else(|| format!("PR #{number} was not present in the fetched facts snapshot"))?;
    Ok(classify_pr(&facts, &ctx))
}

fn facts_from_snapshot(snapshot: &FactSnapshot, org_members: &HashSet<String>) -> Vec<PrFacts> {
    let stats_by = snapshot
        .stats
        .iter()
        .map(|row| (row.number, row))
        .collect::<HashMap<_, _>>();
    let files_by = snapshot
        .files
        .iter()
        .map(|row| (row.number, row))
        .collect::<HashMap<_, _>>();
    let mut reviews_by = HashMap::<u64, Vec<Review>>::new();
    for review in &snapshot.reviews {
        reviews_by
            .entry(review.number)
            .or_default()
            .push(review.clone());
    }
    let mut comments_by = HashMap::<u64, Vec<Comment>>::new();
    for comment in &snapshot.comments {
        comments_by
            .entry(comment.number)
            .or_default()
            .push(comment.clone());
    }
    let mut commits_by = HashMap::<u64, Vec<Commit>>::new();
    for commit in &snapshot.commits {
        commits_by
            .entry(commit.number)
            .or_default()
            .push(commit.clone());
    }

    snapshot
        .meta
        .iter()
        .map(|meta| {
            let stats = stats_by.get(&meta.number);
            let file_paths = files_by
                .get(&meta.number)
                .map(|row| {
                    row.files
                        .iter()
                        .map(|file| file.path.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            PrFacts {
                number: meta.number,
                author: meta.author.clone().unwrap_or_default(),
                title: meta.title.clone(),
                created_at: meta.created_at.clone().unwrap_or_default(),
                review_decision: meta.review_decision.clone().unwrap_or_default(),
                merge_state_status: stats
                    .and_then(|row| row.merge_state_status.clone())
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
                maintainer_can_modify: meta.maintainer_can_modify,
                is_org_member: meta
                    .author
                    .as_ref()
                    .map(|author| org_members.contains(author))
                    .unwrap_or(false),
                head_ref_oid: stats
                    .and_then(|row| row.head_ref_oid.clone())
                    .unwrap_or_default(),
                labels: meta.labels.clone(),
                file_paths,
                reviews: latest_reviews_by_author(
                    &reviews_by.remove(&meta.number).unwrap_or_default(),
                ),
                comments: comments_by.remove(&meta.number).unwrap_or_default(),
                commits: commits_by.remove(&meta.number).unwrap_or_default(),
            }
        })
        .collect()
}

fn build_report(
    all_facts: &[PrFacts],
    rate: Option<RateReport>,
    warnings: Vec<String>,
) -> ClassifyReport {
    let ctx = build_context(all_facts);
    let mut by_tag = KNOWN_TAGS
        .iter()
        .map(|name| ((*name).to_string(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut by_number = BTreeMap::new();
    for facts in all_facts {
        let tags = classify_pr(facts, &ctx);
        for tag in &tags {
            by_tag
                .entry(tag.name.clone())
                .or_default()
                .push(facts.number);
        }
        by_number.insert(facts.number.to_string(), tags);
    }
    by_tag.retain(|_, nums| !nums.is_empty());
    ClassifyReport {
        generated_at: now_timestamp(),
        open_pr_total: all_facts.len(),
        classified_count: all_facts.len(),
        by_tag,
        by_number,
        rate,
        warnings,
    }
}

fn build_context(all_facts: &[PrFacts]) -> ClassifyContext {
    let mut title_index_by_author = HashMap::<String, Vec<u64>>::new();
    for facts in all_facts {
        title_index_by_author
            .entry(format!("{}\0{}", facts.author, facts.title))
            .or_default()
            .push(facts.number);
    }
    ClassifyContext {
        title_index_by_author,
    }
}

fn classify_pr(facts: &PrFacts, ctx: &ClassifyContext) -> Vec<Tag> {
    [
        tag_bot_only_approval(facts),
        tag_needs_rebase(facts),
        tag_forbidden_surface(facts),
        tag_unlabeled(facts),
        tag_duplicate_title(facts, ctx),
        tag_non_ascii_slug(facts),
        tag_maintainer_edits_disabled(facts),
        tag_org_member(facts),
        tag_unresolved_changes_requested(facts),
        tag_stale_approval(facts),
        tag_awaiting_author_response(facts),
        tag_awaiting_reviewer_response(facts),
        tag_awaiting_first_review(facts),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn tag_bot_only_approval(facts: &PrFacts) -> Option<Tag> {
    is_bot_only_approval(&facts.review_decision, &facts.reviews).then(|| Tag {
        name: "bot-only-approval".to_string(),
        reason: "reviewDecision=APPROVED; every APPROVED review is bot-authored".to_string(),
        source: "gh.reviewDecision+latestReviews".to_string(),
        awaiting_hours: None,
    })
}

fn tag_needs_rebase(facts: &PrFacts) -> Option<Tag> {
    (facts.merge_state_status == "DIRTY" || facts.merge_state_status == "BEHIND").then(|| Tag {
        name: "needs-rebase".to_string(),
        reason: format!("mergeStateStatus={}", facts.merge_state_status),
        source: "gh.mergeStateStatus".to_string(),
        awaiting_hours: None,
    })
}

fn tag_forbidden_surface(facts: &PrFacts) -> Option<Tag> {
    let hits = derive_forbidden(&facts.file_paths);
    (!hits.is_empty()).then(|| Tag {
        name: "forbidden-surface".to_string(),
        reason: format!(
            "path matches AGENTS.md forbidden surfaces: {}",
            hits.join(", ")
        ),
        source: "files+lane.deriveForbidden".to_string(),
        awaiting_hours: None,
    })
}

fn tag_unlabeled(facts: &PrFacts) -> Option<Tag> {
    let missing = ["size/", "risk/", "type/"]
        .into_iter()
        .filter(|prefix| !facts.labels.iter().any(|label| label.starts_with(prefix)))
        .collect::<Vec<_>>();
    (!missing.is_empty()).then(|| Tag {
        name: "unlabeled".to_string(),
        reason: format!("missing label prefixes: {}", missing.join(", ")),
        source: "gh.labels".to_string(),
        awaiting_hours: None,
    })
}

fn tag_duplicate_title(facts: &PrFacts, ctx: &ClassifyContext) -> Option<Tag> {
    let key = format!("{}\0{}", facts.author, facts.title);
    let siblings = ctx.title_index_by_author.get(&key)?;
    if siblings.len() < 2 {
        return None;
    }
    let others = siblings
        .iter()
        .copied()
        .filter(|number| *number != facts.number)
        .collect::<Vec<_>>();
    (!others.is_empty()).then(|| Tag {
        name: "duplicate-title".to_string(),
        reason: format!(
            "same author has another open PR(s) with byte-for-byte title: #{}",
            others
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", #")
        ),
        source: "cross-pr.titleIndexByAuthor".to_string(),
        awaiting_hours: None,
    })
}

fn tag_non_ascii_slug(facts: &PrFacts) -> Option<Tag> {
    let (lane, hits) = derive_lane(&facts.file_paths);
    if lane != Lane::DesignSystem && !hits.contains(&Lane::DesignSystem) {
        return None;
    }
    let offenders = facts
        .file_paths
        .iter()
        .filter_map(|path| {
            path.strip_prefix("design-systems/")
                .and_then(|rest| rest.split('/').next())
        })
        .filter(|slug| {
            !slug
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        })
        .map(str::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    (!offenders.is_empty()).then(|| Tag {
        name: "non-ascii-slug".to_string(),
        reason: format!(
            "design-system slug fails /^[a-z0-9-]+$/: {}",
            offenders.into_iter().collect::<Vec<_>>().join(", ")
        ),
        source: "files+lane.DESIGN_DIR".to_string(),
        awaiting_hours: None,
    })
}

fn tag_maintainer_edits_disabled(facts: &PrFacts) -> Option<Tag> {
    (facts.maintainer_can_modify == Some(false)).then(|| Tag {
        name: "maintainer-edits-disabled".to_string(),
        reason: "maintainerCanModify=false on the fork - maintainers cannot push to the PR branch"
            .to_string(),
        source: "gh.maintainerCanModify".to_string(),
        awaiting_hours: None,
    })
}

fn tag_org_member(facts: &PrFacts) -> Option<Tag> {
    facts.is_org_member.then(|| Tag {
        name: "org-member".to_string(),
        reason: format!("author {} is a member of the repo owner org", facts.author),
        source: "gh.api.orgs.members".to_string(),
        awaiting_hours: None,
    })
}

fn tag_unresolved_changes_requested(facts: &PrFacts) -> Option<Tag> {
    let reviewers = facts
        .reviews
        .iter()
        .filter(|review| review.state == "CHANGES_REQUESTED")
        .filter_map(|review| review.author.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if !reviewers.is_empty() {
        return Some(Tag {
            name: "unresolved-changes-requested".to_string(),
            reason: format!(
                "latestReviews carries CHANGES_REQUESTED from: {}",
                reviewers.into_iter().collect::<Vec<_>>().join(", ")
            ),
            source: "gh.latestReviews[].state".to_string(),
            awaiting_hours: None,
        });
    }
    (facts.review_decision == "CHANGES_REQUESTED").then(|| Tag {
        name: "unresolved-changes-requested".to_string(),
        reason: "reviewDecision=CHANGES_REQUESTED at PR level; no per-reviewer CHANGES_REQUESTED state in latest-per-author reduction of fetched reviews".to_string(),
        source: "gh.reviewDecision".to_string(),
        awaiting_hours: None,
    })
}

fn tag_stale_approval(facts: &PrFacts) -> Option<Tag> {
    if facts.head_ref_oid.is_empty() {
        return None;
    }
    let stale = facts
        .reviews
        .iter()
        .filter(|review| review.state == "APPROVED")
        .filter_map(|review| {
            let oid = review.commit_oid.as_deref()?;
            (!oid.is_empty() && oid != facts.head_ref_oid).then(|| {
                format!(
                    "{}@{}",
                    review.author.as_deref().unwrap_or("(unknown)"),
                    oid.chars().take(7).collect::<String>()
                )
            })
        })
        .collect::<Vec<_>>();
    (!stale.is_empty()).then(|| Tag {
        name: "stale-approval".to_string(),
        reason: format!(
            "APPROVED review(s) at {} predate current head {}",
            stale.join(", "),
            facts.head_ref_oid.chars().take(7).collect::<String>()
        ),
        source: "gh.latestReviews[].commit.oid+gh.headRefOid".to_string(),
        awaiting_hours: None,
    })
}

fn tag_awaiting_author_response(facts: &PrFacts) -> Option<Tag> {
    let reviewer = human_reviewer_signal_at(facts)?;
    let author = author_signal_at(facts)?;
    if reviewer <= author {
        return None;
    }
    let gap_hours = hours_since(reviewer)?;
    (gap_hours >= AWAITING_THRESHOLD_HOURS).then(|| Tag {
        name: "awaiting-author-response-24h".to_string(),
        reason: format!(
            "latest human-reviewer signal ({}) is {}h ago and newer than latest author signal ({})",
            iso_from_seconds(reviewer),
            gap_hours,
            iso_from_seconds(author)
        ),
        source: "latestReviews+comments+commits".to_string(),
        awaiting_hours: Some(gap_hours),
    })
}

fn tag_awaiting_reviewer_response(facts: &PrFacts) -> Option<Tag> {
    let reviewer = human_reviewer_signal_at(facts)?;
    let author = author_signal_at(facts)?;
    if author <= reviewer {
        return None;
    }
    let gap_hours = hours_since(author)?;
    (gap_hours >= AWAITING_THRESHOLD_HOURS).then(|| Tag {
        name: "awaiting-reviewer-response-24h".to_string(),
        reason: format!(
            "latest author signal ({}) is {}h ago and newer than latest human-reviewer signal ({})",
            iso_from_seconds(author),
            gap_hours,
            iso_from_seconds(reviewer)
        ),
        source: "latestReviews+comments+commits".to_string(),
        awaiting_hours: Some(gap_hours),
    })
}

fn tag_awaiting_first_review(facts: &PrFacts) -> Option<Tag> {
    if human_reviewer_signal_at(facts).is_some() {
        return None;
    }
    let created_at = parse_iso_seconds(&facts.created_at)?;
    let age_hours = hours_since(created_at)?;
    (age_hours >= AWAITING_THRESHOLD_HOURS).then(|| Tag {
        name: "awaiting-first-review-24h".to_string(),
        reason: format!(
            "no human review or non-author non-bot comment exists; createdAt is {age_hours}h ago"
        ),
        source: "latestReviews+comments+createdAt".to_string(),
        awaiting_hours: Some(age_hours),
    })
}

fn author_signal_at(facts: &PrFacts) -> Option<i64> {
    let mut max = None;
    for commit in &facts.commits {
        if commit.author_login.as_deref() != Some(facts.author.as_str()) {
            continue;
        }
        update_max(&mut max, commit.committed_date.as_deref());
    }
    for comment in &facts.comments {
        if comment.author.as_deref() != Some(facts.author.as_str()) {
            continue;
        }
        update_max(&mut max, comment.created_at.as_deref());
    }
    max
}

fn human_reviewer_signal_at(facts: &PrFacts) -> Option<i64> {
    let mut max = None;
    for review in &facts.reviews {
        let Some(author) = review.author.as_deref() else {
            continue;
        };
        if author == facts.author || is_bot_authored(Some(author), &review.body) {
            continue;
        }
        update_max(&mut max, review.submitted_at.as_deref());
    }
    for comment in &facts.comments {
        let Some(author) = comment.author.as_deref() else {
            continue;
        };
        if author == facts.author || is_bot_authored(Some(author), &comment.body) {
            continue;
        }
        update_max(&mut max, comment.created_at.as_deref());
    }
    max
}

fn update_max(max: &mut Option<i64>, iso: Option<&str>) {
    let Some(seconds) = iso.and_then(parse_iso_seconds) else {
        return;
    };
    if max.map(|existing| seconds > existing).unwrap_or(true) {
        *max = Some(seconds);
    }
}

fn hours_since(seconds: i64) -> Option<u64> {
    let now = now_seconds();
    (now >= seconds).then_some(((now - seconds) / 3600) as u64)
}

fn format_single_pr(number: u64, tags: &[Tag]) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "PR #{number} - {} tag{}",
        tags.len(),
        if tags.len() == 1 { "" } else { "s" }
    ));
    if tags.is_empty() {
        lines.push("  (no tags matched)".to_string());
    }
    for tag in tags {
        let suffix = tag
            .awaiting_hours
            .map(|hours| format!("  (awaiting {hours}h)"))
            .unwrap_or_default();
        lines.push(format!("  - {}{}", tag.name, suffix));
        lines.push(format!("      reason: {}", tag.reason));
        lines.push(format!("      source: {}", tag.source));
    }
    lines.join("\n")
}

fn rate_summary(rate: Option<&RateReport>) -> String {
    let Some(rate) = rate else {
        return String::new();
    };
    match rate.cost {
        Some(cost) => format!(
            "  rate cost={} remaining={}/{} reset={}",
            cost, rate.after.remaining, rate.after.limit, rate.after.reset_at
        ),
        None => format!(
            "  rate remaining={}/{} reset={} cost=N/A",
            rate.after.remaining, rate.after.limit, rate.after.reset_at
        ),
    }
}

fn warning_summary(warnings: &[String]) -> String {
    if warnings.is_empty() {
        String::new()
    } else {
        format!("  warnings={}", warnings.len())
    }
}

fn write_report(report: &ClassifyReport, name: Option<&str>) -> Result<PathBuf, String> {
    let dir = PathBuf::from(".tmp/duty/classify");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    let path = dir.join(format!(
        "{}.json",
        name.filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| timestamp_stem(&report.generated_at))
    ));
    let text = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize classify report: {error}"))?;
    fs::write(&path, format!("{text}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

fn now_timestamp() -> String {
    iso_from_seconds(now_seconds())
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_iso_seconds(iso: &str) -> Option<i64> {
    if iso.len() < 19 {
        return None;
    }
    let year = iso.get(0..4)?.parse::<i64>().ok()?;
    let month = iso.get(5..7)?.parse::<i64>().ok()?;
    let day = iso.get(8..10)?.parse::<i64>().ok()?;
    let hour = iso.get(11..13)?.parse::<i64>().ok()?;
    let minute = iso.get(14..16)?.parse::<i64>().ok()?;
    let second = iso.get(17..19)?.parse::<i64>().ok()?;
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

fn iso_from_seconds(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn timestamp_stem(iso: &str) -> String {
    if iso.len() >= 19 {
        return format!(
            "{}-{}-{}T{}{}{}Z",
            &iso[0..4],
            &iso[5..7],
            &iso[8..10],
            &iso[11..13],
            &iso[14..16],
            &iso[17..19]
        );
    }
    iso.replace([':', '-'], "")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month_prime + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month, day)
}

use std::collections::{HashMap, HashSet};

use duty_core::{FactSnapshot, PrMeta, Review};
use serde::Serialize;

use crate::{
    bot::{is_bot_only_approval, latest_reviews_by_author},
    cli::{OutputFormat, QueueOptions},
    lane::{derive_forbidden, derive_lane, Lane},
};

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Bucket {
    MergeReady,
    ApprovedBlocked,
    ChangesRequested,
    NeedsRebase,
    New,
    Draft,
    Stale,
}

impl Bucket {
    fn as_str(self) -> &'static str {
        match self {
            Bucket::MergeReady => "merge-ready",
            Bucket::ApprovedBlocked => "approved-blocked",
            Bucket::ChangesRequested => "changes-requested",
            Bucket::NeedsRebase => "needs-rebase",
            Bucket::New => "new",
            Bucket::Draft => "draft",
            Bucket::Stale => "stale",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "merge-ready" => Some(Bucket::MergeReady),
            "approved-blocked" => Some(Bucket::ApprovedBlocked),
            "changes-requested" => Some(Bucket::ChangesRequested),
            "needs-rebase" => Some(Bucket::NeedsRebase),
            "new" => Some(Bucket::New),
            "draft" => Some(Bucket::Draft),
            "stale" => Some(Bucket::Stale),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListedPr {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) age_days: u64,
    pub(crate) stale_days: u64,
    pub(crate) is_draft: bool,
    pub(crate) review_decision: String,
    pub(crate) merge_state_status: String,
    pub(crate) size: Option<String>,
    pub(crate) risk: Option<String>,
    pub(crate) pr_type: Option<String>,
    pub(crate) changed_files: u64,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) head_ref_name: String,
    pub(crate) base_ref_name: String,
    pub(crate) lane: Lane,
    pub(crate) lane_hits: Vec<Lane>,
    pub(crate) forbidden: Vec<String>,
    pub(crate) bucket: Bucket,
    pub(crate) bot_only_approval: bool,
}

pub(crate) fn classify_list(snapshot: &FactSnapshot) -> Vec<ListedPr> {
    let stats_by_num = snapshot
        .stats
        .iter()
        .map(|row| (row.number, row))
        .collect::<HashMap<_, _>>();
    let files_by_num = snapshot
        .files
        .iter()
        .map(|row| (row.number, row))
        .collect::<HashMap<_, _>>();
    let mut reviews_by_num = HashMap::<u64, Vec<Review>>::new();
    for review in &snapshot.reviews {
        reviews_by_num
            .entry(review.number)
            .or_default()
            .push(review.clone());
    }

    let now_days = current_epoch_days();
    snapshot
        .meta
        .iter()
        .map(|meta| {
            let stats = stats_by_num.get(&meta.number);
            let paths = files_by_num
                .get(&meta.number)
                .map(|row| {
                    row.files
                        .iter()
                        .map(|file| file.path.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let (lane, lane_hits) = derive_lane(&paths);
            let review_decision = meta.review_decision.clone().unwrap_or_default();
            let merge_state_status = stats
                .and_then(|row| row.merge_state_status.clone())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let age_days = meta
                .created_at
                .as_deref()
                .map(|created| days_since(created, now_days))
                .unwrap_or(0);
            let stale_days = meta
                .updated_at
                .as_deref()
                .map(|updated| days_since(updated, now_days))
                .unwrap_or(0);
            let reviews = reviews_by_num.remove(&meta.number).unwrap_or_default();
            let latest = latest_reviews_by_author(&reviews);
            ListedPr {
                number: meta.number,
                title: meta.title.clone(),
                author: meta.author.clone().unwrap_or_else(|| "-".to_string()),
                age_days,
                stale_days,
                is_draft: meta.is_draft.unwrap_or(false),
                review_decision: review_decision.clone(),
                merge_state_status: merge_state_status.clone(),
                size: label_by_prefix(meta, "size/"),
                risk: label_by_prefix(meta, "risk/"),
                pr_type: label_by_prefix(meta, "type/"),
                changed_files: stats.and_then(|row| row.changed_files).unwrap_or(0),
                additions: stats.and_then(|row| row.additions).unwrap_or(0),
                deletions: stats.and_then(|row| row.deletions).unwrap_or(0),
                head_ref_name: stats
                    .and_then(|row| row.head_ref_name.clone())
                    .or_else(|| meta.head_ref_name.clone())
                    .unwrap_or_default(),
                base_ref_name: stats
                    .and_then(|row| row.base_ref_name.clone())
                    .unwrap_or_else(|| "main".to_string()),
                lane,
                lane_hits,
                forbidden: derive_forbidden(&paths),
                bucket: derive_bucket(
                    meta.is_draft.unwrap_or(false),
                    &review_decision,
                    &merge_state_status,
                    stale_days,
                ),
                bot_only_approval: is_bot_only_approval(&review_decision, &latest),
            }
        })
        .collect()
}

pub(crate) fn apply_filters(prs: &[ListedPr], options: &QueueOptions) -> Vec<ListedPr> {
    let lanes = parse_filter_set(options.lane.as_deref(), Lane::parse);
    let buckets = parse_filter_set(options.bucket.as_deref(), Bucket::parse);
    let authors = options
        .author
        .as_deref()
        .map(|raw| raw.split(',').map(str::to_string).collect::<HashSet<_>>());

    prs.iter()
        .filter(|pr| {
            (options.include_drafts || !pr.is_draft)
                && lanes
                    .as_ref()
                    .map(|lanes| lanes.contains(&pr.lane))
                    .unwrap_or(true)
                && buckets
                    .as_ref()
                    .map(|buckets| buckets.contains(&pr.bucket))
                    .unwrap_or(true)
                && authors
                    .as_ref()
                    .map(|authors| authors.contains(&pr.author))
                    .unwrap_or(true)
        })
        .cloned()
        .collect()
}

pub(crate) fn print_list(
    prs: &[ListedPr],
    total: usize,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(prs)
                .map_err(|error| format!("failed to serialize list JSON: {error}"))?;
            println!("{text}");
        }
        OutputFormat::Text => {
            println!("{}", format_human_report(prs, total));
        }
    }
    Ok(())
}

fn derive_bucket(
    is_draft: bool,
    review_decision: &str,
    merge_state_status: &str,
    stale_days: u64,
) -> Bucket {
    if is_draft {
        return Bucket::Draft;
    }
    if review_decision == "APPROVED" {
        if merge_state_status == "CLEAN" || merge_state_status == "UNSTABLE" {
            return Bucket::MergeReady;
        }
        return Bucket::ApprovedBlocked;
    }
    if merge_state_status == "DIRTY" || merge_state_status == "BEHIND" {
        return Bucket::NeedsRebase;
    }
    if review_decision == "CHANGES_REQUESTED" {
        return Bucket::ChangesRequested;
    }
    if stale_days > 14 {
        return Bucket::Stale;
    }
    Bucket::New
}

fn label_by_prefix(meta: &PrMeta, prefix: &str) -> Option<String> {
    meta.labels
        .iter()
        .find_map(|label| label.strip_prefix(prefix).map(str::to_string))
}

fn parse_filter_set<T>(raw: Option<&str>, parse: impl Fn(&str) -> Option<T>) -> Option<HashSet<T>>
where
    T: Eq + std::hash::Hash,
{
    raw.map(|raw| raw.split(',').filter_map(parse).collect())
}

fn format_human_report(prs: &[ListedPr], total: usize) -> String {
    let mut by_bucket = HashMap::<Bucket, Vec<ListedPr>>::new();
    for pr in prs {
        by_bucket.entry(pr.bucket).or_default().push(pr.clone());
    }

    let mut lines = Vec::new();
    if prs.len() == total {
        lines.push(format!("duty PR triage - {total} open PRs"));
    } else {
        lines.push(format!(
            "duty PR triage - showing {} of {total} open PRs",
            prs.len()
        ));
    }
    lines.push(String::new());

    for bucket in bucket_order() {
        let Some(mut rows) = by_bucket.remove(&bucket) else {
            continue;
        };
        if rows.is_empty() {
            continue;
        }
        rows.sort_by(|a, b| {
            a.lane
                .order()
                .cmp(&b.lane.order())
                .then(a.stale_days.cmp(&b.stale_days))
        });
        lines.push(format!("| {}  ({})", bucket.as_str(), rows.len()));
        for pr in rows {
            lines.push(format_human_row(&pr));
        }
        lines.push(String::new());
    }

    let forbidden = prs
        .iter()
        .filter(|pr| !pr.forbidden.is_empty())
        .collect::<Vec<_>>();
    if !forbidden.is_empty() {
        lines.push(format!("| forbidden-surface hits  ({})", forbidden.len()));
        for pr in forbidden {
            lines.push(format!(
                "  #{}  {}  {}",
                pr.number,
                pr.forbidden.join(", "),
                truncate(&pr.title, 60)
            ));
        }
        lines.push(String::new());
    }

    let bot_only = prs
        .iter()
        .filter(|pr| pr.bot_only_approval)
        .collect::<Vec<_>>();
    if !bot_only.is_empty() {
        lines.push(format!("| bot-only approval  ({})", bot_only.len()));
        lines.push("  reviewDecision=APPROVED, but every APPROVED review is bot-authored".into());
        for pr in bot_only {
            lines.push(format!("  #{}  {}", pr.number, truncate(&pr.title, 70)));
        }
        lines.push(String::new());
    }

    lines.push("legend: age = created/updated days ago   lane = derived from touched paths".into());
    lines.push("        risk / sz / t = gh label values (size/, risk/, type/ prefixes)".into());
    lines.push("        forbid:N = N path matches against forbidden surfaces".into());
    lines.push("        bot-only = APPROVED with only bot-authored approvals".into());
    lines.join("\n")
}

fn bucket_order() -> [Bucket; 7] {
    [
        Bucket::MergeReady,
        Bucket::ApprovedBlocked,
        Bucket::NeedsRebase,
        Bucket::ChangesRequested,
        Bucket::New,
        Bucket::Stale,
        Bucket::Draft,
    ]
}

fn format_human_row(pr: &ListedPr) -> String {
    let flags = [
        pr.risk.as_ref().map(|risk| format!("risk:{}", &risk[0..1])),
        pr.size.as_ref().map(|size| format!("sz:{size}")),
        pr.pr_type.as_ref().map(|kind| format!("t:{kind}")),
        (!pr.forbidden.is_empty()).then(|| format!("forbid:{}", pr.forbidden.len())),
        pr.bot_only_approval.then(|| "bot-only".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    let age = format!("{:>2}d/{:>2}d", pr.age_days, pr.stale_days);
    format!(
        "  #{:<5}  {:<8}  {}  {:<16}  {:<34}  {}",
        pr.number,
        pr.lane.tag(),
        age,
        truncate(&pr.author, 16),
        flags,
        truncate(&pr.title, 64)
    )
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    out.push('~');
    out
}

fn current_epoch_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

fn days_since(iso: &str, now_days: i64) -> u64 {
    parse_yyyy_mm_dd(iso)
        .map(|(year, month, day)| {
            let days = days_from_civil(year, month, day);
            now_days.saturating_sub(days).max(0) as u64
        })
        .unwrap_or(0)
}

fn parse_yyyy_mm_dd(iso: &str) -> Option<(i64, i64, i64)> {
    if iso.len() < 10 {
        return None;
    }
    let year = iso.get(0..4)?.parse().ok()?;
    let month = iso.get(5..7)?.parse().ok()?;
    let day = iso.get(8..10)?.parse().ok()?;
    Some((year, month, day))
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

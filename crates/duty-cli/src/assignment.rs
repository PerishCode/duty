use std::collections::{BTreeMap, HashMap, HashSet};

use duty_core::{AssignmentEvent, Comment, Commit, FactSnapshot, Review};
use serde::Serialize;

use crate::{
    classify::{tags_by_number, Tag},
    cli::{AssignmentOptions, OutputFormat},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssignmentEntry {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) assignee: String,
    pub(crate) assigned_at: Option<String>,
    pub(crate) assigned_by: Option<String>,
    pub(crate) self_assigned: bool,
    pub(crate) assigned_hours_ago: Option<u64>,
    pub(crate) idle_since_at: Option<String>,
    pub(crate) idle_hours: Option<u64>,
    pub(crate) state_badges: Vec<String>,
    pub(crate) status: String,
    pub(crate) blockers: Vec<String>,
    pub(crate) tags: Vec<Tag>,
    pub(crate) is_draft: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnassignedEntry {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) state_badges: Vec<String>,
    pub(crate) status: String,
    pub(crate) blockers: Vec<String>,
    pub(crate) is_draft: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssignmentReport {
    pub(crate) generated_at: String,
    pub(crate) open_pr_total: usize,
    pub(crate) assigned_count: usize,
    pub(crate) unassigned_count: usize,
    pub(crate) by_assignee: BTreeMap<String, Vec<AssignmentEntry>>,
    pub(crate) unassigned: Vec<UnassignedEntry>,
}

#[derive(Debug, Clone)]
struct AssignmentFacts {
    number: u64,
    title: String,
    review_decision: String,
    merge_state_status: String,
    assignees: Vec<String>,
    is_draft: bool,
    comments: Vec<Comment>,
    commits: Vec<Commit>,
    reviews: Vec<Review>,
}

pub(crate) fn run_assignment(
    snapshot: &FactSnapshot,
    options: &AssignmentOptions,
    org_members: &HashSet<String>,
    me_login: Option<String>,
) -> Result<(), String> {
    let report = build_report(snapshot, options.queue.include_drafts, org_members)?;
    if options.queue.format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("failed to serialize assignment JSON: {error}"))?
        );
        return Ok(());
    }

    let user_filter = match options.user.as_deref() {
        Some("me") => me_login.clone(),
        Some(login) => Some(login.to_string()),
        None => None,
    };
    println!(
        "{}",
        format_human_report(
            &report,
            HumanOptions {
                me_login: me_login.as_deref(),
                user_filter: user_filter.as_deref(),
                show_unassigned_detail: options.unassigned,
            },
        )
    );
    Ok(())
}

pub(crate) fn build_report(
    snapshot: &FactSnapshot,
    include_drafts: bool,
    org_members: &HashSet<String>,
) -> Result<AssignmentReport, String> {
    let facts = assignment_facts(snapshot, include_drafts);
    let tags_by = tags_by_number(snapshot, org_members)?;
    let events_by = assignment_events_by_number(snapshot);
    let now = now_seconds();
    let mut assigned_count = 0usize;
    let mut by_assignee = BTreeMap::<String, Vec<AssignmentEntry>>::new();
    let mut unassigned = Vec::<UnassignedEntry>::new();

    for facts in &facts {
        let tags = tags_by.get(&facts.number).cloned().unwrap_or_default();
        let events = events_by.get(&facts.number).cloned().unwrap_or_default();
        if facts.assignees.is_empty() {
            let (status, blockers) = derive_status(&tags, facts);
            unassigned.push(UnassignedEntry {
                number: facts.number,
                title: facts.title.clone(),
                state_badges: derive_state_badges(facts),
                status,
                blockers,
                is_draft: facts.is_draft,
            });
            continue;
        }

        assigned_count += 1;
        for entry in build_assignment_entries(facts, &events, &tags, now) {
            by_assignee
                .entry(entry.assignee.clone())
                .or_default()
                .push(entry);
        }
    }

    for rows in by_assignee.values_mut() {
        rows.sort_by(|a, b| {
            b.idle_hours
                .unwrap_or(0)
                .cmp(&a.idle_hours.unwrap_or(0))
                .then(a.number.cmp(&b.number))
        });
    }
    unassigned.sort_by(|a, b| a.number.cmp(&b.number));

    Ok(AssignmentReport {
        generated_at: iso_from_seconds(now),
        open_pr_total: facts.len(),
        assigned_count,
        unassigned_count: unassigned.len(),
        by_assignee,
        unassigned,
    })
}

fn assignment_facts(snapshot: &FactSnapshot, include_drafts: bool) -> Vec<AssignmentFacts> {
    let stats_by = snapshot
        .stats
        .iter()
        .map(|row| (row.number, row))
        .collect::<HashMap<_, _>>();
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
    let mut reviews_by = HashMap::<u64, Vec<Review>>::new();
    for review in &snapshot.reviews {
        reviews_by
            .entry(review.number)
            .or_default()
            .push(review.clone());
    }

    snapshot
        .meta
        .iter()
        .filter(|meta| include_drafts || !meta.is_draft.unwrap_or(false))
        .map(|meta| AssignmentFacts {
            number: meta.number,
            title: meta.title.clone(),
            review_decision: meta
                .review_decision
                .clone()
                .unwrap_or_else(|| "REVIEW_REQUIRED".to_string()),
            merge_state_status: stats_by
                .get(&meta.number)
                .and_then(|row| row.merge_state_status.clone())
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            assignees: meta.assignees.clone(),
            is_draft: meta.is_draft.unwrap_or(false),
            comments: comments_by.remove(&meta.number).unwrap_or_default(),
            commits: commits_by.remove(&meta.number).unwrap_or_default(),
            reviews: reviews_by.remove(&meta.number).unwrap_or_default(),
        })
        .collect()
}

fn assignment_events_by_number(snapshot: &FactSnapshot) -> HashMap<u64, Vec<AssignmentEvent>> {
    let mut events_by = HashMap::<u64, Vec<AssignmentEvent>>::new();
    for event in &snapshot.assignment_events {
        events_by
            .entry(event.number)
            .or_default()
            .push(event.clone());
    }
    events_by
}

fn build_assignment_entries(
    facts: &AssignmentFacts,
    events: &[AssignmentEvent],
    tags: &[Tag],
    now: i64,
) -> Vec<AssignmentEntry> {
    let event_index = index_assignment_events(events);
    let state_badges = derive_state_badges(facts);
    let (status, blockers) = derive_status(tags, facts);

    facts
        .assignees
        .iter()
        .map(|assignee| {
            let event = event_index.get(assignee);
            let assigned_at = event.and_then(|event| event.created_at.clone());
            let assigned_by = event.and_then(|event| event.actor.clone());
            let assigned_seconds = assigned_at.as_deref().and_then(parse_iso_seconds);
            let assigned_hours_ago =
                assigned_seconds.and_then(|seconds| hours_between(seconds, now));
            let last_activity_at = last_assignee_activity_at(facts, assignee);
            let idle_since_at = max_iso(assigned_at.as_deref(), last_activity_at.as_deref());
            let idle_hours = idle_since_at
                .as_deref()
                .and_then(parse_iso_seconds)
                .and_then(|seconds| hours_between(seconds, now));
            AssignmentEntry {
                number: facts.number,
                title: facts.title.clone(),
                assignee: assignee.clone(),
                assigned_at,
                assigned_by: assigned_by.clone(),
                self_assigned: assigned_by.as_deref() == Some(assignee.as_str()),
                assigned_hours_ago,
                idle_since_at,
                idle_hours,
                state_badges: state_badges.clone(),
                status: status.clone(),
                blockers: blockers.clone(),
                tags: tags.to_vec(),
                is_draft: facts.is_draft,
            }
        })
        .collect()
}

fn index_assignment_events(events: &[AssignmentEvent]) -> HashMap<String, AssignmentEvent> {
    let mut sorted = events.to_vec();
    sorted.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let mut map = HashMap::new();
    for event in sorted {
        let Some(assignee) = &event.assignee else {
            continue;
        };
        if event.kind == "ASSIGNED" {
            map.insert(assignee.clone(), event);
        } else if event.kind == "UNASSIGNED" {
            map.remove(assignee);
        }
    }
    map
}

fn last_assignee_activity_at(facts: &AssignmentFacts, assignee: &str) -> Option<String> {
    let mut latest = None::<String>;
    for commit in &facts.commits {
        if commit.author_login.as_deref() == Some(assignee) {
            update_latest(&mut latest, commit.committed_date.as_deref());
        }
    }
    for comment in &facts.comments {
        if comment.author.as_deref() == Some(assignee) {
            update_latest(&mut latest, comment.created_at.as_deref());
        }
    }
    for review in &facts.reviews {
        if review.author.as_deref() == Some(assignee) {
            update_latest(&mut latest, review.submitted_at.as_deref());
        }
    }
    latest
}

fn update_latest(current: &mut Option<String>, candidate: Option<&str>) {
    let Some(candidate) = candidate else {
        return;
    };
    if current
        .as_deref()
        .map(|existing| candidate > existing)
        .unwrap_or(true)
    {
        *current = Some(candidate.to_string());
    }
}

fn max_iso(left: Option<&str>, right: Option<&str>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left > right { left } else { right }.to_string()),
        (Some(left), None) => Some(left.to_string()),
        (None, Some(right)) => Some(right.to_string()),
        (None, None) => None,
    }
}

fn derive_state_badges(facts: &AssignmentFacts) -> Vec<String> {
    let mut badges = Vec::new();
    badges.push(if facts.review_decision.is_empty() {
        "REVIEW_REQUIRED".to_string()
    } else {
        facts.review_decision.clone()
    });
    if facts.merge_state_status != "CLEAN" && facts.merge_state_status != "UNKNOWN" {
        badges.push(facts.merge_state_status.clone());
    }
    if facts.is_draft {
        badges.push("draft".to_string());
    }
    badges
}

fn derive_status(tags: &[Tag], facts: &AssignmentFacts) -> (String, Vec<String>) {
    let by_name = tags
        .iter()
        .map(|tag| (tag.name.as_str(), tag))
        .collect::<HashMap<_, _>>();
    let mut blockers = Vec::new();
    if by_name.contains_key("needs-rebase") {
        blockers.push("needs-rebase (main has moved)".to_string());
    }
    for name in ["unresolved-changes-requested", "stale-approval"] {
        if let Some(tag) = by_name.get(name) {
            blockers.push(tag.reason.clone());
        }
    }
    if let Some(tag) = by_name.get("awaiting-author-response-24h") {
        blockers.push(format!(
            "awaiting author for {}",
            format_duration(tag.awaiting_hours)
        ));
    }
    if let Some(tag) = by_name.get("awaiting-reviewer-response-24h") {
        blockers.push(format!(
            "awaiting reviewer for {}",
            format_duration(tag.awaiting_hours)
        ));
    }
    if let Some(tag) = by_name.get("awaiting-first-review-24h") {
        blockers.push(format!(
            "no human review yet ({} since createdAt)",
            format_duration(tag.awaiting_hours)
        ));
    }

    let merge_ready = facts.review_decision == "APPROVED"
        && (facts.merge_state_status == "CLEAN" || facts.merge_state_status == "UNSTABLE");
    let status = if !blockers.is_empty() {
        "blocked"
    } else if by_name.contains_key("bot-only-approval") {
        "approved (bot-only - no human formal sign-off)"
    } else if merge_ready {
        "ready to merge"
    } else {
        "in review"
    };
    (status.to_string(), blockers)
}

struct HumanOptions<'a> {
    me_login: Option<&'a str>,
    user_filter: Option<&'a str>,
    show_unassigned_detail: bool,
}

fn format_human_report(report: &AssignmentReport, options: HumanOptions<'_>) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "PR assignment overview - {} open PRs - {} assigned - {} unassigned",
        report.open_pr_total, report.assigned_count, report.unassigned_count
    ));
    lines.push(String::new());

    let mut buckets = report.by_assignee.iter().collect::<Vec<_>>();
    buckets.sort_by(|a, b| {
        if options.me_login == Some(a.0.as_str()) {
            return std::cmp::Ordering::Less;
        }
        if options.me_login == Some(b.0.as_str()) {
            return std::cmp::Ordering::Greater;
        }
        b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0))
    });

    for (login, entries) in buckets {
        if options
            .user_filter
            .map(|filter| filter != login.as_str())
            .unwrap_or(false)
        {
            continue;
        }
        let you = if options.me_login == Some(login.as_str()) {
            " (you)"
        } else {
            ""
        };
        lines.push(format!(
            "| {}{} - {} PR{}",
            login,
            you,
            entries.len(),
            if entries.len() == 1 { "" } else { "s" }
        ));
        lines.push(String::new());
        for entry in entries {
            lines.push(format!(
                "  #{:>5}  {}{}",
                entry.number,
                truncate(&entry.title, 64),
                if entry.is_draft { " [draft]" } else { "" }
            ));
            let assigned = entry
                .assigned_hours_ago
                .map(|hours| format!("assigned {} ago", format_duration(Some(hours))))
                .unwrap_or_else(|| "assignment event older than fetched window".to_string());
            let by = entry
                .assigned_by
                .as_ref()
                .map(|login| {
                    if entry.self_assigned {
                        " (self-assigned)".to_string()
                    } else {
                        format!(" by {login}")
                    }
                })
                .unwrap_or_default();
            let idle = entry
                .idle_hours
                .map(|hours| format!(" - idle {}", format_duration(Some(hours))))
                .unwrap_or_default();
            lines.push(format!("         {assigned}{by}{idle}"));
            lines.push(format!(
                "         state: {}",
                entry.state_badges.join(" / ")
            ));
            lines.push(format!("         status: {}", entry.status));
            for blocker in &entry.blockers {
                lines.push(format!("           - {blocker}"));
            }
            lines.push(String::new());
        }
    }

    if options.user_filter.is_none() {
        if options.show_unassigned_detail {
            lines.push(format!("| (unassigned) - {} PRs", report.unassigned.len()));
            lines.push(String::new());
            for entry in &report.unassigned {
                lines.push(format!(
                    "  #{:>5}  {}{}",
                    entry.number,
                    truncate(&entry.title, 64),
                    if entry.is_draft { " [draft]" } else { "" }
                ));
                lines.push(format!(
                    "         state: {}  status: {}",
                    entry.state_badges.join(" / "),
                    entry.status
                ));
                for blocker in &entry.blockers {
                    lines.push(format!("           - {blocker}"));
                }
            }
        } else {
            lines.push(format!("| (unassigned) - {} PRs", report.unassigned_count));
            lines.push("    see `duty assignment --unassigned` for the full list".to_string());
        }
    }

    lines.join("\n")
}

fn format_duration(hours: Option<u64>) -> String {
    let Some(hours) = hours else {
        return "(unknown)".to_string();
    };
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    let rem = hours - days * 24;
    if rem == 0 {
        format!("{days}d")
    } else {
        format!("{days}d {rem}h")
    }
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

fn hours_between(from_seconds: i64, now_seconds: i64) -> Option<u64> {
    (now_seconds >= from_seconds).then_some(((now_seconds - from_seconds) / 3600) as u64)
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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

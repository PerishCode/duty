use std::{
    collections::HashSet,
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use duty_core::{
    parse_plain_pr_list, AssignmentEvent, Comment, Commit, FactSnapshot, FileChange,
    OpenPullRequest, PrFiles, PrMeta, PrStats, QueueSnapshot, Review, SnapshotSource,
};
use serde::{de::DeserializeOwned, Deserialize};
use tracing::debug;

const PR_LIST_PAGE_SIZE: usize = 30;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonPrMeta {
    number: u64,
    title: String,
    author: Option<JsonLogin>,
    created_at: Option<String>,
    updated_at: Option<String>,
    is_draft: Option<bool>,
    review_decision: Option<String>,
    labels: Option<Vec<JsonName>>,
    maintainer_can_modify: Option<bool>,
    assignees: Option<Vec<JsonLogin>>,
    head_ref_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JsonLogin {
    login: String,
}

#[derive(Debug, Deserialize)]
struct JsonName {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonPrFiles {
    number: u64,
    files: Vec<JsonFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonFile {
    path: String,
    additions: Option<u64>,
    deletions: Option<u64>,
    change_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlPage<N> {
    data: GraphqlData<N>,
}

#[derive(Debug, Deserialize)]
struct GraphqlData<N> {
    repository: GraphqlRepository<N>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRepository<N> {
    #[serde(rename = "pullRequests")]
    pull_requests: GraphqlConnection<N>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlConnection<N> {
    nodes: Vec<N>,
    page_info: GraphqlPageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlStats {
    number: u64,
    additions: Option<u64>,
    deletions: Option<u64>,
    changed_files: Option<u64>,
    head_ref_name: Option<String>,
    head_ref_oid: Option<String>,
    base_ref_name: Option<String>,
    mergeable: Option<String>,
    merge_state_status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlReviewNode {
    number: u64,
    reviews: GraphqlNodes<GraphqlReview>,
}

#[derive(Debug, Deserialize)]
struct GraphqlNodes<N> {
    nodes: Vec<N>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlReview {
    author: Option<JsonLogin>,
    body: Option<String>,
    state: String,
    submitted_at: Option<String>,
    commit: Option<GraphqlCommitOid>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCommitOid {
    oid: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlCommitNode {
    number: u64,
    commits: GraphqlNodes<GraphqlCommitEntry>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCommitEntry {
    commit: GraphqlCommit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlCommit {
    oid: Option<String>,
    committed_date: Option<String>,
    author: Option<GraphqlCommitAuthor>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCommitAuthor {
    user: Option<JsonLogin>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlCommentNode {
    number: u64,
    comments: GraphqlNodes<GraphqlComment>,
    review_threads: GraphqlNodes<GraphqlReviewThread>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlComment {
    author: Option<JsonLogin>,
    body: Option<String>,
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlReviewThread {
    comments: GraphqlNodes<GraphqlComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlAssignmentNode {
    number: u64,
    timeline_items: GraphqlNodes<GraphqlAssignmentItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlAssignmentItem {
    #[serde(rename = "__typename")]
    typename: String,
    created_at: Option<String>,
    actor: Option<JsonLogin>,
    assignee: Option<GraphqlAssignee>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlAssignee {
    #[serde(rename = "__typename")]
    typename: String,
    login: Option<String>,
}

pub(crate) fn fetch_open_prs(repo: &str, limit: usize) -> Result<QueueSnapshot, String> {
    match fetch_open_pr_meta(repo, limit) {
        Ok(meta) => {
            return Ok(QueueSnapshot {
                repo: repo.to_string(),
                fetched_at: now_timestamp(),
                source: SnapshotSource::GhJson,
                prs: meta.iter().map(OpenPullRequest::from).collect(),
            });
        }
        Err(error) => {
            debug!(error = %error, "gh JSON PR list failed; trying plain output");
        }
    }

    fetch_plain_queue(repo, limit)
}

pub(crate) fn fetch_fact_snapshot(repo: &str, limit: usize) -> Result<FactSnapshot, String> {
    let mut warnings = Vec::new();
    let (meta, source) = match fetch_open_pr_meta(repo, limit) {
        Ok(meta) => (meta, SnapshotSource::GhFacts),
        Err(error) => {
            warnings.push(format!("meta JSON fetch failed: {error}"));
            let plain = fetch_plain_queue(repo, limit)?;
            (
                plain.prs.iter().map(meta_from_plain_pr).collect(),
                SnapshotSource::GhPlain,
            )
        }
    };

    let target_numbers = meta.iter().map(|row| row.number).collect::<HashSet<_>>();
    let mut stats = fetch_chunk("stats", &mut warnings, || fetch_open_pr_stats(repo, limit));
    stats.retain(|row| target_numbers.contains(&row.number));
    let mut files = fetch_chunk("files", &mut warnings, || fetch_open_pr_files(repo, limit));
    files.retain(|row| target_numbers.contains(&row.number));
    let mut reviews = fetch_chunk("reviews", &mut warnings, || {
        fetch_open_pr_reviews(repo, limit)
    });
    reviews.retain(|row| target_numbers.contains(&row.number));
    let mut commits = fetch_chunk("commits", &mut warnings, || {
        fetch_open_pr_commits(repo, limit)
    });
    commits.retain(|row| target_numbers.contains(&row.number));
    let mut comments = fetch_chunk("comments", &mut warnings, || {
        fetch_open_pr_comments(repo, limit)
    });
    comments.retain(|row| target_numbers.contains(&row.number));
    let mut assignment_events = fetch_chunk("assignment events", &mut warnings, || {
        fetch_open_pr_assignment_events(repo, limit)
    });
    assignment_events.retain(|row| target_numbers.contains(&row.number));

    Ok(FactSnapshot {
        repo: repo.to_string(),
        fetched_at: now_timestamp(),
        source,
        warnings,
        meta,
        stats,
        files,
        reviews,
        commits,
        comments,
        assignment_events,
    })
}

fn fetch_chunk<T>(
    label: &str,
    warnings: &mut Vec<String>,
    fetch: impl FnOnce() -> Result<Vec<T>, String>,
) -> Vec<T> {
    match fetch() {
        Ok(rows) => rows,
        Err(error) => {
            warnings.push(format!("{label} fetch failed: {error}"));
            Vec::new()
        }
    }
}

fn fetch_plain_queue(repo: &str, limit: usize) -> Result<QueueSnapshot, String> {
    let stdout = run_gh(&[
        "pr".to_string(),
        "list".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--state".to_string(),
        "open".to_string(),
        "--search".to_string(),
        "sort:updated-desc".to_string(),
        "--limit".to_string(),
        limit.to_string(),
    ])?;
    Ok(QueueSnapshot {
        repo: repo.to_string(),
        fetched_at: now_timestamp(),
        source: SnapshotSource::GhPlain,
        prs: parse_plain_pr_list(&stdout),
    })
}

fn fetch_open_pr_meta(repo: &str, limit: usize) -> Result<Vec<PrMeta>, String> {
    let stdout = run_gh(&[
        "pr".to_string(),
        "list".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--state".to_string(),
        "open".to_string(),
        "--search".to_string(),
        "sort:updated-desc".to_string(),
        "--limit".to_string(),
        limit.to_string(),
        "--json".to_string(),
        "number,title,author,createdAt,updatedAt,isDraft,reviewDecision,labels,maintainerCanModify,assignees,headRefName".to_string(),
    ])?;
    let rows = serde_json::from_str::<Vec<JsonPrMeta>>(&stdout)
        .map_err(|error| format!("failed to parse gh PR metadata JSON: {error}"))?;
    Ok(rows.into_iter().map(from_json_meta).collect())
}

fn fetch_open_pr_files(repo: &str, limit: usize) -> Result<Vec<PrFiles>, String> {
    let stdout = run_gh(&[
        "pr".to_string(),
        "list".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--state".to_string(),
        "open".to_string(),
        "--search".to_string(),
        "sort:updated-desc".to_string(),
        "--limit".to_string(),
        limit.to_string(),
        "--json".to_string(),
        "number,files".to_string(),
    ])?;
    let rows = serde_json::from_str::<Vec<JsonPrFiles>>(&stdout)
        .map_err(|error| format!("failed to parse gh PR files JSON: {error}"))?;
    Ok(rows
        .into_iter()
        .map(|row| PrFiles {
            number: row.number,
            files: row.files.into_iter().map(from_json_file).collect(),
        })
        .collect())
}

fn fetch_open_pr_stats(repo: &str, limit: usize) -> Result<Vec<PrStats>, String> {
    let rows = fetch_paginated_pr_list::<GraphqlStats>(
        repo,
        limit,
        r#"number
        additions
        deletions
        changedFiles
        headRefName
        headRefOid
        baseRefName
        mergeable
        mergeStateStatus"#,
    )?;
    Ok(rows.into_iter().map(from_graphql_stats).collect())
}

fn fetch_open_pr_reviews(repo: &str, limit: usize) -> Result<Vec<Review>, String> {
    let rows = fetch_paginated_pr_list::<GraphqlReviewNode>(
        repo,
        limit,
        r#"number
        reviews(last: 30) {
          nodes {
            author { login }
            body
            state
            submittedAt
            commit { oid }
          }
        }"#,
    )?;
    Ok(rows
        .into_iter()
        .flat_map(|row| {
            row.reviews.nodes.into_iter().map(move |review| Review {
                number: row.number,
                author: review.author.map(|author| author.login),
                body: review.body.unwrap_or_default(),
                state: review.state,
                submitted_at: review.submitted_at,
                commit_oid: review.commit.map(|commit| commit.oid),
            })
        })
        .collect())
}

fn fetch_open_pr_commits(repo: &str, limit: usize) -> Result<Vec<Commit>, String> {
    let rows = fetch_paginated_pr_list::<GraphqlCommitNode>(
        repo,
        limit,
        r#"number
        commits(last: 5) {
          nodes {
            commit {
              oid
              committedDate
              author { user { login } }
            }
          }
        }"#,
    )?;
    Ok(rows
        .into_iter()
        .flat_map(|row| {
            row.commits.nodes.into_iter().map(move |entry| Commit {
                number: row.number,
                oid: entry.commit.oid,
                committed_date: entry.commit.committed_date,
                author_login: entry
                    .commit
                    .author
                    .and_then(|author| author.user.map(|user| user.login)),
            })
        })
        .collect())
}

fn fetch_open_pr_comments(repo: &str, limit: usize) -> Result<Vec<Comment>, String> {
    let rows = fetch_paginated_pr_list::<GraphqlCommentNode>(
        repo,
        limit,
        r#"number
        comments(last: 30) {
          nodes {
            author { login }
            body
            createdAt
          }
        }
        reviewThreads(last: 20) {
          nodes {
            comments(first: 20) {
              nodes {
                author { login }
                body
                createdAt
              }
            }
          }
        }"#,
    )?;
    let mut comments = Vec::new();
    for row in rows {
        for comment in row.comments.nodes {
            comments.push(from_graphql_comment(row.number, comment, "issue"));
        }
        for thread in row.review_threads.nodes {
            for comment in thread.comments.nodes {
                comments.push(from_graphql_comment(row.number, comment, "inline"));
            }
        }
    }
    Ok(comments)
}

fn fetch_open_pr_assignment_events(
    repo: &str,
    limit: usize,
) -> Result<Vec<AssignmentEvent>, String> {
    let rows = fetch_paginated_pr_list::<GraphqlAssignmentNode>(
        repo,
        limit,
        r#"number
        timelineItems(itemTypes: [ASSIGNED_EVENT, UNASSIGNED_EVENT], last: 20) {
          nodes {
            __typename
            ... on AssignedEvent {
              createdAt
              actor { login }
              assignee {
                __typename
                ... on User { login }
              }
            }
            ... on UnassignedEvent {
              createdAt
              actor { login }
              assignee {
                __typename
                ... on User { login }
              }
            }
          }
        }"#,
    )?;
    Ok(rows
        .into_iter()
        .flat_map(|row| {
            row.timeline_items
                .nodes
                .into_iter()
                .map(move |event| AssignmentEvent {
                    number: row.number,
                    kind: match event.typename.as_str() {
                        "AssignedEvent" => "ASSIGNED".to_string(),
                        "UnassignedEvent" => "UNASSIGNED".to_string(),
                        other => other.to_string(),
                    },
                    created_at: event.created_at,
                    actor: event.actor.map(|actor| actor.login),
                    assignee: event.assignee.and_then(|assignee| {
                        if assignee.typename == "User" {
                            assignee.login
                        } else {
                            None
                        }
                    }),
                })
        })
        .collect())
}

fn fetch_paginated_pr_list<N>(repo: &str, limit: usize, node_fields: &str) -> Result<Vec<N>, String>
where
    N: DeserializeOwned,
{
    let (owner, name) = split_repo(repo)?;
    let query = format!(
        r#"query($owner: String!, $name: String!, $first: Int!, $cursor: String) {{
  repository(owner: $owner, name: $name) {{
    pullRequests(states: OPEN, first: $first, after: $cursor, orderBy: {{ field: UPDATED_AT, direction: DESC }}) {{
      nodes {{ {node_fields} }}
      pageInfo {{ hasNextPage endCursor }}
    }}
  }}
}}"#
    );

    let mut all = Vec::new();
    let mut cursor = None::<String>;
    while all.len() < limit {
        let remaining = limit - all.len();
        let first = remaining.min(PR_LIST_PAGE_SIZE);
        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-F".to_string(),
            format!("owner={owner}"),
            "-F".to_string(),
            format!("name={name}"),
            "-F".to_string(),
            format!("first={first}"),
        ];
        if let Some(cursor) = &cursor {
            args.push("-F".to_string());
            args.push(format!("cursor={cursor}"));
        }
        args.push("-f".to_string());
        args.push(format!("query={query}"));

        let stdout = run_gh(&args)?;
        let page = serde_json::from_str::<GraphqlPage<N>>(&stdout)
            .map_err(|error| format!("failed to parse gh GraphQL page: {error}"))?;
        let mut nodes = page.data.repository.pull_requests.nodes;
        all.append(&mut nodes);
        let info = page.data.repository.pull_requests.page_info;
        if !info.has_next_page {
            break;
        }
        cursor = info.end_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(all)
}

fn split_repo(repo: &str) -> Result<(&str, &str), String> {
    repo.split_once('/')
        .filter(|(owner, name)| !owner.is_empty() && !name.is_empty())
        .ok_or_else(|| format!("repo must look like owner/name, got {repo}"))
}

fn from_json_meta(row: JsonPrMeta) -> PrMeta {
    PrMeta {
        number: row.number,
        title: row.title,
        author: row.author.map(|author| author.login),
        created_at: row.created_at,
        updated_at: row.updated_at,
        is_draft: row.is_draft,
        review_decision: row.review_decision,
        labels: row
            .labels
            .unwrap_or_default()
            .into_iter()
            .map(|label| label.name)
            .collect(),
        maintainer_can_modify: row.maintainer_can_modify,
        assignees: row
            .assignees
            .unwrap_or_default()
            .into_iter()
            .map(|assignee| assignee.login)
            .collect(),
        head_ref_name: row.head_ref_name,
    }
}

fn meta_from_plain_pr(pr: &OpenPullRequest) -> PrMeta {
    PrMeta {
        number: pr.number,
        title: pr.title.clone(),
        author: pr.author.clone(),
        created_at: None,
        updated_at: pr.updated_at.clone(),
        is_draft: pr.is_draft,
        review_decision: None,
        labels: Vec::new(),
        maintainer_can_modify: None,
        assignees: Vec::new(),
        head_ref_name: pr.head_ref.clone(),
    }
}

fn from_json_file(file: JsonFile) -> FileChange {
    FileChange {
        path: file.path,
        additions: file.additions,
        deletions: file.deletions,
        change_type: file.change_type,
    }
}

fn from_graphql_stats(row: GraphqlStats) -> PrStats {
    PrStats {
        number: row.number,
        additions: row.additions,
        deletions: row.deletions,
        changed_files: row.changed_files,
        head_ref_name: row.head_ref_name,
        head_ref_oid: row.head_ref_oid,
        base_ref_name: row.base_ref_name,
        mergeable: row.mergeable,
        merge_state_status: row.merge_state_status,
    }
}

fn from_graphql_comment(number: u64, comment: GraphqlComment, source: &str) -> Comment {
    Comment {
        number,
        author: comment.author.map(|author| author.login),
        body: comment.body.unwrap_or_default(),
        created_at: comment.created_at,
        source: source.to_string(),
    }
}

fn run_gh(args: &[String]) -> Result<String, String> {
    let backoff = [Duration::from_millis(800), Duration::from_millis(1600)];
    let mut last_error = String::new();
    for attempt in 0..=backoff.len() {
        let output = Command::new("gh")
            .args(args)
            .output()
            .map_err(|error| format!("failed to run gh: {error}"))?;
        if output.status.success() {
            return String::from_utf8(output.stdout)
                .map_err(|error| format!("gh returned non-UTF-8 output: {error}"));
        }

        last_error = format!(
            "gh {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        if attempt == backoff.len() || !looks_transient(&last_error) {
            break;
        }
        sleep(backoff[attempt]);
    }
    Err(last_error)
}

fn looks_transient(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("http 5")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
        || lower.contains("timeout")
        || lower.contains("eof")
        || lower.contains("econnreset")
        || lower.contains("etimedout")
        || lower.contains("eai_again")
}

fn now_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    seconds.to_string()
}

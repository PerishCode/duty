use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use duty_core::{parse_plain_pr_list, OpenPullRequest, QueueSnapshot, SnapshotSource};
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonPullRequest {
    number: u64,
    title: String,
    author: Option<JsonUser>,
    head_ref_name: Option<String>,
    updated_at: Option<String>,
    is_draft: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JsonUser {
    login: String,
}

pub(crate) fn fetch_open_prs(repo: &str, limit: usize) -> Result<QueueSnapshot, String> {
    let json_args = [
        "pr",
        "list",
        "--repo",
        repo,
        "--state",
        "open",
        "--limit",
        &limit.to_string(),
        "--json",
        "number,title,author,headRefName,updatedAt,isDraft",
    ];
    match run_gh(&json_args) {
        Ok(stdout) => {
            let rows = serde_json::from_str::<Vec<JsonPullRequest>>(&stdout)
                .map_err(|error| format!("failed to parse gh JSON output: {error}"))?;
            return Ok(QueueSnapshot {
                repo: repo.to_string(),
                fetched_at: now_timestamp(),
                source: SnapshotSource::GhJson,
                prs: rows.into_iter().map(from_json_pr).collect(),
            });
        }
        Err(error) => {
            debug!(error = %error, "gh JSON PR list failed; trying plain output");
        }
    }

    let plain_args = [
        "pr",
        "list",
        "--repo",
        repo,
        "--state",
        "open",
        "--limit",
        &limit.to_string(),
    ];
    let stdout = run_gh(&plain_args)?;
    Ok(QueueSnapshot {
        repo: repo.to_string(),
        fetched_at: now_timestamp(),
        source: SnapshotSource::GhPlain,
        prs: parse_plain_pr_list(&stdout),
    })
}

fn from_json_pr(row: JsonPullRequest) -> OpenPullRequest {
    OpenPullRequest {
        number: row.number,
        title: row.title,
        author: row.author.map(|author| author.login),
        head_ref: row.head_ref_name,
        state: Some("OPEN".to_string()),
        updated_at: row.updated_at,
        is_draft: row.is_draft,
    }
}

fn run_gh(args: &[&str]) -> Result<String, String> {
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

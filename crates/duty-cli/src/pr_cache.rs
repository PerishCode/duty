use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use duty_core::{
    Comment, Commit, FileChange, PullRequestView, Review, SnapshotSource, StatusCheck,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::debug;

pub(crate) const CURRENT_SCHEMA: u32 = 1;
pub(crate) const DEFAULT_ROOT: &str = ".tmp/duty";
pub(crate) const INCLUDE_CLOSED_ENV: &str = "DUTY_PR_INCLUDE_CLOSED";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CacheMode {
    Offline,
    Incremental,
    Refresh,
}

impl CacheMode {
    pub(crate) fn from_flags(offline: bool, refresh: bool) -> Result<Self, String> {
        match (offline, refresh) {
            (true, true) => Err("--offline and --refresh are mutually exclusive".to_string()),
            (true, false) => Ok(CacheMode::Offline),
            (false, true) => Ok(CacheMode::Refresh),
            (false, false) => Ok(CacheMode::Incremental),
        }
    }
}

pub(crate) fn include_closed_prs(flag: bool) -> bool {
    if flag {
        return true;
    }
    std::env::var(INCLUDE_CLOSED_ENV)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum Subresource {
    Metadata,
    Files,
    Checks,
    Reviews,
    IssueComments,
    ReviewComments,
    Commits,
}

impl Subresource {
    pub(crate) fn filename(self) -> &'static str {
        match self {
            Subresource::Metadata => "metadata.json",
            Subresource::Files => "files.json",
            Subresource::Checks => "checks.json",
            Subresource::Reviews => "reviews.json",
            Subresource::IssueComments => "issue_comments.json",
            Subresource::ReviewComments => "review_comments.json",
            Subresource::Commits => "commits.json",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RepoPaths {
    root: PathBuf,
}

impl RepoPaths {
    pub(crate) fn new(root: impl Into<PathBuf>, repo: &str) -> Result<Self, String> {
        let (org, name) = split_repo(repo)?;
        let root = root.into().join(org).join(name);
        Ok(RepoPaths { root })
    }

    pub(crate) fn prs_dir(&self) -> PathBuf {
        self.root.join("prs")
    }

    pub(crate) fn pr_dir(&self, number: u64) -> PathBuf {
        self.prs_dir().join(number.to_string())
    }

    pub(crate) fn pr_index(&self) -> PathBuf {
        self.prs_dir().join("index.json")
    }

    pub(crate) fn pr_subresource(&self, number: u64, sub: Subresource) -> PathBuf {
        self.pr_dir(number).join(sub.filename())
    }
}

fn split_repo(repo: &str) -> Result<(&str, &str), String> {
    repo.split_once('/')
        .filter(|(org, name)| !org.is_empty() && !name.is_empty())
        .ok_or_else(|| format!("repo must be owner/name; got: {repo}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Envelope<T> {
    pub(crate) schema_version: u32,
    pub(crate) fetched_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) etag: Option<String>,
    pub(crate) payload: T,
}

impl<T> Envelope<T> {
    pub(crate) fn new(payload: T, source_updated_at: Option<String>) -> Self {
        Envelope {
            schema_version: CURRENT_SCHEMA,
            fetched_at: now_epoch_seconds(),
            source_updated_at,
            etag: None,
            payload,
        }
    }
}

pub(crate) fn now_epoch_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PrIndexEntry {
    pub(crate) number: u64,
    pub(crate) state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) updated_at: Option<String>,
    #[serde(default)]
    pub(crate) labels: Vec<String>,
    pub(crate) fetched_at: String,
}

pub(crate) type PrIndex = BTreeMap<u64, PrIndexEntry>;

pub(crate) fn write_subresource<T: Serialize>(
    path: &Path,
    envelope: &Envelope<T>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create cache directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(envelope)
        .map_err(|error| format!("failed to serialize cache envelope: {error}"))?;
    let partial = partial_path(path);
    fs::write(&partial, format!("{text}\n")).map_err(|error| {
        format!(
            "failed to write cache partial {}: {error}",
            partial.display()
        )
    })?;
    fs::rename(&partial, path).map_err(|error| {
        let _ = fs::remove_file(&partial);
        format!(
            "failed to promote cache partial {} -> {}: {error}",
            partial.display(),
            path.display()
        )
    })
}

pub(crate) fn read_subresource<T: DeserializeOwned>(path: &Path) -> Option<Envelope<T>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            debug!(
                error = %error,
                path = %path.display(),
                "cache read failed; treating as miss"
            );
            return None;
        }
    };
    match serde_json::from_str::<Envelope<T>>(&text) {
        Ok(envelope) if envelope.schema_version == CURRENT_SCHEMA => Some(envelope),
        Ok(envelope) => {
            debug!(
                schema = envelope.schema_version,
                expected = CURRENT_SCHEMA,
                path = %path.display(),
                "cache schema mismatch; treating as miss"
            );
            None
        }
        Err(error) => {
            debug!(
                error = %error,
                path = %path.display(),
                "cache parse failed; treating as miss"
            );
            None
        }
    }
}

fn partial_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("payload");
    path.with_file_name(format!(".{file_name}.partial"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PrMetadataPayload {
    pub(crate) repo: String,
    pub(crate) number: u64,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) state: String,
    pub(crate) author: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) updated_at: Option<String>,
    pub(crate) is_draft: Option<bool>,
    pub(crate) review_decision: Option<String>,
    pub(crate) merge_state_status: Option<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) additions: Option<u64>,
    pub(crate) deletions: Option<u64>,
    pub(crate) changed_files: Option<u64>,
    pub(crate) base_ref_name: Option<String>,
    pub(crate) head_ref_name: Option<String>,
    pub(crate) head_ref_oid: Option<String>,
    pub(crate) maintainer_can_modify: Option<bool>,
    pub(crate) assignees: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) struct SplitView {
    pub(crate) metadata: PrMetadataPayload,
    pub(crate) files: Vec<FileChange>,
    pub(crate) checks: Vec<StatusCheck>,
    pub(crate) reviews: Vec<Review>,
    pub(crate) issue_comments: Vec<Comment>,
    pub(crate) review_comments: Vec<Comment>,
    pub(crate) commits: Vec<Commit>,
}

pub(crate) fn split_view(view: &PullRequestView) -> SplitView {
    let metadata = PrMetadataPayload {
        repo: view.repo.clone(),
        number: view.number,
        url: view.url.clone(),
        title: view.title.clone(),
        body: view.body.clone(),
        state: view.state.clone(),
        author: view.author.clone(),
        created_at: view.created_at.clone(),
        updated_at: view.updated_at.clone(),
        is_draft: view.is_draft,
        review_decision: view.review_decision.clone(),
        merge_state_status: view.merge_state_status.clone(),
        labels: view.labels.clone(),
        additions: view.additions,
        deletions: view.deletions,
        changed_files: view.changed_files,
        base_ref_name: view.base_ref_name.clone(),
        head_ref_name: view.head_ref_name.clone(),
        head_ref_oid: view.head_ref_oid.clone(),
        maintainer_can_modify: view.maintainer_can_modify,
        assignees: view.assignees.clone(),
        warnings: view.warnings.clone(),
    };
    let (issue_comments, review_comments): (Vec<_>, Vec<_>) = view
        .comments
        .iter()
        .cloned()
        .partition(|comment| comment.source != "inline");
    SplitView {
        metadata,
        files: view.files.clone(),
        checks: view.status_check_rollup.clone(),
        reviews: view.reviews.clone(),
        issue_comments,
        review_comments,
        commits: view.commits.clone(),
    }
}

pub(crate) struct JoinedParts {
    pub(crate) metadata: PrMetadataPayload,
    pub(crate) files: Vec<FileChange>,
    pub(crate) checks: Vec<StatusCheck>,
    pub(crate) reviews: Vec<Review>,
    pub(crate) issue_comments: Vec<Comment>,
    pub(crate) review_comments: Vec<Comment>,
    pub(crate) commits: Vec<Commit>,
    pub(crate) fetched_at: String,
}

pub(crate) fn join_view(parts: JoinedParts) -> PullRequestView {
    let mut comments = parts.issue_comments;
    comments.extend(parts.review_comments);
    let metadata = parts.metadata;
    PullRequestView {
        repo: metadata.repo,
        fetched_at: parts.fetched_at,
        source: SnapshotSource::Cache,
        warnings: metadata.warnings,
        number: metadata.number,
        url: metadata.url,
        title: metadata.title,
        body: metadata.body,
        state: metadata.state,
        author: metadata.author,
        created_at: metadata.created_at,
        updated_at: metadata.updated_at,
        is_draft: metadata.is_draft,
        review_decision: metadata.review_decision,
        merge_state_status: metadata.merge_state_status,
        labels: metadata.labels,
        additions: metadata.additions,
        deletions: metadata.deletions,
        changed_files: metadata.changed_files,
        base_ref_name: metadata.base_ref_name,
        head_ref_name: metadata.head_ref_name,
        head_ref_oid: metadata.head_ref_oid,
        maintainer_can_modify: metadata.maintainer_can_modify,
        assignees: metadata.assignees,
        files: parts.files,
        status_check_rollup: parts.checks,
        reviews: parts.reviews,
        comments,
        commits: parts.commits,
    }
}

pub(crate) fn read_pr_index(path: &Path) -> PrIndex {
    read_subresource::<PrIndex>(path)
        .map(|env| env.payload)
        .unwrap_or_default()
}

pub(crate) fn write_pr_index(path: &Path, index: &PrIndex) -> Result<(), String> {
    let envelope = Envelope::new(index.clone(), None);
    write_subresource(path, &envelope)
}

pub(crate) fn evict_pr_dir(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to evict cache directory {}: {error}",
            path.display()
        )),
    }
}

/// Removes legacy `<root>/cache/<owner>__<name>/views/` directories left over
/// from the pre-resource-tree layout. Idempotent. Other legacy artifacts under
/// `<root>/cache/` (queue, facts, classify) remain until their respective
/// surfaces migrate to the new layout.
pub(crate) fn wipe_legacy_view_cache(root: &Path) -> Result<(), String> {
    let legacy_cache = root.join("cache");
    let entries = match fs::read_dir(&legacy_cache) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            debug!(
                error = %error,
                path = %legacy_cache.display(),
                "legacy cache scan failed; skipping wipe"
            );
            return Ok(());
        }
    };
    for entry in entries.flatten() {
        let views = entry.path().join("views");
        if views.is_dir() {
            if let Err(error) = fs::remove_dir_all(&views) {
                debug!(
                    error = %error,
                    path = %views.display(),
                    "failed to remove legacy views/ dir"
                );
                continue;
            }
            tracing::info!(path = %views.display(), "removed legacy view cache");
        }
    }
    Ok(())
}

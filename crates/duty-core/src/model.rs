use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct OpenPullRequest {
    pub number: u64,
    pub title: String,
    pub author: Option<String>,
    pub head_ref: Option<String>,
    pub state: Option<String>,
    pub updated_at: Option<String>,
    pub is_draft: Option<bool>,
}

impl From<&PrMeta> for OpenPullRequest {
    fn from(meta: &PrMeta) -> Self {
        Self {
            number: meta.number,
            title: meta.title.clone(),
            author: meta.author.clone(),
            head_ref: meta.head_ref_name.clone(),
            state: Some("OPEN".to_string()),
            updated_at: meta.updated_at.clone(),
            is_draft: meta.is_draft,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotSource {
    GhJson,
    GhPlain,
    GhFacts,
    GhView,
    Cache,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub repo: String,
    pub fetched_at: String,
    pub source: SnapshotSource,
    pub prs: Vec<OpenPullRequest>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FactSnapshot {
    pub repo: String,
    pub fetched_at: String,
    pub source: SnapshotSource,
    pub warnings: Vec<String>,
    pub meta: Vec<PrMeta>,
    pub stats: Vec<PrStats>,
    pub files: Vec<PrFiles>,
    pub reviews: Vec<Review>,
    pub commits: Vec<Commit>,
    pub comments: Vec<Comment>,
    pub assignment_events: Vec<AssignmentEvent>,
}

impl FactSnapshot {
    pub fn queue_prs(&self) -> Vec<OpenPullRequest> {
        self.meta.iter().map(OpenPullRequest::from).collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestView {
    pub repo: String,
    pub fetched_at: String,
    pub source: SnapshotSource,
    pub warnings: Vec<String>,
    pub number: u64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub state: String,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub is_draft: Option<bool>,
    pub review_decision: Option<String>,
    pub merge_state_status: Option<String>,
    pub labels: Vec<String>,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub changed_files: Option<u64>,
    pub base_ref_name: Option<String>,
    pub head_ref_name: Option<String>,
    pub head_ref_oid: Option<String>,
    pub maintainer_can_modify: Option<bool>,
    pub assignees: Vec<String>,
    pub files: Vec<FileChange>,
    pub status_check_rollup: Vec<StatusCheck>,
    pub reviews: Vec<Review>,
    pub comments: Vec<Comment>,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrMeta {
    pub number: u64,
    pub title: String,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub is_draft: Option<bool>,
    pub review_decision: Option<String>,
    pub labels: Vec<String>,
    pub maintainer_can_modify: Option<bool>,
    pub assignees: Vec<String>,
    pub head_ref_name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrStats {
    pub number: u64,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub changed_files: Option<u64>,
    pub head_ref_name: Option<String>,
    pub head_ref_oid: Option<String>,
    pub base_ref_name: Option<String>,
    pub mergeable: Option<String>,
    pub merge_state_status: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StatusCheck {
    pub typename: Option<String>,
    pub name: Option<String>,
    pub workflow_name: Option<String>,
    pub conclusion: Option<String>,
    pub status: Option<String>,
    pub state: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshot {
    pub remaining: u64,
    pub limit: u64,
    pub reset_at: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrFiles {
    pub number: u64,
    pub files: Vec<FileChange>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Review {
    pub number: u64,
    pub author: Option<String>,
    pub body: String,
    pub state: String,
    pub submitted_at: Option<String>,
    pub commit_oid: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Commit {
    pub number: u64,
    pub oid: Option<String>,
    pub committed_date: Option<String>,
    pub author_login: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub number: u64,
    pub author: Option<String>,
    pub body: String,
    pub created_at: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssignmentEvent {
    pub number: u64,
    pub kind: String,
    pub created_at: Option<String>,
    pub actor: Option<String>,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DutyConfig {
    pub github: GithubConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GithubConfig {
    pub default_repo: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CacheConfig {
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { ttl_seconds: 900 }
    }
}

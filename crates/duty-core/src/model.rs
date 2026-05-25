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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotSource {
    GhJson,
    GhPlain,
    Cache,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub repo: String,
    pub fetched_at: String,
    pub source: SnapshotSource,
    pub prs: Vec<OpenPullRequest>,
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

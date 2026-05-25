pub mod model;
pub mod plain;

pub use model::{
    AssignmentEvent, CacheConfig, Comment, Commit, DutyConfig, FactSnapshot, FileChange,
    GithubConfig, OpenPullRequest, PrFiles, PrMeta, PrStats, PullRequestView, QueueSnapshot,
    RateLimitSnapshot, Review, SnapshotSource, StatusCheck,
};
pub use plain::parse_plain_pr_list;

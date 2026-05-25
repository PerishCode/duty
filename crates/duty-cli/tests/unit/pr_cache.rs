use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::pr_cache::{
    join_view, read_subresource, split_view, wipe_legacy_view_cache, write_subresource, CacheMode,
    Envelope, JoinedParts, RepoPaths, Subresource, CURRENT_SCHEMA,
};

use duty_core::{
    Comment, Commit, FileChange, PullRequestView, Review, SnapshotSource, StatusCheck,
};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let path =
            std::env::temp_dir().join(format!("duty-pr-cache-{pid}-{nanos}-{counter}-{label}"));
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn repo_paths_split_org_and_name() {
    let paths = RepoPaths::new(".tmp/duty", "nexu-io/open-design").unwrap();
    assert_eq!(
        paths.prs_dir(),
        PathBuf::from(".tmp/duty/nexu-io/open-design/prs")
    );
    assert_eq!(
        paths.pr_index(),
        PathBuf::from(".tmp/duty/nexu-io/open-design/prs/index.json")
    );
    assert_eq!(
        paths.pr_dir(2862),
        PathBuf::from(".tmp/duty/nexu-io/open-design/prs/2862")
    );
    assert_eq!(
        paths.pr_subresource(2862, Subresource::Metadata),
        PathBuf::from(".tmp/duty/nexu-io/open-design/prs/2862/metadata.json")
    );
    assert_eq!(
        paths.pr_subresource(2862, Subresource::ReviewComments),
        PathBuf::from(".tmp/duty/nexu-io/open-design/prs/2862/review_comments.json")
    );
}

#[test]
fn repo_paths_rejects_malformed() {
    assert!(RepoPaths::new(".tmp/duty", "no-slash").is_err());
    assert!(RepoPaths::new(".tmp/duty", "/missing-org").is_err());
    assert!(RepoPaths::new(".tmp/duty", "missing-name/").is_err());
}

#[test]
fn subresource_filenames_match_github_naming() {
    assert_eq!(Subresource::Metadata.filename(), "metadata.json");
    assert_eq!(Subresource::Reviews.filename(), "reviews.json");
    assert_eq!(
        Subresource::ReviewComments.filename(),
        "review_comments.json"
    );
    assert_eq!(Subresource::IssueComments.filename(), "issue_comments.json");
    assert_eq!(Subresource::Commits.filename(), "commits.json");
    assert_eq!(Subresource::Checks.filename(), "checks.json");
    assert_eq!(Subresource::Files.filename(), "files.json");
}

#[test]
fn envelope_serializes_with_schema_version() {
    let env = Envelope::new(vec![1u32, 2, 3], Some("2026-05-25T00:00:00Z".to_string()));
    let json = serde_json::to_string(&env).unwrap();
    assert!(json.contains("\"schema_version\":1"));
    assert!(json.contains("\"payload\":[1,2,3]"));
    assert!(json.contains("\"source_updated_at\":\"2026-05-25T00:00:00Z\""));
}

#[test]
fn envelope_roundtrip_preserves_payload() {
    let env = Envelope::new(42u64, None);
    let json = serde_json::to_string(&env).unwrap();
    let parsed: Envelope<u64> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.payload, 42);
    assert_eq!(parsed.schema_version, CURRENT_SCHEMA);
    assert_eq!(parsed.source_updated_at, None);
    assert!(parsed.etag.is_none());
}

#[test]
fn write_then_read_roundtrip() {
    let tmp = TempDir::new("rw");
    let path = tmp.path().join("nested/dir/payload.json");
    let env = Envelope::new(vec!["a".to_string(), "b".to_string()], None);
    write_subresource(&path, &env).unwrap();

    let read: Envelope<Vec<String>> = read_subresource(&path).expect("should be a hit");
    assert_eq!(read.payload, env.payload);
    assert_eq!(read.schema_version, CURRENT_SCHEMA);
    // partial file should not linger
    assert!(!tmp.path().join("nested/dir/.payload.json.partial").exists());
}

#[test]
fn read_missing_path_returns_none() {
    let tmp = TempDir::new("missing");
    let path = tmp.path().join("absent.json");
    let read: Option<Envelope<u64>> = read_subresource(&path);
    assert!(read.is_none());
}

#[test]
fn read_corrupt_file_returns_none() {
    let tmp = TempDir::new("corrupt");
    let path = tmp.path().join("corrupt.json");
    fs::write(&path, "not json at all").unwrap();
    let read: Option<Envelope<u64>> = read_subresource(&path);
    assert!(read.is_none());
}

#[test]
fn read_schema_mismatch_returns_none() {
    let tmp = TempDir::new("schema");
    let path = tmp.path().join("future.json");
    // Hand-craft a future-version envelope; structurally compatible but version-bumped.
    let body = serde_json::json!({
        "schema_version": CURRENT_SCHEMA + 999,
        "fetched_at": "0",
        "payload": 7u64,
    });
    fs::write(&path, body.to_string()).unwrap();
    let read: Option<Envelope<u64>> = read_subresource(&path);
    assert!(read.is_none(), "schema bump must be treated as a miss");
}

#[test]
fn write_creates_parent_directories() {
    let tmp = TempDir::new("mkdir");
    let path = tmp.path().join("a/b/c/d/payload.json");
    let env = Envelope::new(true, None);
    write_subresource(&path, &env).unwrap();
    assert!(path.exists());
}

#[test]
fn cache_mode_resolves_three_states() {
    assert_eq!(
        CacheMode::from_flags(false, false).unwrap(),
        CacheMode::Incremental
    );
    assert_eq!(
        CacheMode::from_flags(true, false).unwrap(),
        CacheMode::Offline
    );
    assert_eq!(
        CacheMode::from_flags(false, true).unwrap(),
        CacheMode::Refresh
    );
    assert!(CacheMode::from_flags(true, true).is_err());
}

#[test]
fn legacy_wipe_removes_only_views_dirs() {
    let tmp = TempDir::new("legacy-wipe");
    let root = tmp.path();
    // Construct legacy layout: cache/<o>__<r>/views/<n>.json and cache/<o>__<r>/{open-prs,facts}.json
    let legacy = root.join("cache/nexu-io__open-design");
    fs::create_dir_all(legacy.join("views")).unwrap();
    fs::write(legacy.join("views/2862.json"), "{}").unwrap();
    fs::write(legacy.join("open-prs.json"), "{}").unwrap();
    fs::write(legacy.join("facts.json"), "{}").unwrap();
    // A second repo with no views/ — should be untouched.
    let other = root.join("cache/some__other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("facts.json"), "{}").unwrap();

    wipe_legacy_view_cache(root).unwrap();

    assert!(!legacy.join("views").exists(), "views/ must be removed");
    assert!(legacy.join("open-prs.json").exists(), "open-prs.json kept");
    assert!(legacy.join("facts.json").exists(), "facts.json kept");
    assert!(other.join("facts.json").exists(), "untouched repo kept");
}

#[test]
fn legacy_wipe_is_idempotent_when_root_missing() {
    let tmp = TempDir::new("legacy-wipe-missing");
    // Don't create cache/ at all.
    wipe_legacy_view_cache(tmp.path()).unwrap();
    wipe_legacy_view_cache(tmp.path()).unwrap();
}

#[test]
fn split_then_join_roundtrip_preserves_view() {
    let original = sample_view();
    let split = split_view(&original);
    // Issue vs review comments split by source label.
    assert!(split.issue_comments.iter().all(|c| c.source == "issue"));
    assert!(split.review_comments.iter().all(|c| c.source == "inline"));
    let rejoined = join_view(JoinedParts {
        metadata: split.metadata,
        files: split.files,
        checks: split.checks,
        reviews: split.reviews,
        issue_comments: split.issue_comments,
        review_comments: split.review_comments,
        commits: split.commits,
        fetched_at: original.fetched_at.clone(),
    });
    // join_view forces source = Cache; compare everything else.
    assert_eq!(rejoined.source, SnapshotSource::Cache);
    let mut expected = original.clone();
    expected.source = SnapshotSource::Cache;
    // Comments order: issue first then inline (split preserves order; join concatenates).
    expected.comments.sort_by(|a, b| a.source.cmp(&b.source));
    let mut rejoined_sorted = rejoined.clone();
    rejoined_sorted
        .comments
        .sort_by(|a, b| a.source.cmp(&b.source));
    assert_eq!(rejoined_sorted, expected);
}

fn sample_view() -> PullRequestView {
    PullRequestView {
        repo: "nexu-io/open-design".to_string(),
        fetched_at: "1700000000".to_string(),
        source: SnapshotSource::GhView,
        warnings: vec!["partial fetch x".to_string()],
        number: 2862,
        url: "https://github.com/nexu-io/open-design/pull/2862".to_string(),
        title: "tiny padding tweak".to_string(),
        body: "Fixes #2852".to_string(),
        state: "OPEN".to_string(),
        author: Some("vitalysinitsin".to_string()),
        created_at: Some("2026-05-25T00:00:00Z".to_string()),
        updated_at: Some("2026-05-25T05:10:39Z".to_string()),
        is_draft: Some(false),
        review_decision: Some("APPROVED".to_string()),
        merge_state_status: Some("UNKNOWN".to_string()),
        labels: vec!["size/XS".to_string(), "risk/medium".to_string()],
        additions: Some(3),
        deletions: Some(3),
        changed_files: Some(1),
        base_ref_name: Some("main".to_string()),
        head_ref_name: Some("issue2852".to_string()),
        head_ref_oid: Some("deadbeef".to_string()),
        maintainer_can_modify: Some(true),
        assignees: vec![],
        files: vec![FileChange {
            path: "apps/web/src/index.css".to_string(),
            additions: Some(3),
            deletions: Some(3),
            change_type: Some("MODIFIED".to_string()),
        }],
        status_check_rollup: vec![StatusCheck {
            typename: None,
            name: Some("ci".to_string()),
            workflow_name: Some("ci".to_string()),
            conclusion: Some("SUCCESS".to_string()),
            status: None,
            state: None,
            context: None,
        }],
        reviews: vec![Review {
            number: 2862,
            author: Some("lefarcen".to_string()),
            body: "nice".to_string(),
            state: "COMMENTED".to_string(),
            submitted_at: Some("2026-05-25T05:10:39Z".to_string()),
            commit_oid: None,
        }],
        comments: vec![
            Comment {
                number: 2862,
                author: Some("lefarcen".to_string()),
                body: "issue-level note".to_string(),
                created_at: Some("2026-05-25T05:00:00Z".to_string()),
                source: "issue".to_string(),
            },
            Comment {
                number: 2862,
                author: Some("lefarcen".to_string()),
                body: "inline review note".to_string(),
                created_at: Some("2026-05-25T05:05:00Z".to_string()),
                source: "inline".to_string(),
            },
        ],
        commits: vec![Commit {
            number: 2862,
            oid: Some("deadbeef".to_string()),
            committed_date: Some("2026-05-25T00:00:00Z".to_string()),
            author_login: Some("vitalysinitsin".to_string()),
        }],
    }
}

#[test]
fn write_uses_hidden_partial_filename() {
    let tmp = TempDir::new("partial");
    let path = tmp.path().join("payload.json");
    let env = Envelope::new(1u64, None);
    write_subresource(&path, &env).unwrap();
    // After successful write, the partial must be renamed away.
    // The hidden naming pattern is .<file>.partial — verify it's gone, not the basename.
    let partial = tmp.path().join(".payload.json.partial");
    assert!(!partial.exists());
    assert!(path.exists());
}

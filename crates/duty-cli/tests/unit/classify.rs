use std::collections::HashSet;

use duty_core::{
    Comment, Commit, FactSnapshot, FileChange, PrFiles, PrMeta, PrStats, RateLimitSnapshot, Review,
    SnapshotSource,
};

use crate::{
    classify::{run_classify, tags_for_number_with_org_members, ClassifyRunContext, RateReport},
    cli::{ClassifyOptions, LogLevel, OutputFormat, QueueOptions},
};

#[test]
fn emits_tags_from_fact_snapshot() {
    let tags = tags_for_number_with_org_members(&snapshot(), 1, &HashSet::new()).expect("tags");
    let names = tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>();

    assert!(names.contains(&"bot-only-approval"));
    assert!(names.contains(&"stale-approval"));
    assert!(names.contains(&"maintainer-edits-disabled"));
}

#[test]
fn detects_org_member_from_runtime_context() {
    let org_members = HashSet::from(["alice".to_string()]);
    let tags =
        tags_for_number_with_org_members(&snapshot(), 1, &org_members).expect("org member tags");
    let names = tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>();

    assert!(names.contains(&"org-member"));
}

#[test]
fn detects_rebase_forbidden_unlabeled_and_duplicate_title() {
    let snapshot = snapshot();
    let tags = tags_for_number_with_org_members(&snapshot, 2, &HashSet::new()).expect("tags");
    let names = tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>();

    assert!(names.contains(&"needs-rebase"));
    assert!(names.contains(&"forbidden-surface"));
    assert!(names.contains(&"unlabeled"));
    assert!(names.contains(&"duplicate-title"));
}

#[test]
fn runs_single_and_all_classify_output_paths() {
    let snapshot = snapshot();
    let single = ClassifyOptions {
        queue: options(OutputFormat::Text),
        number: Some(1),
        all: false,
        print: false,
        name: None,
    };
    run_classify(&snapshot, &single, &ClassifyRunContext::default()).expect("single classify");

    let all = ClassifyOptions {
        queue: options(OutputFormat::Json),
        number: None,
        all: true,
        print: false,
        name: Some("unit-classify".to_string()),
    };
    let context = ClassifyRunContext {
        rate: Some(RateReport {
            before: RateLimitSnapshot {
                remaining: 100,
                limit: 5000,
                reset_at: "2026-05-25T05:00:00Z".to_string(),
            },
            after: RateLimitSnapshot {
                remaining: 90,
                limit: 5000,
                reset_at: "2026-05-25T05:00:00Z".to_string(),
            },
            cost: Some(10),
        }),
        ..ClassifyRunContext::default()
    };
    run_classify(&snapshot, &all, &context).expect("all classify");
}

fn options(format: OutputFormat) -> QueueOptions {
    QueueOptions {
        repo: None,
        config: None,
        cache_dir: None,
        limit: 30,
        format,
        offline: false,
        refresh: false,
        pr_include_closed: false,
        include_drafts: false,
        lane: None,
        bucket: None,
        author: None,
        log_level: LogLevel::Off,
    }
}

fn snapshot() -> FactSnapshot {
    FactSnapshot {
        repo: "nexu-io/open-design".to_string(),
        fetched_at: "1".to_string(),
        source: SnapshotSource::GhFacts,
        warnings: Vec::new(),
        meta: vec![
            PrMeta {
                number: 1,
                title: "same title".to_string(),
                author: Some("alice".to_string()),
                created_at: Some("2026-05-20T00:00:00Z".to_string()),
                updated_at: Some("2026-05-25T00:00:00Z".to_string()),
                is_draft: Some(false),
                review_decision: Some("APPROVED".to_string()),
                labels: vec![
                    "size/S".to_string(),
                    "risk/low".to_string(),
                    "type/bugfix".to_string(),
                ],
                maintainer_can_modify: Some(false),
                assignees: Vec::new(),
                head_ref_name: Some("fix/one".to_string()),
            },
            PrMeta {
                number: 2,
                title: "same title".to_string(),
                author: Some("alice".to_string()),
                created_at: Some("2026-05-20T00:00:00Z".to_string()),
                updated_at: Some("2026-05-25T00:00:00Z".to_string()),
                is_draft: Some(false),
                review_decision: Some(String::new()),
                labels: Vec::new(),
                maintainer_can_modify: Some(true),
                assignees: Vec::new(),
                head_ref_name: Some("fix/two".to_string()),
            },
        ],
        stats: vec![
            PrStats {
                number: 1,
                additions: Some(1),
                deletions: Some(1),
                changed_files: Some(1),
                head_ref_name: Some("fix/one".to_string()),
                head_ref_oid: Some("head-new".to_string()),
                base_ref_name: Some("main".to_string()),
                mergeable: Some("MERGEABLE".to_string()),
                merge_state_status: Some("CLEAN".to_string()),
            },
            PrStats {
                number: 2,
                additions: Some(1),
                deletions: Some(1),
                changed_files: Some(1),
                head_ref_name: Some("fix/two".to_string()),
                head_ref_oid: Some("head-two".to_string()),
                base_ref_name: Some("main".to_string()),
                mergeable: Some("CONFLICTING".to_string()),
                merge_state_status: Some("DIRTY".to_string()),
            },
        ],
        files: vec![
            PrFiles {
                number: 1,
                files: Vec::new(),
            },
            PrFiles {
                number: 2,
                files: vec![FileChange {
                    path: "apps/nextjs/src/page.tsx".to_string(),
                    additions: Some(1),
                    deletions: Some(1),
                    change_type: Some("ADDED".to_string()),
                }],
            },
        ],
        reviews: vec![Review {
            number: 1,
            author: Some("reviewer".to_string()),
            body: "<!-- looper:review -->".to_string(),
            state: "APPROVED".to_string(),
            submitted_at: Some("2026-05-25T00:00:00Z".to_string()),
            commit_oid: Some("old-head".to_string()),
        }],
        commits: vec![Commit {
            number: 1,
            oid: Some("head-new".to_string()),
            committed_date: Some("2026-05-25T00:00:00Z".to_string()),
            author_login: Some("alice".to_string()),
        }],
        comments: vec![Comment {
            number: 1,
            author: Some("alice".to_string()),
            body: "updated".to_string(),
            created_at: Some("2026-05-25T00:00:00Z".to_string()),
            source: "issue".to_string(),
        }],
        assignment_events: Vec::new(),
    }
}

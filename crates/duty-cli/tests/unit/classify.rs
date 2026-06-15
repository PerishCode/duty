use std::collections::HashSet;

use duty_core::{
    Comment, Commit, FactSnapshot, FileChange, PrFiles, PrMeta, PrStats, RateLimitSnapshot, Review,
    SnapshotSource,
};

use crate::{
    classify::{
        build_report_from_snapshot, run_classify, tags_for_number_with_org_members,
        ClassifyRunContext, RateReport,
    },
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
fn automation_stamped_maintainer_review_counts_as_reviewer_signal() {
    let snapshot = automation_stamped_review_snapshot();
    let tags = tags_for_number_with_org_members(&snapshot, 7, &HashSet::new()).expect("tags");

    assert!(
        tags.iter()
            .all(|tag| tag.name != "awaiting-reviewer-response-24h"),
        "maintainer reviews submitted through Looper still count as reviewer-side activity"
    );
    assert!(
        tags.iter()
            .all(|tag| tag.name != "awaiting-author-response-24h"),
        "an APPROVED review is not an author-action request"
    );
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

#[test]
fn flags_author_cluster_when_open_pr_count_meets_threshold() {
    let snapshot = author_cluster_snapshot(7);
    let report = build_report_from_snapshot(&snapshot, &HashSet::new());

    let cluster_numbers = report
        .by_tag
        .get("author-cluster")
        .cloned()
        .unwrap_or_default();
    assert_eq!(cluster_numbers, (1..=7).collect::<Vec<_>>());

    for number in 1..=7 {
        let tags = report
            .by_number
            .get(&number.to_string())
            .expect("tags for PR");
        let cluster = tags
            .iter()
            .find(|tag| tag.name == "author-cluster")
            .expect("author-cluster tag");
        assert_eq!(cluster.cluster_size, Some(7));
    }

    assert_eq!(
        report
            .by_author
            .get("prolific")
            .cloned()
            .unwrap_or_default(),
        (1..=7).collect::<Vec<_>>()
    );
}

#[test]
fn does_not_flag_author_cluster_below_threshold() {
    let snapshot = author_cluster_snapshot(6);
    let report = build_report_from_snapshot(&snapshot, &HashSet::new());

    assert!(
        !report.by_tag.contains_key("author-cluster"),
        "author-cluster should not appear when an author has 6 PRs"
    );
    for number in 1..=6 {
        let tags = report
            .by_number
            .get(&number.to_string())
            .expect("tags for PR");
        assert!(tags.iter().all(|tag| tag.name != "author-cluster"));
    }
}

#[test]
fn cluster_size_is_serialized_only_when_present() {
    let snapshot = author_cluster_snapshot(7);
    let report = build_report_from_snapshot(&snapshot, &HashSet::new());
    let json = serde_json::to_string(&report).expect("serialize report");

    assert!(json.contains("\"clusterSize\":7"));
    assert!(json.contains("\"byAuthor\":{"));
    assert!(
        !json.contains("\"clusterSize\":null"),
        "skip_serializing_if should hide null clusterSize"
    );
}

fn author_cluster_snapshot(pr_count: u64) -> FactSnapshot {
    let meta = (1..=pr_count)
        .map(|number| PrMeta {
            number,
            title: format!("cluster pr {number}"),
            author: Some("prolific".to_string()),
            created_at: Some("2026-05-25T00:00:00Z".to_string()),
            updated_at: Some("2026-05-25T00:00:00Z".to_string()),
            is_draft: Some(false),
            review_decision: Some(String::new()),
            labels: vec![
                "size/S".to_string(),
                "risk/low".to_string(),
                "type/feature".to_string(),
            ],
            maintainer_can_modify: Some(true),
            assignees: Vec::new(),
            head_ref_name: Some(format!("feat/cluster-{number}")),
        })
        .collect::<Vec<_>>();
    let stats = (1..=pr_count)
        .map(|number| PrStats {
            number,
            additions: Some(1),
            deletions: Some(0),
            changed_files: Some(1),
            head_ref_name: Some(format!("feat/cluster-{number}")),
            head_ref_oid: Some(format!("head-{number}")),
            base_ref_name: Some("main".to_string()),
            mergeable: Some("MERGEABLE".to_string()),
            merge_state_status: Some("CLEAN".to_string()),
        })
        .collect::<Vec<_>>();
    let files = (1..=pr_count)
        .map(|number| PrFiles {
            number,
            files: vec![FileChange {
                path: format!("docs/cluster-{number}.md"),
                additions: Some(1),
                deletions: Some(0),
                change_type: Some("ADDED".to_string()),
            }],
        })
        .collect::<Vec<_>>();
    FactSnapshot {
        repo: "nexu-io/open-design".to_string(),
        fetched_at: "1".to_string(),
        source: SnapshotSource::GhFacts,
        warnings: Vec::new(),
        meta,
        stats,
        files,
        reviews: Vec::new(),
        commits: Vec::new(),
        comments: Vec::new(),
        assignment_events: Vec::new(),
    }
}

fn automation_stamped_review_snapshot() -> FactSnapshot {
    FactSnapshot {
        repo: "nexu-io/open-design".to_string(),
        fetched_at: "1".to_string(),
        source: SnapshotSource::GhFacts,
        warnings: Vec::new(),
        meta: vec![PrMeta {
            number: 7,
            title: "review signal".to_string(),
            author: Some("contributor".to_string()),
            created_at: Some("2099-01-01T00:00:00Z".to_string()),
            updated_at: Some("2099-01-01T00:20:00Z".to_string()),
            is_draft: Some(false),
            review_decision: Some(String::new()),
            labels: vec![
                "size/S".to_string(),
                "risk/low".to_string(),
                "type/bugfix".to_string(),
            ],
            maintainer_can_modify: Some(true),
            assignees: Vec::new(),
            head_ref_name: Some("fix/review-signal".to_string()),
        }],
        stats: vec![PrStats {
            number: 7,
            additions: Some(1),
            deletions: Some(1),
            changed_files: Some(1),
            head_ref_name: Some("fix/review-signal".to_string()),
            head_ref_oid: Some("head-new".to_string()),
            base_ref_name: Some("main".to_string()),
            mergeable: Some("MERGEABLE".to_string()),
            merge_state_status: Some("BLOCKED".to_string()),
        }],
        files: vec![PrFiles {
            number: 7,
            files: vec![FileChange {
                path: "apps/web/tests/components/FileViewer.test.tsx".to_string(),
                additions: Some(1),
                deletions: Some(1),
                change_type: Some("MODIFIED".to_string()),
            }],
        }],
        reviews: vec![Review {
            number: 7,
            author: Some("PerishCode".to_string()),
            body: "<!-- looper:review outcome=clean -->".to_string(),
            state: "APPROVED".to_string(),
            submitted_at: Some("2099-01-01T00:20:00Z".to_string()),
            commit_oid: Some("head-new".to_string()),
        }],
        commits: vec![Commit {
            number: 7,
            oid: Some("head-new".to_string()),
            committed_date: Some("2099-01-01T00:10:00Z".to_string()),
            author_login: Some("contributor".to_string()),
        }],
        comments: vec![Comment {
            number: 7,
            author: Some("contributor".to_string()),
            body: "updated".to_string(),
            created_at: Some("2099-01-01T00:10:00Z".to_string()),
            source: "issue".to_string(),
        }],
        assignment_events: Vec::new(),
    }
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

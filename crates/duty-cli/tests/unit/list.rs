use duty_core::{FactSnapshot, FileChange, PrFiles, PrMeta, PrStats, SnapshotSource};

use crate::{
    cli::{OutputFormat, QueueOptions},
    lane::Lane,
    list::{apply_filters, classify_list, print_list, Bucket},
};

#[test]
fn classifies_lane_bucket_and_labels_from_fact_snapshot() {
    let snapshot = snapshot();

    let prs = classify_list(&snapshot);

    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].lane, Lane::Contract);
    assert_eq!(prs[0].bucket, Bucket::MergeReady);
    assert_eq!(prs[0].risk.as_deref(), Some("low"));
    assert_eq!(prs[1].lane, Lane::Docs);
    assert_eq!(prs[1].bucket, Bucket::New);
}

#[test]
fn filters_by_lane_bucket_author_and_draft_state() {
    let prs = classify_list(&snapshot());
    let mut options = options();
    options.lane = Some("contract".to_string());
    options.bucket = Some("merge-ready".to_string());
    options.author = Some("alice".to_string());

    let filtered = apply_filters(&prs, &options);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].number, 1);
}

#[test]
fn prints_text_and_json_reports() {
    let prs = classify_list(&snapshot());

    print_list(&prs, prs.len(), OutputFormat::Text).expect("text report");
    print_list(&prs, prs.len(), OutputFormat::Json).expect("json report");
}

fn options() -> QueueOptions {
    QueueOptions {
        repo: None,
        config: None,
        cache_dir: None,
        limit: 30,
        format: OutputFormat::Text,
        offline: false,
        refresh: false,
        pr_include_closed: false,
        include_drafts: false,
        lane: None,
        bucket: None,
        author: None,
        log_level: crate::cli::LogLevel::Off,
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
                title: "contract change".to_string(),
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
                maintainer_can_modify: Some(true),
                assignees: Vec::new(),
                head_ref_name: Some("fix/contracts".to_string()),
            },
            PrMeta {
                number: 2,
                title: "docs change".to_string(),
                author: Some("bob".to_string()),
                created_at: Some("2026-05-20T00:00:00Z".to_string()),
                updated_at: Some("2026-05-25T00:00:00Z".to_string()),
                is_draft: Some(false),
                review_decision: Some(String::new()),
                labels: Vec::new(),
                maintainer_can_modify: Some(true),
                assignees: Vec::new(),
                head_ref_name: Some("docs/readme".to_string()),
            },
        ],
        stats: vec![
            PrStats {
                number: 1,
                additions: Some(5),
                deletions: Some(1),
                changed_files: Some(1),
                head_ref_name: Some("fix/contracts".to_string()),
                head_ref_oid: Some("abc".to_string()),
                base_ref_name: Some("main".to_string()),
                mergeable: Some("MERGEABLE".to_string()),
                merge_state_status: Some("CLEAN".to_string()),
            },
            PrStats {
                number: 2,
                additions: Some(5),
                deletions: Some(1),
                changed_files: Some(1),
                head_ref_name: Some("docs/readme".to_string()),
                head_ref_oid: Some("def".to_string()),
                base_ref_name: Some("main".to_string()),
                mergeable: Some("MERGEABLE".to_string()),
                merge_state_status: Some("UNKNOWN".to_string()),
            },
        ],
        files: vec![
            PrFiles {
                number: 1,
                files: vec![FileChange {
                    path: "packages/contracts/src/api/foo.ts".to_string(),
                    additions: Some(5),
                    deletions: Some(1),
                    change_type: Some("MODIFIED".to_string()),
                }],
            },
            PrFiles {
                number: 2,
                files: vec![FileChange {
                    path: "README.md".to_string(),
                    additions: Some(5),
                    deletions: Some(1),
                    change_type: Some("MODIFIED".to_string()),
                }],
            },
        ],
        reviews: Vec::new(),
        commits: Vec::new(),
        comments: Vec::new(),
        assignment_events: Vec::new(),
    }
}

use std::collections::HashSet;

use duty_core::{
    AssignmentEvent, Comment, Commit, FactSnapshot, PrFiles, PrMeta, PrStats, Review,
    SnapshotSource,
};

use crate::{
    assignment::{build_report, run_assignment},
    cli::{AssignmentOptions, LogLevel, OutputFormat, QueueOptions},
};

#[test]
fn builds_assignment_report_with_blockers_and_unassigned() {
    let report = build_report(&snapshot(), false, &HashSet::new()).expect("assignment report");

    assert_eq!(report.open_pr_total, 2);
    assert_eq!(report.assigned_count, 1);
    assert_eq!(report.unassigned_count, 1);
    let reviewer = report.by_assignee.get("reviewer").expect("reviewer bucket");
    assert_eq!(reviewer[0].number, 1);
    assert_eq!(reviewer[0].assigned_by.as_deref(), Some("maintainer"));
    assert_eq!(reviewer[0].status, "blocked");
    assert!(reviewer[0]
        .blockers
        .iter()
        .any(|blocker| blocker.contains("CHANGES_REQUESTED")));
}

#[test]
fn prints_text_and_json_assignment_reports() {
    let snapshot = snapshot();
    let text = options(OutputFormat::Text);
    run_assignment(
        &snapshot,
        &text,
        &HashSet::new(),
        Some("reviewer".to_string()),
    )
    .expect("assignment text");

    let json = options(OutputFormat::Json);
    run_assignment(&snapshot, &json, &HashSet::new(), None).expect("assignment json");
}

fn options(format: OutputFormat) -> AssignmentOptions {
    AssignmentOptions {
        queue: QueueOptions {
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
        },
        user: None,
        unassigned: true,
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
                title: "needs author update".to_string(),
                author: Some("alice".to_string()),
                created_at: Some("2026-05-20T00:00:00Z".to_string()),
                updated_at: Some("2026-05-25T00:00:00Z".to_string()),
                is_draft: Some(false),
                review_decision: Some("CHANGES_REQUESTED".to_string()),
                labels: vec![
                    "size/S".to_string(),
                    "risk/low".to_string(),
                    "type/bugfix".to_string(),
                ],
                maintainer_can_modify: Some(true),
                assignees: vec!["reviewer".to_string()],
                head_ref_name: Some("fix/one".to_string()),
            },
            PrMeta {
                number: 2,
                title: "unassigned docs".to_string(),
                author: Some("bob".to_string()),
                created_at: Some("2026-05-20T00:00:00Z".to_string()),
                updated_at: Some("2026-05-25T00:00:00Z".to_string()),
                is_draft: Some(false),
                review_decision: Some(String::new()),
                labels: Vec::new(),
                maintainer_can_modify: Some(true),
                assignees: Vec::new(),
                head_ref_name: Some("docs/two".to_string()),
            },
        ],
        stats: vec![
            PrStats {
                number: 1,
                additions: Some(5),
                deletions: Some(1),
                changed_files: Some(1),
                head_ref_name: Some("fix/one".to_string()),
                head_ref_oid: Some("head-one".to_string()),
                base_ref_name: Some("main".to_string()),
                mergeable: Some("MERGEABLE".to_string()),
                merge_state_status: Some("CLEAN".to_string()),
            },
            PrStats {
                number: 2,
                additions: Some(1),
                deletions: Some(1),
                changed_files: Some(1),
                head_ref_name: Some("docs/two".to_string()),
                head_ref_oid: Some("head-two".to_string()),
                base_ref_name: Some("main".to_string()),
                mergeable: Some("MERGEABLE".to_string()),
                merge_state_status: Some("UNKNOWN".to_string()),
            },
        ],
        files: vec![PrFiles {
            number: 1,
            files: Vec::new(),
        }],
        reviews: vec![Review {
            number: 1,
            author: Some("reviewer".to_string()),
            body: "Please fix the edge case.".to_string(),
            state: "CHANGES_REQUESTED".to_string(),
            submitted_at: Some("2026-05-25T00:00:00Z".to_string()),
            commit_oid: Some("head-one".to_string()),
        }],
        commits: vec![Commit {
            number: 1,
            oid: Some("head-one".to_string()),
            committed_date: Some("2026-05-24T00:00:00Z".to_string()),
            author_login: Some("alice".to_string()),
        }],
        comments: vec![Comment {
            number: 1,
            author: Some("reviewer".to_string()),
            body: "Inline detail.".to_string(),
            created_at: Some("2026-05-25T01:00:00Z".to_string()),
            source: "inline".to_string(),
        }],
        assignment_events: vec![AssignmentEvent {
            number: 1,
            kind: "ASSIGNED".to_string(),
            created_at: Some("2026-05-24T12:00:00Z".to_string()),
            actor: Some("maintainer".to_string()),
            assignee: Some("reviewer".to_string()),
        }],
    }
}

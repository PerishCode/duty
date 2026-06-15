use duty_core::{
    Comment, Commit, FileChange, PullRequestView, Review, SnapshotSource, StatusCheck,
};

use crate::{
    cli::OutputFormat,
    lane::Lane,
    view::{build_brief, print_view},
};

#[test]
fn builds_review_brief_from_single_pr_view() {
    let brief = build_brief(&view_snapshot());

    assert_eq!(brief.number, 42);
    assert_eq!(brief.lane, Lane::Contract);
    assert_eq!(brief.seams_touched, vec!["packages/contracts"]);
    assert_eq!(brief.top_files.len(), 1);
    assert_eq!(brief.filter_suppressed_file_count, 1);
    assert!(!brief.bot_only_approval);
    assert_eq!(brief.reviews.len(), 1);
    assert_eq!(brief.reviews[0].author, "reviewer");
    assert_eq!(brief.reviews[0].state, "APPROVED");
    assert_eq!(brief.comments.len(), 1);
    assert_eq!(brief.checks[0].workflow, "guard");
    assert_eq!(brief.checks[0].failing, 1);
}

#[test]
fn prints_text_and_json_view_reports() {
    let snapshot = view_snapshot();

    print_view(&snapshot, OutputFormat::Text).expect("text view");
    print_view(&snapshot, OutputFormat::Json).expect("json view");
}

fn view_snapshot() -> PullRequestView {
    PullRequestView {
        repo: "nexu-io/open-design".to_string(),
        fetched_at: "1".to_string(),
        source: SnapshotSource::GhView,
        warnings: vec!["inline review comments fetch failed: demo".to_string()],
        number: 42,
        url: "https://github.com/nexu-io/open-design/pull/42".to_string(),
        title: "contract brief".to_string(),
        body: "## Why\nUseful context".to_string(),
        state: "OPEN".to_string(),
        author: Some("alice".to_string()),
        created_at: Some("2026-05-20T00:00:00Z".to_string()),
        updated_at: Some("2026-05-25T00:00:00Z".to_string()),
        is_draft: Some(false),
        review_decision: Some("APPROVED".to_string()),
        merge_state_status: Some("BLOCKED".to_string()),
        labels: vec![
            "size/S".to_string(),
            "risk/low".to_string(),
            "type/bugfix".to_string(),
        ],
        additions: Some(11),
        deletions: Some(2),
        changed_files: Some(2),
        base_ref_name: Some("main".to_string()),
        head_ref_name: Some("fix/contracts".to_string()),
        head_ref_oid: Some("head-new".to_string()),
        maintainer_can_modify: Some(true),
        assignees: vec!["reviewer".to_string()],
        files: vec![
            FileChange {
                path: "packages/contracts/src/api/foo.ts".to_string(),
                additions: Some(10),
                deletions: Some(1),
                change_type: Some("MODIFIED".to_string()),
            },
            FileChange {
                path: "pnpm-lock.yaml".to_string(),
                additions: Some(1),
                deletions: Some(1),
                change_type: Some("MODIFIED".to_string()),
            },
        ],
        status_check_rollup: vec![
            StatusCheck {
                typename: Some("CheckRun".to_string()),
                name: Some("guard".to_string()),
                workflow_name: Some("guard".to_string()),
                conclusion: Some("FAILURE".to_string()),
                status: Some("COMPLETED".to_string()),
                state: None,
                context: None,
            },
            StatusCheck {
                typename: Some("CheckRun".to_string()),
                name: Some("lint".to_string()),
                workflow_name: Some("lint".to_string()),
                conclusion: Some("SUCCESS".to_string()),
                status: Some("COMPLETED".to_string()),
                state: None,
                context: None,
            },
        ],
        reviews: vec![Review {
            number: 42,
            author: Some("reviewer".to_string()),
            body: "<!-- looper:review -->".to_string(),
            state: "APPROVED".to_string(),
            submitted_at: Some("2026-05-25T00:00:00Z".to_string()),
            commit_oid: Some("head-new".to_string()),
        }],
        comments: vec![Comment {
            number: 42,
            author: Some("reviewer".to_string()),
            body: "Please check the contract shape.".to_string(),
            created_at: Some("2026-05-25T00:00:00Z".to_string()),
            source: "inline".to_string(),
        }],
        commits: vec![Commit {
            number: 42,
            oid: Some("head-new".to_string()),
            committed_date: Some("2026-05-25T00:00:00Z".to_string()),
            author_login: Some("alice".to_string()),
        }],
    }
}

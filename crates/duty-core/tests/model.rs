use duty_core::{FactSnapshot, PrMeta, SnapshotSource};

#[test]
fn fact_snapshot_exposes_queue_view_from_meta() {
    let snapshot = FactSnapshot {
        repo: "nexu-io/open-design".to_string(),
        fetched_at: "1".to_string(),
        source: SnapshotSource::GhFacts,
        warnings: Vec::new(),
        meta: vec![PrMeta {
            number: 2856,
            title: "fix(daemon): run Trae CLI ACP with yolo".to_string(),
            author: Some("JasonYang0104".to_string()),
            created_at: Some("2026-05-25T03:11:18Z".to_string()),
            updated_at: Some("2026-05-25T03:13:19Z".to_string()),
            is_draft: Some(false),
            review_decision: Some("REVIEW_REQUIRED".to_string()),
            labels: vec!["type/fix".to_string()],
            maintainer_can_modify: Some(true),
            assignees: Vec::new(),
            head_ref_name: Some("fix/trae-cli-yolo-acp".to_string()),
        }],
        stats: Vec::new(),
        files: Vec::new(),
        reviews: Vec::new(),
        commits: Vec::new(),
        comments: Vec::new(),
        assignment_events: Vec::new(),
    };

    let queue = snapshot.queue_prs();

    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].number, 2856);
    assert_eq!(queue[0].author.as_deref(), Some("JasonYang0104"));
    assert_eq!(queue[0].head_ref.as_deref(), Some("fix/trae-cli-yolo-acp"));
}

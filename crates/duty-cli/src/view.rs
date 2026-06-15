use std::collections::{BTreeSet, HashMap};

use duty_core::{FileChange, PullRequestView, Review, StatusCheck};
use serde::Serialize;

use crate::{
    bot::{
        condense, is_bot_authored, is_bot_login, is_bot_only_approval, latest_reviews_by_author,
    },
    cli::OutputFormat,
    lane::{derive_forbidden, derive_lane, derive_seams, is_noisy_file, Lane},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewBrief {
    pub(crate) number: u64,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) state: String,
    pub(crate) review_decision: String,
    pub(crate) merge_state_status: String,
    pub(crate) is_draft: bool,
    pub(crate) author: String,
    pub(crate) branch: BranchBrief,
    pub(crate) age: AgeBrief,
    pub(crate) labels: LabelBrief,
    pub(crate) diff: DiffBrief,
    pub(crate) lane: Lane,
    pub(crate) lane_hits: Vec<Lane>,
    pub(crate) forbidden: Vec<String>,
    pub(crate) seams_touched: Vec<String>,
    pub(crate) top_files: Vec<TopFileBrief>,
    pub(crate) filter_suppressed_file_count: usize,
    pub(crate) lane_rules: Vec<String>,
    pub(crate) validation: Vec<ValidationCommand>,
    pub(crate) reviews: Vec<ReviewBrief>,
    pub(crate) review_count_total: usize,
    pub(crate) bot_only_approval: bool,
    pub(crate) comments: Vec<CommentBrief>,
    pub(crate) comment_count_total: usize,
    pub(crate) checks: Vec<CheckBrief>,
    pub(crate) body_preview: String,
    pub(crate) body_chars: usize,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BranchBrief {
    pub(crate) head: String,
    pub(crate) base: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgeBrief {
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) age_days: u64,
    pub(crate) stale_days: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LabelBrief {
    pub(crate) size: Option<String>,
    pub(crate) risk: Option<String>,
    #[serde(rename = "type")]
    pub(crate) pr_type: Option<String>,
    pub(crate) all: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffBrief {
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) changed_files: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopFileBrief {
    pub(crate) path: String,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) change_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationCommand {
    pub(crate) command: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewBrief {
    pub(crate) author: String,
    pub(crate) state: String,
    pub(crate) submitted_at: String,
    pub(crate) body: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentBrief {
    pub(crate) author: String,
    pub(crate) created_at: String,
    pub(crate) body: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckBrief {
    pub(crate) workflow: String,
    pub(crate) passing: u64,
    pub(crate) failing: u64,
    pub(crate) pending: u64,
    pub(crate) total: u64,
}

pub(crate) fn print_view(view: &PullRequestView, format: OutputFormat) -> Result<(), String> {
    let brief = build_brief(view);
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&brief)
                    .map_err(|error| format!("failed to serialize view JSON: {error}"))?
            );
        }
        OutputFormat::Text => println!("{}", format_brief(&brief)),
    }
    Ok(())
}

pub(crate) fn build_brief(view: &PullRequestView) -> ViewBrief {
    let paths = view
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let (lane, lane_hits) = derive_lane(&paths);
    let review_decision = view.review_decision.clone().unwrap_or_default();
    let latest_per_reviewer = latest_reviews_by_author(&view.reviews);
    let now_days = current_epoch_days();
    let top_files = top_files(&view.files);
    let filter_suppressed_file_count = view.files.len().saturating_sub(top_files.len());

    ViewBrief {
        number: view.number,
        url: view.url.clone(),
        title: view.title.clone(),
        state: view.state.clone(),
        review_decision: review_decision.clone(),
        merge_state_status: view
            .merge_state_status
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string()),
        is_draft: view.is_draft.unwrap_or(false),
        author: view
            .author
            .clone()
            .unwrap_or_else(|| "(unknown)".to_string()),
        branch: BranchBrief {
            head: view.head_ref_name.clone().unwrap_or_default(),
            base: view
                .base_ref_name
                .clone()
                .unwrap_or_else(|| "main".to_string()),
        },
        age: AgeBrief {
            created_at: view.created_at.clone().unwrap_or_default(),
            updated_at: view.updated_at.clone().unwrap_or_default(),
            age_days: view
                .created_at
                .as_deref()
                .map(|created| days_since(created, now_days))
                .unwrap_or(0),
            stale_days: view
                .updated_at
                .as_deref()
                .map(|updated| days_since(updated, now_days))
                .unwrap_or(0),
        },
        labels: LabelBrief {
            size: label_by_prefix(&view.labels, "size/"),
            risk: label_by_prefix(&view.labels, "risk/"),
            pr_type: label_by_prefix(&view.labels, "type/"),
            all: view.labels.clone(),
        },
        diff: DiffBrief {
            additions: view.additions.unwrap_or(0),
            deletions: view.deletions.unwrap_or(0),
            changed_files: view.changed_files.unwrap_or(0),
        },
        lane,
        lane_hits,
        forbidden: derive_forbidden(&paths),
        seams_touched: derive_seams(&paths),
        top_files,
        filter_suppressed_file_count,
        lane_rules: lane_rules(lane, &paths),
        validation: derive_validation(&paths),
        reviews: human_review_briefs(&latest_per_reviewer),
        review_count_total: latest_per_reviewer.len(),
        bot_only_approval: is_bot_only_approval(&review_decision, &latest_per_reviewer),
        comments: human_comment_briefs(view),
        comment_count_total: view.comments.len(),
        checks: summarize_checks(&view.status_check_rollup),
        body_preview: condense(&view.body, 400),
        body_chars: view.body.chars().count(),
        warnings: view.warnings.clone(),
    }
}

fn top_files(files: &[FileChange]) -> Vec<TopFileBrief> {
    let mut rows = files
        .iter()
        .filter(|file| !is_noisy_file(&file.path))
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        let a_total = a.additions.unwrap_or(0) + a.deletions.unwrap_or(0);
        let b_total = b.additions.unwrap_or(0) + b.deletions.unwrap_or(0);
        b_total.cmp(&a_total).then(a.path.cmp(&b.path))
    });
    rows.into_iter()
        .take(8)
        .map(|file| TopFileBrief {
            path: file.path,
            additions: file.additions.unwrap_or(0),
            deletions: file.deletions.unwrap_or(0),
            change_type: file.change_type.unwrap_or_else(|| "UNKNOWN".to_string()),
        })
        .collect()
}

fn human_review_briefs(reviews: &[Review]) -> Vec<ReviewBrief> {
    let mut rows = reviews
        .iter()
        .filter(|review| !is_bot_login(review.author.as_deref()))
        .map(|review| ReviewBrief {
            author: review
                .author
                .clone()
                .unwrap_or_else(|| "(unknown)".to_string()),
            state: review.state.clone(),
            submitted_at: review.submitted_at.clone().unwrap_or_default(),
            body: condense(&review.body, 200),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
    rows
}

fn human_comment_briefs(view: &PullRequestView) -> Vec<CommentBrief> {
    let mut rows = view
        .comments
        .iter()
        .filter(|comment| !is_bot_authored(comment.author.as_deref(), &comment.body))
        .map(|comment| CommentBrief {
            author: comment
                .author
                .clone()
                .unwrap_or_else(|| "(unknown)".to_string()),
            created_at: comment.created_at.clone().unwrap_or_default(),
            body: condense(&comment.body, 200),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    rows.truncate(3);
    rows
}

fn summarize_checks(checks: &[StatusCheck]) -> Vec<CheckBrief> {
    let mut groups = HashMap::<String, CheckBrief>::new();
    for check in checks {
        let workflow = check
            .workflow_name
            .clone()
            .or_else(|| check.name.clone())
            .or_else(|| check.context.clone())
            .unwrap_or_else(|| "(unknown)".to_string());
        let bucket = groups.entry(workflow.clone()).or_insert(CheckBrief {
            workflow,
            passing: 0,
            failing: 0,
            pending: 0,
            total: 0,
        });
        bucket.total += 1;
        let conclusion = check
            .conclusion
            .as_deref()
            .or(check.state.as_deref())
            .or(check.status.as_deref())
            .unwrap_or("")
            .to_ascii_uppercase();
        match conclusion.as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => bucket.passing += 1,
            "FAILURE" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" => bucket.failing += 1,
            _ => bucket.pending += 1,
        }
    }
    let mut rows = groups.into_values().collect::<Vec<_>>();
    rows.sort_by(|a, b| b.failing.cmp(&a.failing).then(a.workflow.cmp(&b.workflow)));
    rows
}

fn derive_validation(paths: &[String]) -> Vec<ValidationCommand> {
    let mut cmds = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    add_validation(
        &mut cmds,
        &mut seen,
        "pnpm guard",
        "Open Design TS-first and JS allowlist gate",
    );
    add_validation(
        &mut cmds,
        &mut seen,
        "pnpm typecheck",
        "Open Design workspace-wide typecheck",
    );

    if touched(paths, "apps/web/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/web typecheck",
            "apps/web changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/web test",
            "apps/web changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/web build",
            "apps/web changed",
        );
    }
    if touched(paths, "apps/daemon/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/daemon typecheck",
            "apps/daemon changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/daemon test",
            "apps/daemon changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/daemon build",
            "apps/daemon changed",
        );
    }
    if touched(paths, "apps/desktop/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/desktop typecheck",
            "apps/desktop changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/desktop build",
            "apps/desktop changed",
        );
    }
    if touched(paths, "apps/packaged/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/packaged typecheck",
            "apps/packaged changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/packaged build",
            "apps/packaged changed",
        );
    }
    if touched(paths, "packages/contracts/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/contracts typecheck",
            "packages/contracts changed",
        );
    }
    if touched(paths, "packages/sidecar-proto/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/sidecar-proto typecheck",
            "sidecar-proto changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/sidecar-proto test",
            "sidecar-proto changed",
        );
    }
    if touched(paths, "packages/sidecar/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/sidecar typecheck",
            "packages/sidecar changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/sidecar test",
            "packages/sidecar changed",
        );
    }
    if touched(paths, "packages/platform/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/platform typecheck",
            "packages/platform changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/platform test",
            "packages/platform changed",
        );
    }
    if touched(paths, "tools/dev/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/tools-dev typecheck",
            "tools/dev changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/tools-dev build",
            "tools/dev changed",
        );
    }
    if touched(paths, "tools/pack/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/tools-pack typecheck",
            "tools/pack changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/tools-pack build",
            "tools/pack changed",
        );
    }
    if touched(paths, "tools/pr/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/tools-pr typecheck",
            "tools/pr changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/tools-pr build",
            "tools/pr changed",
        );
    }
    if touched_any(paths, &["e2e/specs/", "e2e/tests/", "e2e/lib/"]) {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm --filter @open-design/e2e typecheck",
            "e2e changed",
        );
        add_validation(
            &mut cmds,
            &mut seen,
            "(cd e2e && pnpm test specs)",
            "e2e specs are the PR smoke gate",
        );
    }
    if touched(paths, "e2e/ui/") {
        add_validation(
            &mut cmds,
            &mut seen,
            "(cd e2e && pnpm exec playwright test -c playwright.config.ts)",
            "Playwright UI changed",
        );
    }
    if paths.iter().any(|path| {
        contains_any_ascii_case(
            path,
            &["sidecar", "stamp", "namespace", "packaged", "tools-pack"],
        )
    }) {
        add_validation(
            &mut cmds,
            &mut seen,
            "# run inspect eval + screenshot for two concurrent namespaces",
            "stamp/namespace surface touched",
        );
    }
    if paths.iter().any(|path| {
        contains_any_ascii_case(path, &["tools-dev", "tools-pack", "log", "logger", ".tmp"])
    }) {
        add_validation(
            &mut cmds,
            &mut seen,
            "pnpm tools-dev logs --namespace <name> --json",
            "path/log surface touched",
        );
    }
    cmds
}

fn add_validation(
    cmds: &mut Vec<ValidationCommand>,
    seen: &mut BTreeSet<String>,
    command: &str,
    reason: &str,
) {
    if seen.insert(command.to_string()) {
        cmds.push(ValidationCommand {
            command: command.to_string(),
            reason: reason.to_string(),
        });
    }
}

fn lane_rules(lane: Lane, paths: &[String]) -> Vec<String> {
    let mut items = Vec::new();
    let skill_roots = roots(paths, "skills/");
    let design_roots = roots(paths, "design-systems/");
    match lane {
        Lane::Skill => {
            items.push(format!(
                "fact: skill roots touched - {}",
                join_or_none(&skill_roots)
            ));
            items.push(format!(
                "fact: SKILL.md present at every touched root - {}",
                yes_no(
                    skill_roots
                        .iter()
                        .all(|root| paths.contains(&format!("{root}/SKILL.md")))
                )
            ));
            items.push(format!(
                "fact: example.html present at a touched root - {}",
                yes_no(has_suffix(paths, "/example.html"))
            ));
            items.push(format!(
                "fact: references/checklist.md present at a touched root - {}",
                yes_no(has_suffix(paths, "/references/checklist.md"))
            ));
            items.push("rule [CONTRIBUTING.zh-CN.md skill hard line 1]: real hand-crafted example.html present".to_string());
            items.push("rule [CONTRIBUTING.zh-CN.md skill hard line 4]: references/checklist.md has at least the P0 gate".to_string());
            items.push("rule [CONTRIBUTING.zh-CN.md skill hard line 6]: self-contained assets and dependency scope".to_string());
        }
        Lane::DesignSystem => {
            items.push(format!(
                "fact: design-system roots touched - {}",
                join_or_none(&design_roots)
            ));
            items.push(format!(
                "fact: DESIGN.md present at a touched root - {}",
                yes_no(has_suffix(paths, "/DESIGN.md"))
            ));
            items.push("rule [code-review-guidelines.md 4.3]: first H1 is picker title; category line uses an existing dropdown group".to_string());
            items.push(
                "rule [CONTRIBUTING.zh-CN.md design-system hard line 1]: nine sections present"
                    .to_string(),
            );
            items.push(
                "rule [CONTRIBUTING.zh-CN.md design-system hard line 5]: ASCII slug only"
                    .to_string(),
            );
        }
        Lane::Craft => {
            items.push(
                "rule [code-review-guidelines.md 4.5]: universal brand-agnostic craft".to_string(),
            );
            items.push("rule [code-review-guidelines.md 4.5]: at least one shipping skill opts in, or a follow-up is named".to_string());
            items.push("reference: existing craft entry shapes - craft/typography.md, craft/color.md, craft/animation-discipline.md".to_string());
        }
        Lane::Contract => {
            items.push("rule [AGENTS.md Boundary constraints]: packages/contracts stays free of app/runtime internals".to_string());
            items.push("rule [AGENTS.md Boundary constraints]: sidecar process stamps have exactly five fields".to_string());
            items.push("rule [code-review-guidelines.md 4.2]: contract change lands before consumers, or in the same PR".to_string());
            items.push("rule [code-review-guidelines.md 4.2]: breaking persisted-format change needs migration and compatibility window".to_string());
        }
        Lane::Docs => {
            items.push("rule [code-review-guidelines.md 7]: documentation-only review checks relative links and anchors".to_string());
            items.push(
                "rule [code-review-guidelines.md 7]: no conflict with AGENTS.md chain".to_string(),
            );
            items.push(
                "rule [AGENTS.md Validation strategy]: pnpm guard and pnpm typecheck required"
                    .to_string(),
            );
        }
        Lane::Multi => {
            items.push(
                "rule [code-review-guidelines.md 3]: public seam motivates the cross-cut"
                    .to_string(),
            );
            items.push("rule [code-review-guidelines.md 3]: owning contract/protocol/primitive change lands first or in the same PR".to_string());
            items.push("rule [code-review-guidelines.md 3]: one clear primary owner".to_string());
        }
        Lane::Default | Lane::Unknown => {
            items.push("rule [AGENTS.md Boundary constraints]: tests live in sibling tests directories, not src".to_string());
            items.push("rule [AGENTS.md Boundary constraints]: shared logic lives in the owning package; no cross-app private imports".to_string());
            items.push("rule [AGENTS.md Boundary constraints]: shared web/daemon API DTOs live in packages/contracts".to_string());
            items.push(
                "reference: forbidden-surface scan in the Boundaries section is authoritative"
                    .to_string(),
            );
        }
    }
    items
}

fn format_brief(brief: &ViewBrief) -> String {
    let mut lines = Vec::new();
    let label_tags = [
        brief
            .labels
            .size
            .as_ref()
            .map(|size| format!("size/{size}")),
        brief
            .labels
            .risk
            .as_ref()
            .map(|risk| format!("risk/{risk}")),
        brief
            .labels
            .pr_type
            .as_ref()
            .map(|kind| format!("type/{kind}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");

    lines.push(format!("PR #{} - {}", brief.number, brief.title));
    lines.push(format!("  url        {}", empty_dash(&brief.url)));
    lines.push(format!("  author     {}", brief.author));
    lines.push(format!(
        "  branch     {} -> {}",
        brief.branch.head, brief.branch.base
    ));
    lines.push(format!(
        "  state      {} - {} - {}{}",
        brief.state,
        empty_review_decision(&brief.review_decision),
        brief.merge_state_status,
        if brief.is_draft { " - draft" } else { "" }
    ));
    lines.push(format!(
        "  age        created {}d ago - updated {}d ago",
        brief.age.age_days, brief.age.stale_days
    ));
    lines.push(format!(
        "  labels     {}",
        if label_tags.is_empty() {
            "(none)"
        } else {
            &label_tags
        }
    ));
    lines.push(format!(
        "  diff       +{} -{} across {} files",
        brief.diff.additions, brief.diff.deletions, brief.diff.changed_files
    ));
    if brief.bot_only_approval {
        lines.push(String::new());
        lines.push("  fact: bot-only approval - reviewDecision=APPROVED and every APPROVED review is bot-authored.".to_string());
        lines.push("  fact: zero APPROVED reviews authored by a non-bot account.".to_string());
    }
    if !brief.warnings.is_empty() {
        lines.push(String::new());
        for warning in &brief.warnings {
            lines.push(format!("  warning: {warning}"));
        }
    }
    lines.push(String::new());

    lines.push("-- Boundaries (lane / forbidden / seams) --".to_string());
    let lane_hits = if brief.lane_hits.len() > 1 {
        format!(
            "  (hits: {})",
            brief
                .lane_hits
                .iter()
                .map(|lane| lane.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        String::new()
    };
    lines.push(format!("  lane       {}{}", brief.lane.as_str(), lane_hits));
    lines.push(format!(
        "  forbidden  {}",
        if brief.forbidden.is_empty() {
            "[none]".to_string()
        } else {
            brief.forbidden.join(", ")
        }
    ));
    lines.push(format!(
        "  seams      {}",
        if brief.seams_touched.is_empty() {
            "[none]".to_string()
        } else {
            brief.seams_touched.join(", ")
        }
    ));
    lines.push(String::new());

    lines.push(format!(
        "-- Top files ({} shown, {} filtered by noisy-file rules) --",
        brief.top_files.len(),
        brief.filter_suppressed_file_count
    ));
    if brief.top_files.is_empty() {
        lines.push("  (no non-noisy files)".to_string());
    }
    for file in &brief.top_files {
        lines.push(format!(
            "  +{} -{}  {}  ({})",
            file.additions, file.deletions, file.path, file.change_type
        ));
    }
    lines.push(String::new());

    lines.push(format!("-- Lane rules ({}) --", brief.lane.as_str()));
    for item in &brief.lane_rules {
        lines.push(format!("  - {item}"));
    }
    lines.push(String::new());

    lines.push("-- Validation (derived from touched packages) --".to_string());
    for cmd in &brief.validation {
        lines.push(format!("  $ {}", cmd.command));
        lines.push(format!("      reason: {}", cmd.reason));
    }
    lines.push(String::new());

    lines.push(format!(
        "-- Recent reviews ({} human-shown of {} total) --",
        brief.reviews.len(),
        brief.review_count_total
    ));
    if brief.reviews.is_empty() {
        lines.push("  (no human reviews yet)".to_string());
    }
    for review in &brief.reviews {
        lines.push(format!(
            "  @{}  {}  {}",
            review.author, review.state, review.submitted_at
        ));
        lines.push(format!("      \"{}\"", review.body));
    }
    lines.push(String::new());

    lines.push(format!(
        "-- Recent comments ({} of {}) --",
        brief.comments.len(),
        brief.comment_count_total
    ));
    if brief.comments.is_empty() {
        lines.push("  (no human comments)".to_string());
    }
    for comment in &brief.comments {
        lines.push(format!("  @{}  {}", comment.author, comment.created_at));
        lines.push(format!("      \"{}\"", comment.body));
    }
    lines.push(String::new());

    lines.push("-- CI --".to_string());
    if brief.checks.is_empty() {
        lines.push("  (no checks reported)".to_string());
    }
    for group in &brief.checks {
        let symbol = if group.failing > 0 {
            "x"
        } else if group.pending > 0 {
            "."
        } else {
            "+"
        };
        lines.push(format!(
            "  {} {:<28} {}/{} pass{}{}",
            symbol,
            truncate(&group.workflow, 28),
            group.passing,
            group.total,
            if group.failing > 0 {
                format!(", {} fail", group.failing)
            } else {
                String::new()
            },
            if group.pending > 0 {
                format!(", {} pending", group.pending)
            } else {
                String::new()
            }
        ));
    }
    lines.push(String::new());

    lines.push(format!(
        "-- PR body (preview, {} chars total) --",
        brief.body_chars
    ));
    lines.push(if brief.body_preview.is_empty() {
        "  (empty body)".to_string()
    } else {
        format!("  {}", brief.body_preview)
    });

    lines.join("\n")
}

fn label_by_prefix(labels: &[String], prefix: &str) -> Option<String> {
    labels
        .iter()
        .find_map(|label| label.strip_prefix(prefix).map(str::to_string))
}

fn touched(paths: &[String], prefix: &str) -> bool {
    paths.iter().any(|path| path.starts_with(prefix))
}

fn touched_any(paths: &[String], prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| touched(paths, prefix))
}

fn contains_any_ascii_case(path: &str, needles: &[&str]) -> bool {
    let lower = path.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn roots(paths: &[String], prefix: &str) -> BTreeSet<String> {
    paths
        .iter()
        .filter_map(|path| {
            let rest = path.strip_prefix(prefix)?;
            let slug = rest.split('/').next()?;
            Some(format!("{prefix}{slug}"))
        })
        .collect()
}

fn join_or_none(values: &BTreeSet<String>) -> String {
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

fn has_suffix(paths: &[String], suffix: &str) -> bool {
    paths.iter().any(|path| path.ends_with(suffix))
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn empty_dash(value: &str) -> &str {
    if value.is_empty() {
        "-"
    } else {
        value
    }
}

fn empty_review_decision(value: &str) -> &str {
    if value.is_empty() {
        "REVIEW_REQUIRED"
    } else {
        value
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    out.push('~');
    out
}

fn current_epoch_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

fn days_since(iso: &str, now_days: i64) -> u64 {
    parse_yyyy_mm_dd(iso)
        .map(|(year, month, day)| {
            let days = days_from_civil(year, month, day);
            now_days.saturating_sub(days).max(0) as u64
        })
        .unwrap_or(0)
}

fn parse_yyyy_mm_dd(iso: &str) -> Option<(i64, i64, i64)> {
    if iso.len() < 10 {
        return None;
    }
    let year = iso.get(0..4)?.parse().ok()?;
    let month = iso.get(5..7)?.parse().ok()?;
    let day = iso.get(8..10)?.parse().ok()?;
    Some((year, month, day))
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month_prime + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

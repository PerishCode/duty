use duty_core::{FactSnapshot, QueueSnapshot, SnapshotSource};

use crate::cli::OutputFormat;

pub(crate) fn print_queue(snapshot: &QueueSnapshot, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(snapshot)
                .map_err(|error| format!("failed to serialize queue JSON: {error}"))?;
            println!("{text}");
        }
        OutputFormat::Text => {
            print_text(snapshot);
        }
    }
    Ok(())
}

pub(crate) fn print_facts(snapshot: &FactSnapshot, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(snapshot)
                .map_err(|error| format!("failed to serialize facts JSON: {error}"))?;
            println!("{text}");
        }
        OutputFormat::Text => {
            print_facts_text(snapshot);
        }
    }
    Ok(())
}

fn print_text(snapshot: &QueueSnapshot) {
    println!(
        "duty queue {} source={} count={} fetched={}",
        snapshot.repo,
        source_label(snapshot.source),
        snapshot.prs.len(),
        snapshot.fetched_at
    );
    for pr in &snapshot.prs {
        let state = pr.state.as_deref().unwrap_or("OPEN");
        let updated = pr.updated_at.as_deref().unwrap_or("-");
        let author = pr.author.as_deref().unwrap_or("-");
        println!(
            "#{:<6} {:<5} {:<20} {:<24} {}",
            pr.number, state, author, updated, pr.title
        );
    }
}

fn print_facts_text(snapshot: &FactSnapshot) {
    println!(
        "duty facts {} source={} meta={} stats={} files={} reviews={} commits={} comments={} assignments={} warnings={} fetched={}",
        snapshot.repo,
        source_label(snapshot.source),
        snapshot.meta.len(),
        snapshot.stats.len(),
        snapshot.files.len(),
        snapshot.reviews.len(),
        snapshot.commits.len(),
        snapshot.comments.len(),
        snapshot.assignment_events.len(),
        snapshot.warnings.len(),
        snapshot.fetched_at
    );
    for warning in &snapshot.warnings {
        println!("warning: {warning}");
    }
    for pr in snapshot.queue_prs() {
        let state = pr.state.as_deref().unwrap_or("OPEN");
        let updated = pr.updated_at.as_deref().unwrap_or("-");
        let author = pr.author.as_deref().unwrap_or("-");
        println!(
            "#{:<6} {:<5} {:<20} {:<24} {}",
            pr.number, state, author, updated, pr.title
        );
    }
}

fn source_label(source: SnapshotSource) -> &'static str {
    match source {
        SnapshotSource::GhJson => "gh-json",
        SnapshotSource::GhPlain => "gh-plain",
        SnapshotSource::GhFacts => "gh-facts",
        SnapshotSource::Cache => "cache",
    }
}

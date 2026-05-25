use std::path::PathBuf;

use duty_core::{
    Comment, Commit, FileChange, PullRequestView, Review, SnapshotSource, StatusCheck,
};
use tracing::debug;

use crate::cli::ViewOptions;
use crate::config::load_config;
use crate::github::{fetch_pr_head_check, fetch_pull_request_view};
use crate::pr_cache::{
    evict_pr_dir, include_closed_prs, join_view, now_epoch_seconds, read_pr_index,
    read_subresource, split_view, wipe_legacy_view_cache, write_pr_index, write_subresource,
    CacheMode, Envelope, JoinedParts, PrIndexEntry, PrMetadataPayload, RepoPaths, Subresource,
    DEFAULT_ROOT,
};

pub(crate) fn load(options: &ViewOptions) -> Result<PullRequestView, String> {
    let queue = &options.queue;
    let config = load_config(queue.config.as_deref())?;
    let repo = queue
        .repo
        .clone()
        .or(config.github.default_repo)
        .ok_or_else(|| {
            "missing repo; pass --repo owner/name or set github.defaultRepo in duty.json"
                .to_string()
        })?;
    let cache_root = queue
        .cache_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT));
    wipe_legacy_view_cache(&cache_root)?;
    let paths = RepoPaths::new(cache_root, &repo)?;
    let mode = CacheMode::from_flags(queue.offline, queue.refresh)?;
    let include_closed = include_closed_prs(queue.pr_include_closed);

    match mode {
        CacheMode::Offline => load_from_cache(&paths, options.number),
        CacheMode::Refresh => {
            let view = full_fetch(&repo, options.number)?;
            persist_and_finalize(&paths, &view, include_closed)?;
            Ok(view)
        }
        CacheMode::Incremental => load_incremental(&repo, options.number, &paths, include_closed),
    }
}

fn load_incremental(
    repo: &str,
    number: u64,
    paths: &RepoPaths,
    include_closed: bool,
) -> Result<PullRequestView, String> {
    let head = match fetch_pr_head_check(repo, number) {
        Ok(head) => head,
        Err(error) => {
            debug!(error = %error, "head-check failed; falling back to cache");
            return load_from_cache(paths, number)
                .map_err(|cache_error| format!("{error}; no usable cache ({cache_error})"));
        }
    };

    if !include_closed && is_closed_state(&head.state) {
        evict_pr_dir(&paths.pr_dir(number))?;
        remove_index_entry(paths, number)?;
        return Err(format!(
            "PR #{number} is {} and --pr-include-closed was not set; cache evicted",
            head.state
        ));
    }

    let index = read_pr_index(&paths.pr_index());
    let cache_is_fresh = match (index.get(&number), head.updated_at.as_deref()) {
        (Some(entry), Some(head_updated)) => {
            entry.updated_at.as_deref() == Some(head_updated) && entry.state == head.state
        }
        _ => false,
    };

    if cache_is_fresh {
        if let Ok(view) = load_from_cache(paths, number) {
            debug!(number = number, "cache fresh; skipping full fetch");
            return Ok(view);
        }
        // Index claimed freshness but reconstruction failed — fall through to refetch.
        debug!(
            number = number,
            "cache index fresh but reconstruction failed; refetching"
        );
    }

    let view = full_fetch(repo, number)?;
    persist_and_finalize(paths, &view, include_closed)?;
    Ok(view)
}

fn full_fetch(repo: &str, number: u64) -> Result<PullRequestView, String> {
    fetch_pull_request_view(repo, number)
}

fn persist_and_finalize(
    paths: &RepoPaths,
    view: &PullRequestView,
    include_closed: bool,
) -> Result<(), String> {
    if !include_closed && is_closed_state(&view.state) {
        evict_pr_dir(&paths.pr_dir(view.number))?;
        remove_index_entry(paths, view.number)?;
        return Ok(());
    }
    persist_subresources(paths, view)?;
    upsert_index_entry(paths, view)?;
    Ok(())
}

fn persist_subresources(paths: &RepoPaths, view: &PullRequestView) -> Result<(), String> {
    let split = split_view(view);
    let source = view.updated_at.clone();

    write_subresource(
        &paths.pr_subresource(view.number, Subresource::Metadata),
        &Envelope::new(split.metadata, source.clone()),
    )?;
    write_subresource(
        &paths.pr_subresource(view.number, Subresource::Files),
        &Envelope::new(split.files, source.clone()),
    )?;
    write_subresource(
        &paths.pr_subresource(view.number, Subresource::Checks),
        &Envelope::new(split.checks, source.clone()),
    )?;
    write_subresource(
        &paths.pr_subresource(view.number, Subresource::Reviews),
        &Envelope::new(split.reviews, source.clone()),
    )?;
    write_subresource(
        &paths.pr_subresource(view.number, Subresource::IssueComments),
        &Envelope::new(split.issue_comments, source.clone()),
    )?;
    write_subresource(
        &paths.pr_subresource(view.number, Subresource::ReviewComments),
        &Envelope::new(split.review_comments, source.clone()),
    )?;
    write_subresource(
        &paths.pr_subresource(view.number, Subresource::Commits),
        &Envelope::new(split.commits, source),
    )?;
    Ok(())
}

fn upsert_index_entry(paths: &RepoPaths, view: &PullRequestView) -> Result<(), String> {
    let mut index = read_pr_index(&paths.pr_index());
    index.insert(
        view.number,
        PrIndexEntry {
            number: view.number,
            state: view.state.clone(),
            updated_at: view.updated_at.clone(),
            labels: view.labels.clone(),
            fetched_at: now_epoch_seconds(),
        },
    );
    write_pr_index(&paths.pr_index(), &index)
}

fn remove_index_entry(paths: &RepoPaths, number: u64) -> Result<(), String> {
    let mut index = read_pr_index(&paths.pr_index());
    if index.remove(&number).is_some() {
        write_pr_index(&paths.pr_index(), &index)?;
    }
    Ok(())
}

fn load_from_cache(paths: &RepoPaths, number: u64) -> Result<PullRequestView, String> {
    let metadata: Envelope<PrMetadataPayload> =
        read_subresource(&paths.pr_subresource(number, Subresource::Metadata))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (metadata.json)"))?;
    let files: Envelope<Vec<FileChange>> =
        read_subresource(&paths.pr_subresource(number, Subresource::Files))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (files.json)"))?;
    let checks: Envelope<Vec<StatusCheck>> =
        read_subresource(&paths.pr_subresource(number, Subresource::Checks))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (checks.json)"))?;
    let reviews: Envelope<Vec<Review>> =
        read_subresource(&paths.pr_subresource(number, Subresource::Reviews))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (reviews.json)"))?;
    let issue_comments: Envelope<Vec<Comment>> =
        read_subresource(&paths.pr_subresource(number, Subresource::IssueComments))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (issue_comments.json)"))?;
    let review_comments: Envelope<Vec<Comment>> =
        read_subresource(&paths.pr_subresource(number, Subresource::ReviewComments))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (review_comments.json)"))?;
    let commits: Envelope<Vec<Commit>> =
        read_subresource(&paths.pr_subresource(number, Subresource::Commits))
            .ok_or_else(|| format!("offline cache miss for PR #{number} (commits.json)"))?;

    let fetched_at = metadata.fetched_at.clone();
    let mut view = join_view(JoinedParts {
        metadata: metadata.payload,
        files: files.payload,
        checks: checks.payload,
        reviews: reviews.payload,
        issue_comments: issue_comments.payload,
        review_comments: review_comments.payload,
        commits: commits.payload,
        fetched_at,
    });
    view.source = SnapshotSource::Cache;
    Ok(view)
}

fn is_closed_state(state: &str) -> bool {
    matches!(state, "CLOSED" | "MERGED")
}

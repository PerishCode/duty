use std::{env, process::exit};

mod bot;
mod cache;
mod classify;
mod cli;
mod config;
mod github;
mod lane;
mod list;
mod output;
mod view;

use classify::{run_classify, ClassifyRunContext, RateReport};
use cli::{help_text, parse_args, CliCommand, LogLevel};
use config::load_config;
use duty_core::SnapshotSource;
use github::{
    fetch_fact_snapshot, fetch_open_prs, fetch_org_members, fetch_pull_request_view,
    fetch_rate_limit,
};
use list::{classify_list, print_list};
use output::{print_facts, print_queue};
use tracing::debug;
use tracing_subscriber::filter::LevelFilter;
use view::print_view;

fn main() {
    match run() {
        Ok(exit_code) => exit(exit_code),
        Err(error) => {
            eprintln!("duty: {error}");
            exit(1);
        }
    }
}

fn run() -> Result<i32, String> {
    let command = parse_args(env::args().skip(1).collect())?;
    init_tracing(command.log_level())?;

    match command {
        CliCommand::Queue(options) => {
            let (repo, cache_dir) = resolve_repo_and_cache(&options)?;
            let cache_path = cache::snapshot_path(&cache_dir, &repo);
            let snapshot = if options.offline {
                let mut cached = cache::read_snapshot(&cache_path)?;
                cached.source = SnapshotSource::Cache;
                cached
            } else {
                match fetch_open_prs(&repo, options.limit) {
                    Ok(snapshot) => {
                        cache::write_snapshot(&cache_path, &snapshot)?;
                        snapshot
                    }
                    Err(error) => {
                        debug!(error = %error, "live GitHub fetch failed; trying cache");
                        let mut cached =
                            cache::read_snapshot(&cache_path).map_err(|cache_error| {
                                format!(
                                    "{error}; no usable cache at {} ({cache_error})",
                                    cache_path.display()
                                )
                            })?;
                        cached.source = SnapshotSource::Cache;
                        cached
                    }
                }
            };

            print_queue(&snapshot, options.format)?;
            Ok(0)
        }
        CliCommand::Facts(options) => {
            let (repo, cache_dir) = resolve_repo_and_cache(&options)?;
            let cache_path = cache::facts_path(&cache_dir, &repo);
            let snapshot = if options.offline {
                let mut cached = cache::read_facts(&cache_path)?;
                cached.source = SnapshotSource::Cache;
                cached
            } else {
                match fetch_fact_snapshot(&repo, options.limit) {
                    Ok(snapshot) => {
                        cache::write_facts(&cache_path, &snapshot)?;
                        snapshot
                    }
                    Err(error) => {
                        debug!(error = %error, "live GitHub facts fetch failed; trying cache");
                        let mut cached = cache::read_facts(&cache_path).map_err(|cache_error| {
                            format!(
                                "{error}; no usable cache at {} ({cache_error})",
                                cache_path.display()
                            )
                        })?;
                        cached.source = SnapshotSource::Cache;
                        cached
                    }
                }
            };

            print_facts(&snapshot, options.format)?;
            Ok(0)
        }
        CliCommand::List(options) => {
            let snapshot = load_fact_snapshot(&options)?;
            let classified = classify_list(&snapshot);
            let filtered = list::apply_filters(&classified, &options);
            print_list(&filtered, classified.len(), options.format)?;
            Ok(0)
        }
        CliCommand::Classify(options) => {
            let (repo, cache_dir) = resolve_repo_and_cache(&options.queue)?;
            let mut run_context = ClassifyRunContext::default();
            let rate_before = if options.all && !options.queue.offline {
                match fetch_rate_limit() {
                    Ok(snapshot) => Some(snapshot),
                    Err(error) => {
                        run_context
                            .warnings
                            .push(format!("rate-limit before fetch failed: {error}"));
                        None
                    }
                }
            } else {
                None
            };
            let snapshot = load_fact_snapshot_resolved(&options.queue, &repo, &cache_dir)?;
            if !options.queue.offline {
                match fetch_org_members(&repo) {
                    Ok(members) => run_context.org_members = members,
                    Err(error) => run_context
                        .warnings
                        .push(format!("org members fetch failed: {error}")),
                }
            }
            if let Some(before) = rate_before {
                match fetch_rate_limit() {
                    Ok(after) => {
                        let cost = if before.reset_at == after.reset_at {
                            Some(before.remaining as i64 - after.remaining as i64)
                        } else {
                            None
                        };
                        run_context.rate = Some(RateReport {
                            before,
                            after,
                            cost,
                        });
                    }
                    Err(error) => run_context
                        .warnings
                        .push(format!("rate-limit after fetch failed: {error}")),
                }
            }
            run_classify(&snapshot, &options, &run_context)?;
            Ok(0)
        }
        CliCommand::View(options) => {
            let view = load_pull_request_view(&options)?;
            print_view(&view, options.queue.format)?;
            Ok(0)
        }
        CliCommand::Help => {
            println!("{}", help_text());
            Ok(0)
        }
        CliCommand::Version => {
            println!("duty {}", build_version());
            Ok(0)
        }
    }
}

fn load_pull_request_view(
    options: &cli::ViewOptions,
) -> Result<duty_core::PullRequestView, String> {
    let (repo, cache_dir) = resolve_repo_and_cache(&options.queue)?;
    let cache_path = cache::view_path(&cache_dir, &repo, options.number);
    if options.queue.offline {
        let mut cached = cache::read_view(&cache_path)?;
        cached.source = SnapshotSource::Cache;
        return Ok(cached);
    }

    match fetch_pull_request_view(&repo, options.number) {
        Ok(snapshot) => {
            cache::write_view(&cache_path, &snapshot)?;
            Ok(snapshot)
        }
        Err(error) => {
            debug!(error = %error, "live GitHub view fetch failed; trying cache");
            let mut cached = cache::read_view(&cache_path).map_err(|cache_error| {
                format!(
                    "{error}; no usable cache at {} ({cache_error})",
                    cache_path.display()
                )
            })?;
            cached.source = SnapshotSource::Cache;
            Ok(cached)
        }
    }
}

fn load_fact_snapshot(options: &cli::QueueOptions) -> Result<duty_core::FactSnapshot, String> {
    let (repo, cache_dir) = resolve_repo_and_cache(options)?;
    load_fact_snapshot_resolved(options, &repo, &cache_dir)
}

fn load_fact_snapshot_resolved(
    options: &cli::QueueOptions,
    repo: &str,
    cache_dir: &std::path::Path,
) -> Result<duty_core::FactSnapshot, String> {
    let cache_path = cache::facts_path(cache_dir, repo);
    if options.offline {
        let mut cached = cache::read_facts(&cache_path)?;
        cached.source = SnapshotSource::Cache;
        return Ok(cached);
    }

    match fetch_fact_snapshot(repo, options.limit) {
        Ok(snapshot) => {
            cache::write_facts(&cache_path, &snapshot)?;
            Ok(snapshot)
        }
        Err(error) => {
            debug!(error = %error, "live GitHub facts fetch failed; trying cache");
            let mut cached = cache::read_facts(&cache_path).map_err(|cache_error| {
                format!(
                    "{error}; no usable cache at {} ({cache_error})",
                    cache_path.display()
                )
            })?;
            cached.source = SnapshotSource::Cache;
            Ok(cached)
        }
    }
}

fn resolve_repo_and_cache(
    options: &cli::QueueOptions,
) -> Result<(String, std::path::PathBuf), String> {
    let config = load_config(options.config.as_deref())?;
    let repo = options
        .repo
        .clone()
        .or(config.github.default_repo)
        .ok_or_else(|| {
            "missing repo; pass --repo owner/name or set github.defaultRepo in duty.json"
                .to_string()
        })?;
    let cache_dir = options
        .cache_dir
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(".tmp/duty/cache"));
    Ok((repo, cache_dir))
}

fn build_version() -> &'static str {
    option_env!("DUTY_BUILD_VERSION").unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")))
}

fn init_tracing(level: LogLevel) -> Result<(), String> {
    let filter = match level {
        LogLevel::Off => LevelFilter::OFF,
        LogLevel::Error => LevelFilter::ERROR,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Debug => LevelFilter::DEBUG,
        LogLevel::Trace => LevelFilter::TRACE,
    };
    if filter == LevelFilter::OFF {
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_max_level(filter)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| format!("failed to initialize logging: {error}"))
}

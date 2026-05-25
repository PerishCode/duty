use std::{env, process::exit};

mod cache;
mod cli;
mod config;
mod github;
mod output;

use cli::{help_text, parse_args, CliCommand, LogLevel};
use config::load_config;
use duty_core::SnapshotSource;
use github::fetch_open_prs;
use output::print_queue;
use tracing::debug;
use tracing_subscriber::filter::LevelFilter;

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

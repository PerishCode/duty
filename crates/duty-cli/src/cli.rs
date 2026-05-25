use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct QueueOptions {
    pub(crate) repo: Option<String>,
    pub(crate) config: Option<PathBuf>,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) limit: usize,
    pub(crate) format: OutputFormat,
    pub(crate) offline: bool,
    pub(crate) refresh: bool,
    pub(crate) pr_include_closed: bool,
    pub(crate) include_drafts: bool,
    pub(crate) lane: Option<String>,
    pub(crate) bucket: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) log_level: LogLevel,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ClassifyOptions {
    pub(crate) queue: QueueOptions,
    pub(crate) number: Option<u64>,
    pub(crate) all: bool,
    pub(crate) print: bool,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ViewOptions {
    pub(crate) queue: QueueOptions,
    pub(crate) number: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct AssignmentOptions {
    pub(crate) queue: QueueOptions,
    pub(crate) user: Option<String>,
    pub(crate) unassigned: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CliCommand {
    Queue(QueueOptions),
    Facts(QueueOptions),
    List(QueueOptions),
    Classify(ClassifyOptions),
    View(ViewOptions),
    Assignment(AssignmentOptions),
    Help,
    Version,
}

impl CliCommand {
    pub(crate) fn log_level(&self) -> LogLevel {
        match self {
            CliCommand::Queue(options) | CliCommand::Facts(options) | CliCommand::List(options) => {
                options.log_level
            }
            CliCommand::Classify(options) => options.queue.log_level,
            CliCommand::View(options) => options.queue.log_level,
            CliCommand::Assignment(options) => options.queue.log_level,
            CliCommand::Help | CliCommand::Version => LogLevel::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CommandMode {
    Queue,
    Facts,
    List,
    Classify,
    View,
    Assignment,
}

pub(crate) fn parse_args(args: Vec<String>) -> Result<CliCommand, String> {
    let mut repo = None;
    let mut config = None;
    let mut cache_dir = None;
    let mut limit = 30usize;
    let mut format = OutputFormat::Text;
    let mut offline = false;
    let mut refresh = false;
    let mut pr_include_closed = false;
    let mut include_drafts = false;
    let mut lane = None;
    let mut bucket = None;
    let mut author = None;
    let mut classify_number = None;
    let mut classify_all = false;
    let mut classify_print = false;
    let mut classify_name = None;
    let mut view_number = None;
    let mut assignment_user = None;
    let mut assignment_unassigned = false;
    let mut log_level = LogLevel::Off;
    let mut command_seen = false;
    let mut mode = CommandMode::Queue;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "queue" if !command_seen => {
                command_seen = true;
                mode = CommandMode::Queue;
            }
            "facts" if !command_seen => {
                command_seen = true;
                mode = CommandMode::Facts;
            }
            "list" if !command_seen => {
                command_seen = true;
                mode = CommandMode::List;
            }
            "classify" if !command_seen => {
                command_seen = true;
                mode = CommandMode::Classify;
            }
            "view" if !command_seen => {
                command_seen = true;
                mode = CommandMode::View;
            }
            "assignment" if !command_seen => {
                command_seen = true;
                mode = CommandMode::Assignment;
            }
            "help" | "--help" | "-h" => return Ok(CliCommand::Help),
            "version" | "--version" | "-V" => return Ok(CliCommand::Version),
            "--repo" => {
                repo = Some(next_value(&mut args, "--repo")?);
            }
            "--config" => {
                config = Some(PathBuf::from(next_value(&mut args, "--config")?));
            }
            "--cache-dir" => {
                cache_dir = Some(PathBuf::from(next_value(&mut args, "--cache-dir")?));
            }
            "--limit" => {
                limit = parse_limit(&next_value(&mut args, "--limit")?)?;
            }
            "--format" => {
                format = parse_format(&next_value(&mut args, "--format")?)?;
            }
            "--json" => {
                format = OutputFormat::Json;
            }
            "--offline" => {
                offline = true;
            }
            "--refresh" => {
                refresh = true;
            }
            "--pr-include-closed" => {
                pr_include_closed = true;
            }
            "--include-drafts" => {
                include_drafts = true;
            }
            "--lane" => {
                lane = Some(next_value(&mut args, "--lane")?);
            }
            "--bucket" => {
                bucket = Some(next_value(&mut args, "--bucket")?);
            }
            "--author" => {
                author = Some(next_value(&mut args, "--author")?);
            }
            "--user" => {
                assignment_user = Some(next_value(&mut args, "--user")?);
            }
            "--unassigned" => {
                assignment_unassigned = true;
            }
            "--all" => {
                classify_all = true;
            }
            "--print" => {
                classify_print = true;
            }
            "--name" => {
                classify_name = Some(next_value(&mut args, "--name")?);
            }
            "--log-level" => {
                log_level = parse_log_level(&next_value(&mut args, "--log-level")?)?;
            }
            other if other.starts_with("--repo=") => {
                repo = Some(other["--repo=".len()..].to_string());
            }
            other if other.starts_with("--config=") => {
                config = Some(PathBuf::from(&other["--config=".len()..]));
            }
            other if other.starts_with("--cache-dir=") => {
                cache_dir = Some(PathBuf::from(&other["--cache-dir=".len()..]));
            }
            other if other.starts_with("--limit=") => {
                limit = parse_limit(&other["--limit=".len()..])?;
            }
            other if other.starts_with("--format=") => {
                format = parse_format(&other["--format=".len()..])?;
            }
            other if other.starts_with("--name=") => {
                classify_name = Some(other["--name=".len()..].to_string());
            }
            other if other.starts_with("--lane=") => {
                lane = Some(other["--lane=".len()..].to_string());
            }
            other if other.starts_with("--bucket=") => {
                bucket = Some(other["--bucket=".len()..].to_string());
            }
            other if other.starts_with("--author=") => {
                author = Some(other["--author=".len()..].to_string());
            }
            other if other.starts_with("--user=") => {
                assignment_user = Some(other["--user=".len()..].to_string());
            }
            other if other.starts_with("--log-level=") => {
                log_level = parse_log_level(&other["--log-level=".len()..])?;
            }
            other if mode == CommandMode::Classify && classify_number.is_none() => {
                classify_number = Some(
                    other
                        .parse::<u64>()
                        .map_err(|_| format!("unsupported classify PR number: {other}"))?,
                );
            }
            other if mode == CommandMode::View && view_number.is_none() => {
                view_number = Some(
                    other
                        .parse::<u64>()
                        .map_err(|_| format!("unsupported view PR number: {other}"))?,
                );
            }
            other => {
                return Err(format!(
                    "unsupported duty argument: {other}\n\n{}",
                    help_text()
                ));
            }
        }
    }

    if offline && refresh {
        return Err("--offline and --refresh are mutually exclusive".to_string());
    }

    let options = QueueOptions {
        repo,
        config,
        cache_dir,
        limit,
        format,
        offline,
        refresh,
        pr_include_closed,
        include_drafts,
        lane,
        bucket,
        author,
        log_level,
    };

    Ok(match mode {
        CommandMode::Queue => CliCommand::Queue(options),
        CommandMode::Facts => CliCommand::Facts(options),
        CommandMode::List => CliCommand::List(options),
        CommandMode::Classify => CliCommand::Classify(ClassifyOptions {
            queue: options,
            number: classify_number,
            all: classify_all,
            print: classify_print,
            name: classify_name,
        }),
        CommandMode::View => CliCommand::View(ViewOptions {
            queue: options,
            number: view_number.filter(|number| *number > 0).ok_or_else(|| {
                "view requires a positive PR number, e.g. duty view 1180".to_string()
            })?,
        }),
        CommandMode::Assignment => CliCommand::Assignment(AssignmentOptions {
            queue: options,
            user: assignment_user,
            unassigned: assignment_unassigned,
        }),
    })
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_limit(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("unsupported limit: {value}"))?;
    if parsed == 0 {
        return Err("--limit must be greater than 0".to_string());
    }
    Ok(parsed)
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        other => Err(format!("unsupported output format: {other}")),
    }
}

fn parse_log_level(value: &str) -> Result<LogLevel, String> {
    match value {
        "off" => Ok(LogLevel::Off),
        "error" => Ok(LogLevel::Error),
        "warn" | "warning" => Ok(LogLevel::Warn),
        "info" => Ok(LogLevel::Info),
        "debug" => Ok(LogLevel::Debug),
        "trace" => Ok(LogLevel::Trace),
        other => Err(format!("unsupported log level: {other}")),
    }
}

pub(crate) fn help_text() -> &'static str {
    r#"duty

Personal maintainer-duty automation.

Commands:
  queue [--repo owner/name] [--limit <n>] [--format text|json]
        [--config <path>] [--cache-dir <path>] [--offline]
        [--log-level off|error|warn|info|debug|trace]
  facts [--repo owner/name] [--limit <n>] [--format text|json]
        [--config <path>] [--cache-dir <path>] [--offline]
        [--log-level off|error|warn|info|debug|trace]
  list  [--repo owner/name] [--limit <n>] [--format text|json]
        [--config <path>] [--cache-dir <path>] [--offline]
        [--lane <list>] [--bucket <list>] [--author <list>] [--include-drafts]
        [--log-level off|error|warn|info|debug|trace]
  classify [num] [--all] [--json] [--print] [--name <stem>]
        [--repo owner/name] [--limit <n>] [--config <path>]
        [--cache-dir <path>] [--offline]
        [--log-level off|error|warn|info|debug|trace]
  view <num> [--json] [--repo owner/name] [--config <path>]
        [--cache-dir <path>] [--offline] [--refresh] [--pr-include-closed]
        [--log-level off|error|warn|info|debug|trace]
  assignment [--json] [--user <login|me>] [--unassigned] [--include-drafts]
        [--repo owner/name] [--limit <n>] [--config <path>]
        [--cache-dir <path>] [--offline]
        [--log-level off|error|warn|info|debug|trace]
  help
  version

Config:
  --config <path>  Load this JSON config. If omitted, duty walks upward from
                   the current directory looking for duty.json.
  --repo <repo>    Overrides github.defaultRepo from config.

Queue behavior:
  queue first asks gh for a JSON PR list, then falls back to the plain
  tabular gh output when GitHub's API path flakes. Successful live snapshots
  are cached under .tmp/duty/cache by default. --offline reads only cache.

Fact behavior:
  facts fetches the chunked PR metadata, stats, files, reviews, comments,
  commits, and assignment event streams needed by list/classify/view/assignment
  parity work. Chunk failures are reported as warnings when a partial snapshot
  can still be produced.

List behavior:
  list classifies a facts snapshot into review-state buckets and path-derived
  lanes, with filters matching the tools-pr list surface.

Classify behavior:
  classify emits factual script-level tags from the facts snapshot. Use a PR
  number for one PR or --all for the full fetched queue. --json applies to
  single-PR stdout output; --all writes .tmp/duty/classify/<stem>.json and
  --print also dumps that full report to stdout. Full-queue reports include
  GraphQL rate-limit telemetry when live GitHub reads are available.

View behavior:
  view fetches one PR and emits a factual review brief with lane/boundary
  observations, denoised top files, validation hints, recent human reviews and
  comments, CI rollup, and PR body preview. The cache is the REST-style tree
  .tmp/duty/<org>/<repo>/prs/<num>/{metadata,files,reviews,...}.json. Default
  mode is incremental: a cheap head-check skips the full refetch when GitHub
  has not bumped the PR's updatedAt. --offline reads only cache. --refresh
  forces a full refetch and overwrite. By default a PR that GitHub reports as
  CLOSED or MERGED is evicted from the cache; pass --pr-include-closed or set
  DUTY_PR_INCLUDE_CLOSED=1 to keep it.

Assignment behavior:
  assignment groups open PRs by current assignee, derives assigned/idle timing
  from assignment timeline events plus assignee activity, and composes status
  and blocker lines from existing classify tags.

Project:
  Source: https://github.com/PerishCode/duty
"#
}

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
    pub(crate) log_level: LogLevel,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CliCommand {
    Queue(QueueOptions),
    Help,
    Version,
}

impl CliCommand {
    pub(crate) fn log_level(&self) -> LogLevel {
        match self {
            CliCommand::Queue(options) => options.log_level,
            CliCommand::Help | CliCommand::Version => LogLevel::Off,
        }
    }
}

pub(crate) fn parse_args(args: Vec<String>) -> Result<CliCommand, String> {
    let mut repo = None;
    let mut config = None;
    let mut cache_dir = None;
    let mut limit = 30usize;
    let mut format = OutputFormat::Text;
    let mut offline = false;
    let mut log_level = LogLevel::Off;
    let mut command_seen = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "queue" if !command_seen => {
                command_seen = true;
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
            "--offline" => {
                offline = true;
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
            other if other.starts_with("--log-level=") => {
                log_level = parse_log_level(&other["--log-level=".len()..])?;
            }
            other => {
                return Err(format!(
                    "unsupported duty argument: {other}\n\n{}",
                    help_text()
                ));
            }
        }
    }

    Ok(CliCommand::Queue(QueueOptions {
        repo,
        config,
        cache_dir,
        limit,
        format,
        offline,
        log_level,
    }))
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

Project:
  Source: https://github.com/PerishCode/duty
"#
}

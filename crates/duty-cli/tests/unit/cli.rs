use std::path::PathBuf;

use crate::cli::{parse_args, CliCommand, LogLevel, OutputFormat};

#[test]
fn defaults_to_queue_command() {
    let command = parse_args(Vec::new()).expect("parse args");

    let CliCommand::Queue(options) = command else {
        panic!("expected queue command");
    };
    assert_eq!(options.limit, 30);
    assert_eq!(options.format, OutputFormat::Text);
    assert_eq!(options.repo, None);
}

#[test]
fn exposes_command_log_level() {
    let command =
        parse_args(vec!["queue".to_string(), "--log-level=debug".to_string()]).expect("parse args");

    assert_eq!(command.log_level(), LogLevel::Debug);
}

#[test]
fn parses_queue_options() {
    let command = parse_args(vec![
        "queue".to_string(),
        "--repo".to_string(),
        "nexu-io/open-design".to_string(),
        "--limit=5".to_string(),
        "--format=json".to_string(),
        "--cache-dir".to_string(),
        ".tmp/cache".to_string(),
        "--offline".to_string(),
    ])
    .expect("parse args");

    let CliCommand::Queue(options) = command else {
        panic!("expected queue command");
    };
    assert_eq!(options.repo.as_deref(), Some("nexu-io/open-design"));
    assert_eq!(options.limit, 5);
    assert_eq!(options.format, OutputFormat::Json);
    assert_eq!(options.cache_dir, Some(PathBuf::from(".tmp/cache")));
    assert!(options.offline);
}

#[test]
fn rejects_zero_limit() {
    let error = parse_args(vec!["queue".to_string(), "--limit=0".to_string()])
        .expect_err("zero limit should fail");

    assert!(error.contains("--limit must be greater than 0"));
}

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
fn parses_facts_command() {
    let command = parse_args(vec![
        "facts".to_string(),
        "--repo=nexu-io/open-design".to_string(),
        "--limit".to_string(),
        "7".to_string(),
    ])
    .expect("parse args");

    let CliCommand::Facts(options) = command else {
        panic!("expected facts command");
    };
    assert_eq!(options.repo.as_deref(), Some("nexu-io/open-design"));
    assert_eq!(options.limit, 7);
}

#[test]
fn parses_classify_single_and_all_modes() {
    let single = parse_args(vec![
        "classify".to_string(),
        "42".to_string(),
        "--json".to_string(),
    ])
    .expect("parse single classify");
    let CliCommand::Classify(single) = single else {
        panic!("expected classify command");
    };
    assert_eq!(single.number, Some(42));
    assert!(!single.all);
    assert_eq!(single.queue.format, OutputFormat::Json);

    let all = parse_args(vec![
        "classify".to_string(),
        "--all".to_string(),
        "--print".to_string(),
        "--name=nightly".to_string(),
    ])
    .expect("parse all classify");
    let CliCommand::Classify(all) = all else {
        panic!("expected classify command");
    };
    assert!(all.all);
    assert!(all.print);
    assert_eq!(all.name.as_deref(), Some("nightly"));
}

#[test]
fn parses_view_command() {
    let command = parse_args(vec![
        "view".to_string(),
        "2856".to_string(),
        "--json".to_string(),
        "--repo=nexu-io/open-design".to_string(),
    ])
    .expect("parse view");

    let CliCommand::View(options) = command else {
        panic!("expected view command");
    };
    assert_eq!(options.number, 2856);
    assert_eq!(options.queue.format, OutputFormat::Json);
    assert_eq!(options.queue.repo.as_deref(), Some("nexu-io/open-design"));
}

#[test]
fn parses_assignment_command() {
    let command = parse_args(vec![
        "assignment".to_string(),
        "--json".to_string(),
        "--user".to_string(),
        "me".to_string(),
        "--unassigned".to_string(),
        "--include-drafts".to_string(),
        "--limit=10".to_string(),
    ])
    .expect("parse assignment");

    let CliCommand::Assignment(options) = command else {
        panic!("expected assignment command");
    };
    assert_eq!(options.user.as_deref(), Some("me"));
    assert!(options.unassigned);
    assert!(options.queue.include_drafts);
    assert_eq!(options.queue.limit, 10);
    assert_eq!(options.queue.format, OutputFormat::Json);
}

#[test]
fn rejects_zero_limit() {
    let error = parse_args(vec!["queue".to_string(), "--limit=0".to_string()])
        .expect_err("zero limit should fail");

    assert!(error.contains("--limit must be greater than 0"));
}

#[test]
fn parses_view_cache_mode_flags() {
    let refresh = parse_args(vec![
        "view".to_string(),
        "2862".to_string(),
        "--refresh".to_string(),
        "--pr-include-closed".to_string(),
    ])
    .expect("parse view --refresh");
    let CliCommand::View(options) = refresh else {
        panic!("expected view command");
    };
    assert!(options.queue.refresh);
    assert!(options.queue.pr_include_closed);
    assert!(!options.queue.offline);
}

#[test]
fn view_defaults_have_cache_flags_off() {
    let command = parse_args(vec!["view".to_string(), "2862".to_string()]).expect("parse view");
    let CliCommand::View(options) = command else {
        panic!("expected view command");
    };
    assert!(!options.queue.offline);
    assert!(!options.queue.refresh);
    assert!(!options.queue.pr_include_closed);
}

#[test]
fn offline_and_refresh_are_mutually_exclusive() {
    let error = parse_args(vec![
        "view".to_string(),
        "1".to_string(),
        "--offline".to_string(),
        "--refresh".to_string(),
    ])
    .expect_err("offline + refresh should fail");
    assert!(error.contains("mutually exclusive"));
}

use duty_core::parse_plain_pr_list;

#[test]
fn parses_tabular_gh_pr_list_output() {
    let input = concat!(
        "2856\tfix(daemon): run Trae CLI ACP with yolo\tJasonYang0104:fix/trae-cli-yolo-acp\tOPEN\t2026-05-25T03:11:18Z\n",
        "2849\tfix(web): show published design systems\tportseif:fix/card-status\tOPEN\t2026-05-24T23:33:41Z\n",
    );

    let prs = parse_plain_pr_list(input);

    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].number, 2856);
    assert_eq!(prs[0].author.as_deref(), Some("JasonYang0104"));
    assert_eq!(prs[1].head_ref.as_deref(), Some("portseif:fix/card-status"));
}

#[test]
fn skips_lines_without_a_numeric_pr_number() {
    let prs = parse_plain_pr_list("not-a-pr\tbad\n42\tgood\towner:branch\tOPEN\tnow\n");

    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 42);
}

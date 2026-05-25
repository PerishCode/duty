use crate::model::OpenPullRequest;

pub fn parse_plain_pr_list(input: &str) -> Vec<OpenPullRequest> {
    input
        .lines()
        .filter_map(parse_plain_pr_line)
        .collect::<Vec<_>>()
}

fn parse_plain_pr_line(line: &str) -> Option<OpenPullRequest> {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() < 2 {
        return None;
    }

    let number = fields.first()?.trim().parse::<u64>().ok()?;
    let title = fields.get(1)?.trim().to_string();
    let head_ref = fields.get(2).map(|value| value.trim().to_string());
    let state = fields.get(3).map(|value| value.trim().to_string());
    let updated_at = fields.get(4).map(|value| value.trim().to_string());
    let author = head_ref
        .as_deref()
        .and_then(|value| value.split_once(':').map(|(owner, _)| owner.to_string()));

    Some(OpenPullRequest {
        number,
        title,
        author,
        head_ref,
        state,
        updated_at,
        is_draft: None,
    })
}

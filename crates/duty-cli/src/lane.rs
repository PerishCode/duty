#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Lane {
    Skill,
    DesignSystem,
    Craft,
    Contract,
    Docs,
    Default,
    Multi,
    Unknown,
}

impl Lane {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Lane::Skill => "skill",
            Lane::DesignSystem => "design-system",
            Lane::Craft => "craft",
            Lane::Contract => "contract",
            Lane::Docs => "docs",
            Lane::Default => "default",
            Lane::Multi => "multi",
            Lane::Unknown => "unknown",
        }
    }

    pub(crate) fn tag(self) -> &'static str {
        match self {
            Lane::Contract => "CONTRACT",
            Lane::Skill => "SKILL",
            Lane::DesignSystem => "DSGN-SYS",
            Lane::Craft => "CRAFT",
            Lane::Docs => "DOCS",
            Lane::Multi => "MULTI",
            Lane::Default => "DEFAULT",
            Lane::Unknown => "UNKNOWN",
        }
    }

    pub(crate) fn order(self) -> usize {
        match self {
            Lane::Contract => 0,
            Lane::Default => 1,
            Lane::Skill => 2,
            Lane::DesignSystem => 3,
            Lane::Craft => 4,
            Lane::Docs => 5,
            Lane::Multi => 6,
            Lane::Unknown => 7,
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "skill" => Some(Lane::Skill),
            "design-system" => Some(Lane::DesignSystem),
            "craft" => Some(Lane::Craft),
            "contract" => Some(Lane::Contract),
            "docs" => Some(Lane::Docs),
            "default" => Some(Lane::Default),
            "multi" => Some(Lane::Multi),
            "unknown" => Some(Lane::Unknown),
            _ => None,
        }
    }
}

pub(crate) fn derive_lane(paths: &[String]) -> (Lane, Vec<Lane>) {
    let mut hits = Vec::<Lane>::new();
    let mut all_docs = !paths.is_empty();
    for path in paths {
        if is_skill(path) {
            push_unique(&mut hits, Lane::Skill);
        } else if is_design_system(path) {
            push_unique(&mut hits, Lane::DesignSystem);
        } else if is_craft(path) {
            push_unique(&mut hits, Lane::Craft);
        } else if is_contract(path) {
            push_unique(&mut hits, Lane::Contract);
        }
        if !is_docs_only(path) {
            all_docs = false;
        }
    }

    if hits.is_empty() && all_docs {
        return (Lane::Docs, vec![Lane::Docs]);
    }
    if hits.is_empty() {
        return (Lane::Default, vec![Lane::Default]);
    }
    if hits.len() == 1 {
        return (hits[0], hits);
    }
    (Lane::Multi, hits)
}

pub(crate) fn derive_forbidden(paths: &[String]) -> Vec<String> {
    let mut hits = Vec::new();
    if paths.iter().any(|path| path.starts_with("apps/nextjs/")) {
        hits.push("restores-apps/nextjs".to_string());
    }
    if paths
        .iter()
        .any(|path| path.starts_with("packages/shared/"))
    {
        hits.push("restores-packages/shared".to_string());
    }
    hits
}

pub(crate) fn derive_seams(paths: &[String]) -> Vec<String> {
    let mut seams = Vec::new();
    if paths
        .iter()
        .any(|path| path.starts_with("packages/contracts/"))
    {
        seams.push("packages/contracts".to_string());
    }
    if paths
        .iter()
        .any(|path| path.starts_with("packages/sidecar-proto/"))
    {
        seams.push("packages/sidecar-proto".to_string());
    }
    if paths.iter().any(|path| {
        path.starts_with("apps/daemon/src/")
            && contains_any_ascii_case(path, &["routes", "api", "sse", "http"])
    }) {
        seams.push("daemon HTTP/SSE routes".to_string());
    }
    if paths
        .iter()
        .any(|path| contains_any_ascii_case(path, &["migration", "schema", "sql"]))
    {
        seams.push("persisted schema".to_string());
    }
    if paths.iter().any(|path| path == "pnpm-workspace.yaml") {
        seams.push("workspace layout".to_string());
    }
    if paths.iter().any(|path| path == "package.json") {
        seams.push("root package.json".to_string());
    }
    seams
}

pub(crate) fn is_noisy_file(path: &str) -> bool {
    path == "pnpm-lock.yaml"
        || path == "CHANGELOG.md"
        || path.ends_with(".lock")
        || path.starts_with("generated/")
        || localized_doc_with_locale(path, "README")
        || localized_doc_with_locale(path, "CONTRIBUTING")
        || localized_doc_with_locale(path, "QUICKSTART")
}

fn push_unique(hits: &mut Vec<Lane>, lane: Lane) {
    if !hits.contains(&lane) {
        hits.push(lane);
    }
}

fn is_skill(path: &str) -> bool {
    path.strip_prefix("skills/")
        .and_then(|rest| rest.split_once('/'))
        .is_some()
}

fn is_design_system(path: &str) -> bool {
    path.strip_prefix("design-systems/")
        .and_then(|rest| rest.split_once('/'))
        .is_some()
}

fn is_craft(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("craft/") else {
        return false;
    };
    rest.ends_with(".md") && !rest.contains('/')
}

fn is_contract(path: &str) -> bool {
    path.starts_with("packages/contracts/")
        || path.starts_with("packages/sidecar-proto/")
        || (path.starts_with("apps/daemon/src/")
            && (path.contains("/routes") || path.contains("/api") || path.contains("/sse")))
}

fn is_docs_only(path: &str) -> bool {
    path.starts_with("docs/")
        || path == "CHANGELOG.md"
        || path == "TRANSLATIONS.md"
        || localized_doc(path, "README")
        || localized_doc(path, "CONTRIBUTING")
        || localized_doc(path, "QUICKSTART")
}

fn localized_doc(path: &str, stem: &str) -> bool {
    path == format!("{stem}.md") || (path.starts_with(&format!("{stem}.")) && path.ends_with(".md"))
}

fn localized_doc_with_locale(path: &str, stem: &str) -> bool {
    path.starts_with(&format!("{stem}.")) && path.ends_with(".md")
}

fn contains_any_ascii_case(path: &str, needles: &[&str]) -> bool {
    let lower = path.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

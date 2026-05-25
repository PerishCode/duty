use std::{fs, path::Path};

use duty_core::QueueSnapshot;

pub(crate) fn snapshot_path(cache_dir: &Path, repo: &str) -> std::path::PathBuf {
    cache_dir
        .join(repo.replace('/', "__"))
        .join("open-prs.json")
}

pub(crate) fn read_snapshot(path: &Path) -> Result<QueueSnapshot, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read cache {}: {error}", path.display()))?;
    serde_json::from_str::<QueueSnapshot>(&text)
        .map_err(|error| format!("failed to parse cache {}: {error}", path.display()))
}

pub(crate) fn write_snapshot(path: &Path, snapshot: &QueueSnapshot) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create cache directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(snapshot)
        .map_err(|error| format!("failed to serialize cache snapshot: {error}"))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("failed to write cache {}: {error}", path.display()))
}

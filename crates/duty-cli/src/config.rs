use std::{env, fs, path::Path};

use duty_core::DutyConfig;

pub(crate) fn load_config(path: Option<&Path>) -> Result<DutyConfig, String> {
    let Some(path) = path.map(|path| path.to_path_buf()).or_else(discover_config) else {
        return Ok(DutyConfig::default());
    };
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read config {}: {error}", path.display()))?;
    serde_json::from_str::<DutyConfig>(&text)
        .map_err(|error| format!("failed to parse config {}: {error}", path.display()))
}

fn discover_config() -> Option<std::path::PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        let candidate = dir.join("duty.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

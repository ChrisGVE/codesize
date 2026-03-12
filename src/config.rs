use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct LangLimits {
    pub file: usize,
    pub function: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ScanOverrides {
    skip_dirs: Option<Vec<String>>,
    skip_suffixes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct FileConfig {
    #[serde(default)]
    limits: HashMap<String, LangLimits>,
    #[serde(default)]
    scan: ScanOverrides,
}

pub struct Config {
    pub limits: HashMap<String, LangLimits>,
    pub skip_dirs: HashSet<String>,
    pub skip_suffixes: HashSet<String>,
}

fn default_limits() -> HashMap<String, LangLimits> {
    [
        ("Rust", 500, 80),
        ("TypeScript", 300, 40),
        ("JavaScript", 300, 40),
        ("Python", 300, 30),
        ("Go", 400, 60),
        ("Java", 300, 30),
        ("C", 500, 60),
        ("C++", 400, 60),
        ("Swift", 400, 50),
        ("Lua", 400, 50),
    ]
    .into_iter()
    .map(|(lang, file, function)| (lang.to_string(), LangLimits { file, function }))
    .collect()
}

fn default_skip_dirs() -> HashSet<String> {
    [".git", ".venv", "node_modules", "target", "dist", "build"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_skip_suffixes() -> HashSet<String> {
    [
        ".d.ts", ".min.js", ".min.ts", ".min.mjs", "_pb2.py", "_pb.go", ".pb.go",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Returns the config file path, preferring $XDG_CONFIG_HOME over ~/.config.
fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;
    Some(base.join("largecode").join("config.toml"))
}

/// Loads configuration from the XDG config file, merging with built-in defaults.
///
/// Language limits from the config file override individual entries in the
/// default table. If `scan.skip_dirs` or `scan.skip_suffixes` are present they
/// replace (not extend) the corresponding defaults.
pub fn load_config() -> Config {
    let file_cfg: FileConfig = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    let mut limits = default_limits();
    for (lang, overrides) in file_cfg.limits {
        limits.insert(lang, overrides);
    }

    let skip_dirs = file_cfg
        .scan
        .skip_dirs
        .map(|v| v.into_iter().collect())
        .unwrap_or_else(default_skip_dirs);

    let skip_suffixes = file_cfg
        .scan
        .skip_suffixes
        .map(|v| v.into_iter().collect())
        .unwrap_or_else(default_skip_suffixes);

    Config {
        limits,
        skip_dirs,
        skip_suffixes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_all_languages() {
        let cfg = load_config();
        for lang in &[
            "Rust",
            "Python",
            "Go",
            "TypeScript",
            "JavaScript",
            "Java",
            "C",
            "C++",
            "Swift",
            "Lua",
        ] {
            assert!(cfg.limits.contains_key(*lang), "Missing limits for {lang}");
        }
    }

    #[test]
    fn default_config_has_skip_dirs() {
        let cfg = load_config();
        assert!(cfg.skip_dirs.contains(".git"));
        assert!(cfg.skip_dirs.contains("node_modules"));
    }

    #[test]
    fn default_config_has_skip_suffixes() {
        let cfg = load_config();
        assert!(cfg.skip_suffixes.contains(".d.ts"));
        assert!(cfg.skip_suffixes.contains(".min.js"));
    }

    #[test]
    fn toml_override_replaces_single_limit() {
        let toml = r#"
[limits.Rust]
file = 999
function = 99
"#;
        let file_cfg: FileConfig = toml::from_str(toml).unwrap();
        let mut limits = default_limits();
        for (lang, ov) in file_cfg.limits {
            limits.insert(lang, ov);
        }
        let rust = &limits["Rust"];
        assert_eq!(rust.file, 999);
        assert_eq!(rust.function, 99);
        // Other languages unchanged
        assert_eq!(limits["Python"].file, 300);
    }
}

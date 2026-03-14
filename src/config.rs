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
    /// Respect .gitignore / .ignore / global git excludes (default: false).
    respect_gitignore: Option<bool>,
    /// Additional gitignore-style filenames to look for in every directory
    /// (e.g. [".npmignore", ".dockerignore"]).  Applied regardless of
    /// `respect_gitignore`.
    respect_ignore_files: Option<Vec<String>>,
    /// Paths to explicit gitignore-pattern files to apply during the walk
    /// (e.g. ["~/.globalignore"]).  Applied regardless of `respect_gitignore`.
    ignore_files: Option<Vec<String>>,
    /// Default CSV output path used when --output is not passed on the CLI.
    default_output_file: Option<String>,
    /// Replaces the built-in skip-directory list when present.
    skip_dirs: Option<Vec<String>>,
    /// Replaces the built-in skip-suffix list when present.
    skip_suffixes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct FileConfig {
    #[serde(default)]
    limits: HashMap<String, LangLimits>,
    #[serde(default)]
    scan: ScanOverrides,
    /// Maps file extensions (with leading dot, e.g. `".rb"`) to language names.
    /// Any language name is valid; if no tree-sitter grammar exists for it,
    /// only the file-length limit is enforced (no function analysis).
    #[serde(default)]
    languages: HashMap<String, String>,
}

pub struct Config {
    pub limits: HashMap<String, LangLimits>,
    /// User-defined extension → language name mappings (extension includes leading dot,
    /// lowercased). Consulted after the built-in extension table.
    pub extra_languages: HashMap<String, String>,
    pub skip_dirs: HashSet<String>,
    pub skip_suffixes: HashSet<String>,
    pub respect_gitignore: bool,
    pub respect_ignore_files: Vec<String>,
    pub ignore_files: Vec<String>,
    pub default_output_file: String,
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
    Some(base.join("codesize").join("config.toml"))
}

/// Loads configuration from the XDG config file, merging with built-in defaults.
///
/// - Individual language limits are overridden per-entry; others keep defaults.
/// - `skip_dirs` / `skip_suffixes` replace the defaults when present.
/// - `respect_ignore_files` / `ignore_files` extend the walk with additional
///   gitignore-style rules and are empty by default.
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

    // Normalize user-supplied extension keys to lowercase with a leading dot.
    let extra_languages = file_cfg
        .languages
        .into_iter()
        .map(|(ext, lang)| {
            let ext = ext.trim().to_lowercase();
            let ext = if ext.starts_with('.') {
                ext
            } else {
                format!(".{ext}")
            };
            (ext, lang)
        })
        .collect();

    Config {
        limits,
        extra_languages,
        skip_dirs,
        skip_suffixes,
        respect_gitignore: file_cfg.scan.respect_gitignore.unwrap_or(false),
        respect_ignore_files: file_cfg.scan.respect_ignore_files.unwrap_or_default(),
        ignore_files: file_cfg.scan.ignore_files.unwrap_or_default(),
        default_output_file: file_cfg
            .scan
            .default_output_file
            .unwrap_or_else(|| "codesize.csv".to_string()),
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
    fn default_config_gitignore_off() {
        let cfg = load_config();
        assert!(!cfg.respect_gitignore);
        assert!(cfg.respect_ignore_files.is_empty());
        assert!(cfg.ignore_files.is_empty());
    }

    #[test]
    fn default_output_file_is_csv() {
        let cfg = load_config();
        assert_eq!(cfg.default_output_file, "codesize.csv");
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
        assert_eq!(limits["Python"].file, 300);
    }
}

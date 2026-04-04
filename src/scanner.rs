use std::collections::HashSet;
use std::path::{Path, PathBuf};

use glob::glob;
use ignore::WalkBuilder;

use crate::config::Config;
use crate::parser::analyze_file;

#[derive(Debug)]
pub struct Finding {
    pub language: String,
    pub exception: String,
    pub function: String,
    pub codefile: String,
    pub lines: usize,
    pub limit: usize,
}

fn builtin_ext_to_lang(ext: &str) -> Option<&'static str> {
    match ext {
        ".rs" => Some("Rust"),
        ".ts" | ".tsx" => Some("TypeScript"),
        ".js" | ".jsx" => Some("JavaScript"),
        ".py" => Some("Python"),
        ".go" => Some("Go"),
        ".java" => Some("Java"),
        ".c" | ".h" => Some("C"),
        ".cpp" | ".cc" | ".cxx" | ".hpp" | ".hh" | ".hxx" | ".ipp" => Some("C++"),
        ".swift" => Some("Swift"),
        ".lua" => Some("Lua"),
        ".m" | ".mm" => Some("ObjC"),
        ".zig" => Some("Zig"),
        _ => None,
    }
}

/// Returns `true` if no directory component of `rel` is in `config.skip_dirs`.
/// Hidden-directory pruning is handled by WalkBuilder's `.hidden(true)` option.
fn in_allowed_dir(rel: &Path, config: &Config) -> bool {
    rel.components().all(|c| {
        let name = c.as_os_str().to_string_lossy();
        !config.skip_dirs.contains(name.as_ref())
    })
}

/// Applies per-entry filename/extension filters, yielding `(path, lang)`.
///
/// Built-in extensions are checked first; user-configured mappings in
/// `config.extra_languages` are consulted as a fallback for unknown extensions.
fn classify(path: PathBuf, config: &Config) -> Option<(PathBuf, String)> {
    let filename = path.file_name()?.to_string_lossy().to_lowercase();
    if config
        .skip_suffixes
        .iter()
        .any(|s| filename.ends_with(s.as_str()))
    {
        return None;
    }
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let ext_key = format!(".{ext}");
    let lang = builtin_ext_to_lang(&ext_key)
        .map(|s| s.to_string())
        .or_else(|| config.extra_languages.get(&ext_key).cloned())?;
    Some((path, lang))
}

/// Iterates over source files under `root`, applying all configured filters.
///
/// Ignore rules are additive:
/// - `config.respect_gitignore` enables standard `.gitignore` / `.ignore` /
///   global git-exclude handling.
/// - `config.respect_ignore_files` adds extra filenames (e.g. `.npmignore`)
///   that are treated as gitignore-style ignore files in every directory.
/// - `config.ignore_files` provides explicit ignore-pattern files to load.
/// - `config.skip_dirs` prunes named directories regardless of ignore rules.
/// - `config.skip_suffixes` filters by filename suffix.
pub fn iter_code_files<'a>(
    root: &'a Path,
    config: &'a Config,
) -> impl Iterator<Item = (PathBuf, String)> + 'a {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .require_git(false);

    for name in &config.respect_ignore_files {
        builder.add_custom_ignore_filename(name);
    }
    for file in &config.ignore_files {
        builder.add_ignore(file);
    }

    builder
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(move |e| {
            e.path()
                .strip_prefix(root)
                .map(|rel| in_allowed_dir(rel, config))
                .unwrap_or(false)
        })
        .filter_map(move |e| classify(e.path().to_path_buf(), config))
}

/// Resolves file/glob patterns relative to `root`.
///
/// When `recursive` is true each pattern is prefixed with `**/` so it
/// matches in all subdirectories.  Patterns without glob metacharacters
/// are treated as literal paths.  Results are deduplicated and filtered
/// through `classify` to ensure only recognised source files are returned.
pub fn resolve_patterns(
    root: &Path,
    patterns: &[String],
    recursive: bool,
    config: &Config,
) -> Vec<(PathBuf, String)> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for pattern in patterns {
        let full = if recursive {
            root.join("**").join(pattern)
        } else {
            root.join(pattern)
        };
        let glob_str = full.to_string_lossy().to_string();

        let Ok(entries) = glob(&glob_str) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.is_file() {
                continue;
            }
            if !seen.insert(entry.clone()) {
                continue;
            }
            if let Some(pair) = classify(entry, config) {
                results.push(pair);
            }
        }
    }
    results
}

/// Collects findings from an iterator of classified source files.
fn collect_findings(
    files: impl Iterator<Item = (PathBuf, String)>,
    root: &Path,
    tolerance_pct: f64,
    config: &Config,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (path, lang) in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        let Some(limits) = config.limits.get(&lang) else {
            continue;
        };
        let file_limit = limits.file;
        let func_limit = limits.function;

        let (line_count, functions) = analyze_file(&path, &lang);

        if line_count > file_limit {
            let excess_pct = (line_count as f64 - file_limit as f64) / file_limit as f64 * 100.0;
            if excess_pct > tolerance_pct {
                findings.push(Finding {
                    language: lang.to_string(),
                    exception: "file".to_string(),
                    function: String::new(),
                    codefile: rel.clone(),
                    lines: line_count,
                    limit: file_limit,
                });
            }
        }

        for (name, start, end) in functions {
            let func_lines = end - start + 1;
            if func_lines > func_limit {
                let excess_pct =
                    (func_lines as f64 - func_limit as f64) / func_limit as f64 * 100.0;
                if excess_pct > tolerance_pct {
                    findings.push(Finding {
                        language: lang.to_string(),
                        exception: "function".to_string(),
                        function: name,
                        codefile: rel.clone(),
                        lines: func_lines,
                        limit: func_limit,
                    });
                }
            }
        }
    }
    findings
}

/// Scans `root` and returns all findings that exceed the configured limits.
pub fn build_report(root: &Path, tolerance_pct: f64, config: &Config) -> Vec<Finding> {
    collect_findings(iter_code_files(root, config), root, tolerance_pct, config)
}

/// Scans files matching `patterns` and returns all findings that exceed limits.
pub fn build_report_from_patterns(
    root: &Path,
    patterns: &[String],
    recursive: bool,
    tolerance_pct: f64,
    config: &Config,
) -> Vec<Finding> {
    let files = resolve_patterns(root, patterns, recursive, config);
    collect_findings(files.into_iter(), root, tolerance_pct, config)
}

/// Writes `findings` as CSV sorted by (language, lines desc).
///
/// Pass `output = Some(path)` to write to a file, or `None` to write to stdout.
pub fn write_csv(findings: &mut Vec<Finding>, output: Option<&Path>) -> anyhow::Result<()> {
    findings.sort_by(|a, b| a.language.cmp(&b.language).then(b.lines.cmp(&a.lines)));

    fn write_records<W: std::io::Write>(
        w: &mut csv::Writer<W>,
        findings: &[Finding],
    ) -> anyhow::Result<()> {
        w.write_record([
            "language",
            "exception",
            "function",
            "codefile",
            "lines",
            "limit",
        ])?;
        for f in findings {
            w.write_record([
                &f.language,
                &f.exception,
                &f.function,
                &f.codefile,
                &f.lines.to_string(),
                &f.limit.to_string(),
            ])?;
        }
        w.flush()?;
        Ok(())
    }

    match output {
        Some(path) => write_records(&mut csv::Writer::from_path(path)?, findings),
        None => write_records(&mut csv::Writer::from_writer(std::io::stdout()), findings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;
    use std::fs;
    use tempfile::TempDir;

    fn make_tree(root: &Path, paths: &[&str]) {
        for rel in paths {
            let full = root.join(rel);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(&full, b"").unwrap();
        }
    }

    fn found_names(root: &Path) -> Vec<String> {
        let cfg = load_config();
        iter_code_files(root, &cfg)
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn d_ts_excluded() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["types/foo.d.ts", "src/bar.ts"]);
        let names = found_names(tmp.path());
        assert!(names.contains(&"bar.ts".to_string()));
        assert!(!names.contains(&"foo.d.ts".to_string()));
    }

    #[test]
    fn min_js_excluded() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["dist/app.min.js", "src/app.js"]);
        let names = found_names(tmp.path());
        assert!(names.contains(&"app.js".to_string()));
        assert!(!names.contains(&"app.min.js".to_string()));
    }

    #[test]
    fn pb2_py_excluded() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["proto/schema_pb2.py", "src/main.py"]);
        let names = found_names(tmp.path());
        assert!(names.contains(&"main.py".to_string()));
        assert!(!names.contains(&"schema_pb2.py".to_string()));
    }

    #[test]
    fn pb_go_excluded() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["proto/schema.pb.go", "cmd/main.go"]);
        let names = found_names(tmp.path());
        assert!(names.contains(&"main.go".to_string()));
        assert!(!names.contains(&"schema.pb.go".to_string()));
    }

    #[test]
    fn node_modules_skipped() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["node_modules/lib.ts", "src/lib.ts"]);
        let names = found_names(tmp.path());
        assert_eq!(names.iter().filter(|n| n.as_str() == "lib.ts").count(), 1);
    }

    #[test]
    fn dot_dirs_skipped() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &[".hidden/secret.py", "src/visible.py"]);
        let names = found_names(tmp.path());
        assert!(names.contains(&"visible.py".to_string()));
        assert!(!names.contains(&"secret.py".to_string()));
    }

    #[test]
    fn gitignore_excludes_ignored_file() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["src/main.py", "src/generated.py"]);
        fs::write(tmp.path().join(".gitignore"), b"generated.py\n").unwrap();
        let mut cfg = load_config();
        cfg.respect_gitignore = true;
        let names: Vec<String> = iter_code_files(tmp.path(), &cfg)
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"main.py".to_string()));
        assert!(!names.contains(&"generated.py".to_string()));
    }

    #[test]
    fn gitignore_off_includes_ignored_file() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["src/main.py", "src/generated.py"]);
        fs::write(tmp.path().join(".gitignore"), b"generated.py\n").unwrap();
        let names: Vec<String> = iter_code_files(tmp.path(), &load_config())
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"main.py".to_string()));
        assert!(names.contains(&"generated.py".to_string()));
    }

    #[test]
    fn respect_ignore_files_honoured() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["src/main.py", "src/vendor.py"]);
        fs::write(tmp.path().join(".myignore"), b"vendor.py\n").unwrap();
        let mut cfg = load_config();
        cfg.respect_ignore_files = vec![".myignore".to_string()];
        let names: Vec<String> = iter_code_files(tmp.path(), &cfg)
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"main.py".to_string()));
        assert!(!names.contains(&"vendor.py".to_string()));
    }

    #[test]
    fn explicit_ignore_file_honoured() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["src/main.py", "src/generated.py"]);
        let ignore_path = tmp.path().join("my.ignore");
        fs::write(&ignore_path, b"generated.py\n").unwrap();
        let mut cfg = load_config();
        cfg.ignore_files = vec![ignore_path.to_string_lossy().into_owned()];
        let names: Vec<String> = iter_code_files(tmp.path(), &cfg)
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"main.py".to_string()));
        assert!(!names.contains(&"generated.py".to_string()));
    }

    fn resolved_names(root: &Path, patterns: &[&str], recursive: bool) -> Vec<String> {
        let cfg = load_config();
        let pats: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        resolve_patterns(root, &pats, recursive, &cfg)
            .into_iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn resolve_literal_file() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["src/main.py", "src/lib.py"]);
        let names = resolved_names(tmp.path(), &["src/main.py"], false);
        assert_eq!(names, vec!["main.py"]);
    }

    #[test]
    fn resolve_glob_non_recursive() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["main.py", "lib.py", "sub/deep.py"]);
        let names = resolved_names(tmp.path(), &["*.py"], false);
        assert!(names.contains(&"main.py".to_string()));
        assert!(names.contains(&"lib.py".to_string()));
        assert!(!names.contains(&"deep.py".to_string()));
    }

    #[test]
    fn resolve_glob_recursive() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["main.py", "sub/deep.py", "sub/inner/more.py"]);
        let names = resolved_names(tmp.path(), &["*.py"], true);
        assert!(names.contains(&"main.py".to_string()));
        assert!(names.contains(&"deep.py".to_string()));
        assert!(names.contains(&"more.py".to_string()));
    }

    #[test]
    fn resolve_skips_unrecognised_extensions() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["main.py", "readme.txt"]);
        let names = resolved_names(tmp.path(), &["*"], false);
        assert!(names.contains(&"main.py".to_string()));
        assert!(!names.contains(&"readme.txt".to_string()));
    }

    #[test]
    fn resolve_deduplicates() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["src/main.py"]);
        let cfg = load_config();
        let pats = vec!["src/main.py".to_string(), "src/*.py".to_string()];
        let results = resolve_patterns(tmp.path(), &pats, false, &cfg);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn resolve_multiple_patterns() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["main.py", "app.rs", "lib.go"]);
        let names = resolved_names(tmp.path(), &["*.py", "*.rs"], false);
        assert!(names.contains(&"main.py".to_string()));
        assert!(names.contains(&"app.rs".to_string()));
        assert!(!names.contains(&"lib.go".to_string()));
    }

    #[test]
    fn resolve_skip_suffixes_applied() {
        let tmp = TempDir::new().unwrap();
        make_tree(tmp.path(), &["app.js", "app.min.js"]);
        let names = resolved_names(tmp.path(), &["*.js"], false);
        assert!(names.contains(&"app.js".to_string()));
        assert!(!names.contains(&"app.min.js".to_string()));
    }
}

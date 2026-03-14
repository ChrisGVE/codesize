use std::path::{Path, PathBuf};

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

/// Scans `root` and returns all findings that exceed the configured limits.
pub fn build_report(root: &Path, tolerance_pct: f64, config: &Config) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (path, lang) in iter_code_files(root, config) {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        let Some(limits) = config.limits.get(&lang) else {
            continue;
        };
        let factor = 1.0 + tolerance_pct / 100.0;
        let file_limit = (limits.file as f64 * factor) as usize;
        let func_limit = (limits.function as f64 * factor) as usize;

        let (line_count, functions) = analyze_file(&path, &lang);

        if line_count > file_limit {
            findings.push(Finding {
                language: lang.to_string(),
                exception: "file".to_string(),
                function: String::new(),
                codefile: rel.clone(),
                lines: line_count,
                limit: file_limit,
            });
        }

        for (name, start, end) in functions {
            let func_lines = end - start + 1;
            if func_lines > func_limit {
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
    findings
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
}

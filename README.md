# codesize

A fast, single-binary CLI tool that scans a source tree and reports files and
functions that exceed per-language size limits.  Results are written to a CSV
file (or printed to stdout) so they can be fed directly into task-management
workflows or CI checks.

Supported languages: **Rust · TypeScript · JavaScript · Python · Go · Java ·
C · C++ · Swift · Lua**

---

## Installation

### From source (requires Rust 1.75+)

```bash
cargo install --path .
```

The binary is installed as `codesize`.

---

## Quick start

```bash
# Scan the current directory, write codesize.csv
codesize

# Scan a specific project
codesize --root ~/projects/myapp

# Print results to stdout instead of a file
codesize --stdout

# Respect .gitignore files in the scanned tree
codesize --gitignore

# Allow 10 % headroom above every language limit
codesize --tolerance 10
```

---

## CLI reference

| Flag | Default | Description |
|---|---|---|
| `--root <path>` | `.` (cwd) | Directory to scan |
| `--output <path>` | `codesize.csv`* | CSV output file path |
| `--stdout` | off | Write CSV to stdout; ignores `--output` |
| `--tolerance <n>` | `0` | Percent tolerance added to every limit |
| `--gitignore` | off | Honour `.gitignore` / `.ignore` files; overrides config |

\* The default output filename can be changed via `default_output_file` in
`config.toml` (see below).

---

## Output format

The CSV has six columns:

```
language,exception,function,codefile,lines,limit
```

| Column | Values |
|---|---|
| `language` | Language name, e.g. `Rust` |
| `exception` | `file` or `function` |
| `function` | Function name; empty for file-level violations |
| `codefile` | Path relative to `--root` |
| `lines` | Measured line count |
| `limit` | Effective limit (after tolerance) |

Rows are sorted by language, then by line count descending.

**Notes**
- `.h` files are classified as C; `.hpp` / `.hh` / `.hxx` as C++.
- Function line counts include the signature and closing brace.
- Arrow functions in JavaScript / TypeScript are counted as functions.

---

## Configuration

codesize looks for a TOML config file at:

1. `$XDG_CONFIG_HOME/codesize/config.toml` (preferred)
2. `~/.config/codesize/config.toml` (fallback)

A fully documented starting-point is provided in
[`config.example.toml`](config.example.toml).  Copy it to the right location:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/codesize"
cp config.example.toml "${XDG_CONFIG_HOME:-$HOME/.config}/codesize/config.toml"
```

### `[scan]` options

| Key | Type | Default | Description |
|---|---|---|---|
| `respect_gitignore` | bool | `false` | Honour `.gitignore`, `.ignore`, global git excludes |
| `respect_ignore_files` | list | `[]` | Extra filenames (e.g. `.npmignore`) treated as ignore files in each directory |
| `ignore_files` | list | `[]` | Explicit ignore-pattern files to load unconditionally |
| `default_output_file` | string | `"codesize.csv"` | Output file when `--output` is not passed |
| `skip_dirs` | list | see below | Directory names to skip entirely (replaces built-in list) |
| `skip_suffixes` | list | see below | Filename suffixes to exclude (replaces built-in list) |

**`respect_ignore_files`** and **`ignore_files`** are additive with each other
and with `respect_gitignore`.  All three can be active simultaneously.

**Default `skip_dirs`:** `.git`, `.venv`, `node_modules`, `target`, `dist`, `build`

**Default `skip_suffixes`:** `.d.ts`, `.min.js`, `.min.ts`, `.min.mjs`,
`_pb2.py`, `_pb.go`, `.pb.go`

### `[limits.<Language>]` options

Override the file or function limit for any supported language.  Only the
entries you specify are changed; all others keep their built-in values.

```toml
[limits.Rust]
file     = 600   # was 500
function = 100   # was 80

[limits.Python]
function = 50    # was 30; leave file limit at 300
```

**Built-in limits**

| Language | File | Function |
|---|---:|---:|
| Rust | 500 | 80 |
| TypeScript | 300 | 40 |
| JavaScript | 300 | 40 |
| Python | 300 | 30 |
| Go | 400 | 60 |
| Java | 300 | 30 |
| C | 500 | 60 |
| C++ | 400 | 60 |
| Swift | 400 | 50 |
| Lua | 400 | 50 |

### Example config

```toml
[scan]
respect_gitignore     = true
respect_ignore_files  = [".npmignore", ".dockerignore"]
ignore_files          = ["~/.config/codesize/global.ignore"]
default_output_file   = "reports/size-report.csv"

[limits.Rust]
file     = 600
function = 100
```

---

## How it works

codesize uses [tree-sitter](https://tree-sitter.github.io/tree-sitter/) to
build an AST for each source file and walks it to find function boundaries.
This means it correctly counts only the lines belonging to each function,
including nested functions, without being tripped up by comments or strings.

File walking uses the [ignore](https://docs.rs/ignore) crate (the same engine
as `ripgrep`) when gitignore support is enabled.

---

## License

MIT — see [LICENSE](LICENSE).

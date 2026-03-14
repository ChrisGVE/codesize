# codesize

[![CI](https://github.com/ChrisGVE/codesize/actions/workflows/ci.yml/badge.svg)](https://github.com/ChrisGVE/codesize/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/ChrisGVE/codesize)](https://github.com/ChrisGVE/codesize/releases/latest)
[![Crates.io](https://img.shields.io/crates/v/codesize)](https://crates.io/crates/codesize)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, single-binary CLI tool that scans a source tree and reports files and
functions that exceed per-language size limits.  Results are written to a CSV
file (or printed to stdout) so they can be fed directly into task-management
workflows or CI checks.

Built-in grammars: **Rust · TypeScript · JavaScript · Python · Go · Java ·
C · C++ · Swift · Lua**

Any other language can be added via config.  If no tree-sitter grammar is
available for it, the file-length limit is still enforced.

---

## Installation

### Homebrew (macOS and Linux)

```bash
brew install ChrisGVE/tap/codesize
```

### From source (requires Rust 1.75+)

```bash
cargo install --git https://github.com/ChrisGVE/codesize
```

Or, from a local clone:

```bash
cargo install --path .
```

---

## Shell completion

Generate and install a completion script for your shell:

```bash
# zsh – add to ~/.zshrc or drop into your completions directory
codesize init zsh >> ~/.zshrc

# bash
codesize init bash >> ~/.bashrc

# fish
codesize init fish > ~/.config/fish/completions/codesize.fish

# elvish / powershell also supported
codesize init elvish
codesize init powershell
```

When installed via Homebrew, completions for bash, zsh, and fish are set up
automatically.

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
| `--fail` | off | Exit with status 1 if any violations are found (for CI) |

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

[`config.toml`](config.toml) in this repository is a fully documented template
listing every option with its built-in default.  Copy it to get started:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/codesize"
cp config.toml "${XDG_CONFIG_HOME:-$HOME/.config}/codesize/config.toml"
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

### `[languages]` — add or remap file extensions

Map any file extension to a language name.  Built-in extensions are checked
first; entries here serve as fallbacks.

```toml
[languages]
".rb"  = "Ruby"
".ex"  = "Elixir"
".exs" = "Elixir"
```

The extension must include the leading dot.  Leading dots and casing are
normalised automatically, so `rb`, `.rb`, and `.RB` all work.

**Grammar availability** determines how much analysis is possible:

| Situation | File limit | Function limit |
|---|---|---|
| Built-in grammar (Rust, Python, …) | yes | yes |
| No grammar (Ruby, Elixir, …) | yes | — (skipped) |

For languages without a grammar you still need a `[limits.<Name>]` entry, but
only `file` is meaningful (the `function` key is accepted and ignored).

### `[limits.<Language>]` options

Override the file or function limit for any language — built-in or custom.
Only the entries you specify are changed; all others keep their built-in values.

```toml
[limits.Rust]
file     = 600   # was 500
function = 100   # was 80

[limits.Python]
function = 50    # was 30; leave file limit at 300

# Custom language added via [languages] above:
[limits.Ruby]
file = 300
function = 30
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

---

## CI and pre-commit integration

### GitHub Actions

```yaml
# .github/workflows/codesize.yml
name: Code size check
on: [push, pull_request]
jobs:
  codesize:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install codesize
        run: cargo install codesize
      - name: Check code size
        run: codesize --root . --stdout --gitignore --fail
```

### pre-commit

Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: codesize
        name: codesize
        language: system
        entry: codesize
        args: [--stdout, --gitignore]
        pass_filenames: false
        always_run: true
```

Requires `codesize` to be installed and on `$PATH`.

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

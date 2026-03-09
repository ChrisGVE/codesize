# LargeCode

LargeCode scans the current working directory and writes a CSV report of files
and functions that exceed per-language size limits.

## Usage

```bash
LargeCode
```

Install with uv:

```bash
uv tool install .
```

Optional flags:

```bash
LargeCode --root /path/to/project --output report.csv
```

Tolerance example:

```bash
LargeCode --tolerance 10
```

Help (also shown on invalid flags/args):

```bash
LargeCode --help
```

## Output format

CSV with headers:

```
language,exception,function,codefile,lines,limit
```

Notes:
- `exception` is `file` or `function`.
- `function` is empty for file-level exceptions.
- `codefile` is relative to the `--root` (or cwd if omitted).
- For ambiguous extensions (e.g. `.h`), LargeCode treats them as C.

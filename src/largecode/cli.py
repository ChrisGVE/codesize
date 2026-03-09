import argparse
import csv
import os
import re
import sys
from dataclasses import dataclass
from typing import Callable, Iterable, List, Optional, Tuple


LIMITS = {
    "Rust": {"file": 500, "function": 80},
    "TypeScript": {"file": 300, "function": 40},
    "JavaScript": {"file": 300, "function": 40},
    "Python": {"file": 300, "function": 30},
    "Go": {"file": 400, "function": 60},
    "Java": {"file": 300, "function": 30},
    "C": {"file": 500, "function": 60},
    "C++": {"file": 400, "function": 60},
    "Swift": {"file": 400, "function": 50},
    "Lua": {"file": 400, "function": 50},
}

EXT_TO_LANG = {
    ".rs": "Rust",
    ".ts": "TypeScript",
    ".tsx": "TypeScript",
    ".js": "JavaScript",
    ".jsx": "JavaScript",
    ".py": "Python",
    ".go": "Go",
    ".java": "Java",
    ".c": "C",
    ".h": "C",
    ".cpp": "C++",
    ".cc": "C++",
    ".cxx": "C++",
    ".hpp": "C++",
    ".hh": "C++",
    ".hxx": "C++",
    ".ipp": "C++",
    ".swift": "Swift",
    ".lua": "Lua",
}

SKIP_DIRS = {
    ".git",
    ".venv",
    "node_modules",
    "target",
    "dist",
    "build",
}


@dataclass
class Finding:
    language: str
    exception: str
    function: str
    codefile: str
    lines: int
    limit: int


def iter_code_files(root: str) -> Iterable[Tuple[str, str]]:
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            d
            for d in dirnames
            if d not in SKIP_DIRS and not d.startswith(".")
        ]
        for filename in filenames:
            _, ext = os.path.splitext(filename)
            lang = EXT_TO_LANG.get(ext.lower())
            if not lang:
                continue
            yield os.path.join(dirpath, filename), lang


def count_lines(path: str) -> int:
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as f:
            return sum(1 for _ in f)
    except OSError:
        return 0


def read_lines(path: str) -> List[str]:
    with open(path, "r", encoding="utf-8", errors="ignore") as f:
        return f.readlines()


def strip_block_comments(text: str) -> str:
    return re.sub(r"/\*.*?\*/", "", text, flags=re.S)


def strip_line_comment(line: str) -> str:
    if "//" in line:
        return line.split("//", 1)[0]
    return line


def brace_scan_functions(
    lines: List[str],
    start_predicate: Callable[[str], Optional[str]],
) -> List[Tuple[str, int, int]]:
    functions: List[Tuple[str, int, int]] = []
    idx = 0
    while idx < len(lines):
        name = start_predicate(lines[idx])
        if not name:
            idx += 1
            continue
        start = idx
        depth = 0
        started = False
        end = len(lines) - 1
        for j in range(idx, len(lines)):
            line = lines[j]
            if "{" in line:
                started = True
            depth += line.count("{")
            depth -= line.count("}")
            if started and depth <= 0:
                end = j
                break
        functions.append((name, start, end))
        idx = end + 1
    return functions


def find_python_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    results: List[Tuple[str, int, int]] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.strip().startswith("@") and not line.startswith(" ") and not line.startswith("\t"):
            decorator_start = i
            i += 1
            while i < len(lines) and lines[i].strip().startswith("@"):
                i += 1
            if i >= len(lines):
                break
            line = lines[i]
        else:
            decorator_start = None
        match = re.match(r"^(async\s+def|def)\s+([A-Za-z_]\w*)\s*\(", line)
        if match and not line.startswith(" ") and not line.startswith("\t"):
            name = match.group(2)
            start = decorator_start if decorator_start is not None else i
            end = len(lines) - 1
            j = i + 1
            while j < len(lines):
                if lines[j].strip() == "":
                    j += 1
                    continue
                if not lines[j].startswith(" ") and not lines[j].startswith("\t"):
                    if re.match(r"^(async\s+def|def|class)\s+", lines[j]):
                        end = j - 1
                        break
                j += 1
            results.append((name, start, end))
            i = end + 1
            continue
        i += 1
    return results


def find_lua_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    results: List[Tuple[str, int, int]] = []
    i = 0
    depth = 0
    while i < len(lines):
        line = lines[i]
        start_match = re.match(r"^\s*(local\s+)?function\s+([A-Za-z_]\w*)", line)
        if start_match and depth == 0:
            name = start_match.group(2)
            start = i
            depth = 1
            j = i + 1
            end = len(lines) - 1
            while j < len(lines):
                if re.match(r"^\s*function\b", lines[j]):
                    depth += 1
                if re.match(r"^\s*end\b", lines[j]):
                    depth -= 1
                    if depth == 0:
                        end = j
                        break
                j += 1
            results.append((name, start, end))
            i = end + 1
            depth = 0
            continue
        i += 1
    return results


def sanitize_cppish_lines(lines: List[str]) -> List[str]:
    text = "".join(lines)
    text = strip_block_comments(text)
    sanitized = []
    for line in text.splitlines(keepends=True):
        sanitized.append(strip_line_comment(line))
    return sanitized


def find_rust_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    sanitized = sanitize_cppish_lines(lines)

    def start_predicate(line: str) -> Optional[str]:
        match = re.match(r"^\s*(pub\s+)?(async\s+)?fn\s+([A-Za-z_]\w*)\s*\(", line)
        return match.group(3) if match else None

    return brace_scan_functions(sanitized, start_predicate)


def find_go_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    sanitized = sanitize_cppish_lines(lines)

    def start_predicate(line: str) -> Optional[str]:
        match = re.match(r"^\s*func\s+(\([^)]*\)\s*)?([A-Za-z_]\w*)\s*\(", line)
        if not match:
            return None
        return match.group(2)

    return brace_scan_functions(sanitized, start_predicate)


def find_tsjs_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    sanitized = sanitize_cppish_lines(lines)

    def start_predicate(line: str) -> Optional[str]:
        match = re.match(r"^\s*(export\s+)?function\s+([A-Za-z_$][\w$]*)\s*\(", line)
        if match:
            return match.group(2)
        match = re.match(
            r"^\s*(export\s+)?(const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(async\s*)?\([^=]*\)\s*=>\s*\{",
            line,
        )
        if match:
            return match.group(3)
        return None

    return brace_scan_functions(sanitized, start_predicate)


def find_swift_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    sanitized = sanitize_cppish_lines(lines)

    def start_predicate(line: str) -> Optional[str]:
        match = re.match(
            r"^\s*(public|private|internal|fileprivate|open|static|class|mutating|nonmutating|override|final|\s)*func\s+([A-Za-z_]\w*)\s*\(",
            line,
        )
        return match.group(2) if match else None

    return brace_scan_functions(sanitized, start_predicate)


def find_java_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    sanitized = sanitize_cppish_lines(lines)

    def start_predicate(line: str) -> Optional[str]:
        if re.match(r"^\s*(if|for|while|switch|catch|else|do|try)\b", line):
            return None
        match = re.match(
            r"^\s*(public|private|protected|static|final|native|synchronized|abstract|\s)*[\w\<\>\[\]]+\s+([A-Za-z_]\w*)\s*\([^;]*\)\s*\{",
            line,
        )
        return match.group(2) if match else None

    return brace_scan_functions(sanitized, start_predicate)


def find_c_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    sanitized = sanitize_cppish_lines(lines)

    def start_predicate(line: str) -> Optional[str]:
        if re.match(r"^\s*(if|for|while|switch|catch|else|do|try)\b", line):
            return None
        match = re.match(
            r"^\s*[A-Za-z_][\w\s\*\&:<>,\[\]]+\s+([A-Za-z_]\w*)\s*\([^;]*\)\s*\{",
            line,
        )
        return match.group(1) if match else None

    return brace_scan_functions(sanitized, start_predicate)


def find_cpp_functions(lines: List[str]) -> List[Tuple[str, int, int]]:
    return find_c_functions(lines)


def find_functions_for_language(lang: str, lines: List[str]) -> List[Tuple[str, int, int]]:
    if lang == "Python":
        return find_python_functions(lines)
    if lang == "Lua":
        return find_lua_functions(lines)
    if lang == "Rust":
        return find_rust_functions(lines)
    if lang == "Go":
        return find_go_functions(lines)
    if lang == "TypeScript":
        return find_tsjs_functions(lines)
    if lang == "JavaScript":
        return find_tsjs_functions(lines)
    if lang == "Swift":
        return find_swift_functions(lines)
    if lang == "Java":
        return find_java_functions(lines)
    if lang == "C":
        return find_c_functions(lines)
    if lang == "C++":
        return find_cpp_functions(lines)
    return []


def build_report(root: str, tolerance_pct: float) -> List[Finding]:
    findings: List[Finding] = []
    for path, lang in iter_code_files(root):
        rel_path = os.path.relpath(path, root)
        file_line_count = count_lines(path)
        file_limit = LIMITS[lang]["file"]
        file_limit_effective = int(file_limit * (1.0 + tolerance_pct / 100.0))
        if file_line_count > file_limit_effective:
            findings.append(
                Finding(
                    language=lang,
                    exception="file",
                    function="",
                    codefile=rel_path,
                    lines=file_line_count,
                    limit=file_limit_effective,
                )
            )
        try:
            lines = read_lines(path)
        except OSError:
            continue
        func_limit = LIMITS[lang]["function"]
        func_limit_effective = int(func_limit * (1.0 + tolerance_pct / 100.0))
        for name, start, end in find_functions_for_language(lang, lines):
            func_lines = end - start + 1
            if func_lines > func_limit_effective:
                findings.append(
                    Finding(
                        language=lang,
                        exception="function",
                        function=name,
                        codefile=rel_path,
                        lines=func_lines,
                        limit=func_limit_effective,
                    )
                )
    return findings


def write_csv(findings: List[Finding], output_path: str) -> None:
    findings.sort(key=lambda f: (f.language, -f.lines))
    with open(output_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(["language", "exception", "function", "codefile", "lines", "limit"])
        for item in findings:
            writer.writerow(
                [
                    item.language,
                    item.exception,
                    item.function,
                    item.codefile,
                    item.lines,
                    item.limit,
                ]
            )


class HelpOnErrorParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        self.print_help(sys.stderr)
        self.exit(2, f"\n{self.prog}: error: {message}\n")


def main(argv: Optional[List[str]] = None) -> int:
    parser = HelpOnErrorParser(
        description="Report code size violations by file and function."
    )
    parser.add_argument(
        "--root",
        default=os.getcwd(),
        help="Root directory to scan (defaults to cwd).",
    )
    parser.add_argument(
        "--output",
        default="largecode.csv",
        help="CSV output path (defaults to largecode.csv in cwd).",
    )
    parser.add_argument(
        "--tolerance",
        type=float,
        default=0.0,
        help="Percent tolerance added to limits (default 0).",
    )
    args = parser.parse_args(argv)
    root = os.path.abspath(args.root)
    if args.tolerance < 0:
        parser.error("--tolerance must be >= 0")
    findings = build_report(root, args.tolerance)
    write_csv(findings, args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""File discovery, limit checking, and report generation."""
from __future__ import annotations

import csv
import os
from dataclasses import dataclass
from typing import Iterable

from .parser import analyze_file

LIMITS: dict[str, dict[str, int]] = {
    "Rust":       {"file": 500, "function": 80},
    "TypeScript": {"file": 300, "function": 40},
    "JavaScript": {"file": 300, "function": 40},
    "Python":     {"file": 300, "function": 30},
    "Go":         {"file": 400, "function": 60},
    "Java":       {"file": 300, "function": 30},
    "C":          {"file": 500, "function": 60},
    "C++":        {"file": 400, "function": 60},
    "Swift":      {"file": 400, "function": 50},
    "Lua":        {"file": 400, "function": 50},
}

EXT_TO_LANG: dict[str, str] = {
    ".rs":   "Rust",
    ".ts":   "TypeScript",
    ".tsx":  "TypeScript",
    ".js":   "JavaScript",
    ".jsx":  "JavaScript",
    ".py":   "Python",
    ".go":   "Go",
    ".java": "Java",
    ".c":    "C",
    ".h":    "C",
    ".cpp":  "C++",
    ".cc":   "C++",
    ".cxx":  "C++",
    ".hpp":  "C++",
    ".hh":   "C++",
    ".hxx":  "C++",
    ".ipp":  "C++",
    ".swift": "Swift",
    ".lua":  "Lua",
}

SKIP_DIRS: frozenset[str] = frozenset({
    ".git", ".venv", "node_modules", "target", "dist", "build",
})


@dataclass
class Finding:
    language: str
    exception: str
    function: str
    codefile: str
    lines: int
    limit: int


def iter_code_files(root: str) -> Iterable[tuple[str, str]]:
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            d for d in dirnames
            if d not in SKIP_DIRS and not d.startswith(".")
        ]
        for filename in filenames:
            _, ext = os.path.splitext(filename)
            lang = EXT_TO_LANG.get(ext.lower())
            if lang:
                yield os.path.join(dirpath, filename), lang


def build_report(root: str, tolerance_pct: float) -> list[Finding]:
    findings: list[Finding] = []
    for path, lang in iter_code_files(root):
        rel = os.path.relpath(path, root)
        limits = LIMITS[lang]
        file_limit = int(limits["file"] * (1.0 + tolerance_pct / 100.0))
        func_limit = int(limits["function"] * (1.0 + tolerance_pct / 100.0))
        line_count, functions = analyze_file(path, lang)
        if line_count > file_limit:
            findings.append(Finding(lang, "file", "", rel, line_count, file_limit))
        for name, start, end in functions:
            func_lines = end - start + 1
            if func_lines > func_limit:
                findings.append(Finding(lang, "function", name, rel, func_lines, func_limit))
    return findings


def write_csv(findings: list[Finding], output_path: str) -> None:
    findings.sort(key=lambda f: (f.language, -f.lines))
    with open(output_path, "w", newline="", encoding="utf-8") as fh:
        writer = csv.writer(fh)
        writer.writerow(["language", "exception", "function", "codefile", "lines", "limit"])
        for item in findings:
            writer.writerow([
                item.language, item.exception, item.function,
                item.codefile, item.lines, item.limit,
            ])

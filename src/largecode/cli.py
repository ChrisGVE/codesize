"""CLI entry point for LargeCode."""
from __future__ import annotations

import argparse
import os
import sys
from typing import Optional

from .scanner import build_report, write_csv


class HelpOnErrorParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        self.print_help(sys.stderr)
        self.exit(2, f"\n{self.prog}: error: {message}\n")


def main(argv: Optional[list[str]] = None) -> int:
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

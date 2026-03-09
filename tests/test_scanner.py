"""Tests for file discovery logic in scanner.py."""
from __future__ import annotations

import os
import tempfile
import pytest
from largecode.scanner import iter_code_files, SKIP_SUFFIXES


def _make_tree(root: str, paths: list[str]) -> None:
    for rel in paths:
        full = os.path.join(root, rel)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        open(full, "w").close()


class TestSkipSuffixes:
    def test_d_ts_excluded(self):
        with tempfile.TemporaryDirectory() as root:
            _make_tree(root, ["types/foo.d.ts", "src/bar.ts"])
            found = [os.path.basename(p) for p, _ in iter_code_files(root)]
            assert "bar.ts" in found
            assert "foo.d.ts" not in found

    def test_min_js_excluded(self):
        with tempfile.TemporaryDirectory() as root:
            _make_tree(root, ["dist/app.min.js", "src/app.js"])
            found = [os.path.basename(p) for p, _ in iter_code_files(root)]
            assert "app.js" in found
            assert "app.min.js" not in found

    def test_pb2_py_excluded(self):
        with tempfile.TemporaryDirectory() as root:
            _make_tree(root, ["proto/schema_pb2.py", "src/main.py"])
            found = [os.path.basename(p) for p, _ in iter_code_files(root)]
            assert "main.py" in found
            assert "schema_pb2.py" not in found

    def test_pb_go_excluded(self):
        with tempfile.TemporaryDirectory() as root:
            _make_tree(root, ["proto/schema.pb.go", "cmd/main.go"])
            found = [os.path.basename(p) for p, _ in iter_code_files(root)]
            assert "main.go" in found
            assert "schema.pb.go" not in found

    def test_regular_ts_included(self):
        with tempfile.TemporaryDirectory() as root:
            _make_tree(root, ["src/server.ts", "src/types.d.ts"])
            found = [os.path.basename(p) for p, _ in iter_code_files(root)]
            assert "server.ts" in found
            assert "types.d.ts" not in found

    def test_skip_dirs_still_apply(self):
        with tempfile.TemporaryDirectory() as root:
            _make_tree(root, ["node_modules/lib.ts", "src/lib.ts"])
            found = [os.path.basename(p) for p, _ in iter_code_files(root)]
            assert found.count("lib.ts") == 1

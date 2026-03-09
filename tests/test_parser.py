"""Tests for tree-sitter-based function detection in parser.py."""
from __future__ import annotations

import pytest
from largecode.parser import analyze_source


def funcs(source: str, lang: str, path: str = "f.x") -> list[tuple[str, int, int]]:
    """Helper: parse *source* for *lang* and return function list."""
    _, functions = analyze_source(source.encode(), path, lang)
    return functions


def names(source: str, lang: str, path: str = "f.x") -> list[str]:
    return [name for name, _, _ in funcs(source, lang, path)]


def line_count(source: str, lang: str, path: str = "f.x") -> int:
    count, _ = analyze_source(source.encode(), path, lang)
    return count


# ---------------------------------------------------------------------------
# Python
# ---------------------------------------------------------------------------

class TestPython:
    def test_simple_function(self):
        src = "def foo(x):\n    return x\n"
        fs = funcs(src, "Python", "f.py")
        assert len(fs) == 1
        assert fs[0][0] == "foo"

    def test_async_function(self):
        src = "async def bar():\n    pass\n"
        assert names(src, "Python", "f.py") == ["bar"]

    def test_multiple_functions(self):
        src = "def a():\n    pass\n\ndef b():\n    pass\n"
        assert names(src, "Python", "f.py") == ["a", "b"]

    def test_method_inside_class(self):
        src = "class C:\n    def method(self):\n        pass\n"
        assert names(src, "Python", "f.py") == ["method"]

    def test_line_span(self):
        src = "def foo():\n    x = 1\n    return x\n"
        fs = funcs(src, "Python", "f.py")
        start, end = fs[0][1], fs[0][2]
        assert end - start + 1 == 3

    def test_empty_source(self):
        assert funcs("", "Python", "f.py") == []

    def test_file_line_count(self):
        src = "x = 1\ny = 2\nz = 3\n"
        assert line_count(src, "Python", "f.py") == 3


# ---------------------------------------------------------------------------
# Rust
# ---------------------------------------------------------------------------

class TestRust:
    def test_simple_fn(self):
        src = "fn hello() -> i32 {\n    42\n}\n"
        assert names(src, "Rust", "f.rs") == ["hello"]

    def test_pub_async_fn(self):
        src = "pub async fn fetch() {}\n"
        assert names(src, "Rust", "f.rs") == ["fetch"]

    def test_multiple_fns(self):
        src = "fn a() {}\nfn b() {}\n"
        assert names(src, "Rust", "f.rs") == ["a", "b"]

    def test_nested_fn(self):
        src = "fn outer() {\n    fn inner() {}\n}\n"
        result = names(src, "Rust", "f.rs")
        assert "outer" in result
        assert "inner" in result

    def test_line_span(self):
        src = "fn foo() {\n    let x = 1;\n    x\n}\n"
        fs = funcs(src, "Rust", "f.rs")
        start, end = fs[0][1], fs[0][2]
        assert end - start + 1 == 4


# ---------------------------------------------------------------------------
# Go
# ---------------------------------------------------------------------------

class TestGo:
    def test_function(self):
        src = "package main\nfunc Add(a, b int) int { return a + b }\n"
        assert "Add" in names(src, "Go", "f.go")

    def test_method(self):
        src = "package main\ntype T struct{}\nfunc (t T) Method() {}\n"
        assert "Method" in names(src, "Go", "f.go")

    def test_multiple(self):
        src = "package p\nfunc A() {}\nfunc B() {}\n"
        ns = names(src, "Go", "f.go")
        assert "A" in ns and "B" in ns


# ---------------------------------------------------------------------------
# JavaScript
# ---------------------------------------------------------------------------

class TestJavaScript:
    def test_function_declaration(self):
        src = "function greet(name) { return 'hi ' + name; }\n"
        assert names(src, "JavaScript", "f.js") == ["greet"]

    def test_arrow_function(self):
        src = "const add = (a, b) => { return a + b; };\n"
        assert names(src, "JavaScript", "f.js") == ["add"]

    def test_async_arrow(self):
        src = "const fetch = async (url) => { return url; };\n"
        assert names(src, "JavaScript", "f.js") == ["fetch"]

    def test_mixed(self):
        src = (
            "function foo() {}\n"
            "const bar = () => {};\n"
        )
        ns = names(src, "JavaScript", "f.js")
        assert "foo" in ns and "bar" in ns

    def test_arrow_without_block_not_counted(self):
        # Arrow with expression body (no braces) — not a block, still an arrow_function node
        src = "const double = x => x * 2;\n"
        # tree-sitter still sees arrow_function here; we count it regardless
        ns = names(src, "JavaScript", "f.js")
        assert "double" in ns


# ---------------------------------------------------------------------------
# TypeScript — includes TSX grammar selection
# ---------------------------------------------------------------------------

class TestTypeScript:
    def test_function_declaration(self):
        src = "function greet(name: string): string { return name; }\n"
        assert names(src, "TypeScript", "f.ts") == ["greet"]

    def test_tsx_grammar_selected(self):
        src = "function App(): JSX.Element { return <div/>; }\n"
        # Should not raise; TSX grammar is used for .tsx paths
        ns = names(src, "TypeScript", "f.tsx")
        assert "App" in ns

    def test_arrow_function(self):
        src = "const fn = (x: number): number => { return x; };\n"
        assert names(src, "TypeScript", "f.ts") == ["fn"]


# ---------------------------------------------------------------------------
# Java
# ---------------------------------------------------------------------------

class TestJava:
    def test_method(self):
        src = "class C {\n    public int add(int a, int b) { return a + b; }\n}\n"
        assert names(src, "Java", "f.java") == ["add"]

    def test_constructor(self):
        src = "class C {\n    public C() {}\n}\n"
        ns = names(src, "Java", "f.java")
        assert "C" in ns

    def test_multiple_methods(self):
        src = "class C {\n    void a() {}\n    void b() {}\n}\n"
        ns = names(src, "Java", "f.java")
        assert "a" in ns and "b" in ns


# ---------------------------------------------------------------------------
# C
# ---------------------------------------------------------------------------

class TestC:
    def test_simple_function(self):
        src = "int add(int a, int b) { return a + b; }\n"
        assert names(src, "C", "f.c") == ["add"]

    def test_void_function(self):
        src = "void noop(void) {}\n"
        assert names(src, "C", "f.c") == ["noop"]

    def test_multiple_functions(self):
        src = "int a(void) { return 1; }\nint b(void) { return 2; }\n"
        ns = names(src, "C", "f.c")
        assert "a" in ns and "b" in ns


# ---------------------------------------------------------------------------
# C++
# ---------------------------------------------------------------------------

class TestCpp:
    def test_function(self):
        src = "int square(int x) { return x * x; }\n"
        assert names(src, "C++", "f.cpp") == ["square"]

    def test_method(self):
        src = "class Foo {\npublic:\n    void bar() {}\n};\n"
        assert names(src, "C++", "f.cpp") == ["bar"]


# ---------------------------------------------------------------------------
# Swift
# ---------------------------------------------------------------------------

class TestSwift:
    def test_simple_func(self):
        src = "func greet(name: String) -> String {\n    return \"Hello \" + name\n}\n"
        assert names(src, "Swift", "f.swift") == ["greet"]

    def test_multiple_funcs(self):
        src = "func a() {}\nfunc b() {}\n"
        ns = names(src, "Swift", "f.swift")
        assert "a" in ns and "b" in ns


# ---------------------------------------------------------------------------
# Lua
# ---------------------------------------------------------------------------

class TestLua:
    def test_global_function(self):
        src = "function greet(name)\n  return 'hi ' .. name\nend\n"
        assert names(src, "Lua", "f.lua") == ["greet"]

    def test_local_function(self):
        src = "local function add(a, b)\n  return a + b\nend\n"
        assert names(src, "Lua", "f.lua") == ["add"]

    def test_multiple_functions(self):
        src = "function a()\nend\nfunction b()\nend\n"
        ns = names(src, "Lua", "f.lua")
        assert "a" in ns and "b" in ns

    def test_line_span(self):
        src = "function foo()\n  local x = 1\n  return x\nend\n"
        fs = funcs(src, "Lua", "f.lua")
        start, end = fs[0][1], fs[0][2]
        assert end - start + 1 == 4

"""Tree-sitter-based function boundary detection."""
from __future__ import annotations

import tree_sitter_c as tsc
import tree_sitter_cpp as tscpp
import tree_sitter_go as tsgo
import tree_sitter_java as tsjava
import tree_sitter_javascript as tsjs
import tree_sitter_lua as tslua
import tree_sitter_python as tspy
import tree_sitter_rust as tsrust
import tree_sitter_swift as tsswift
import tree_sitter_typescript as tsts
from tree_sitter import Language, Node, Parser

# Node types that represent callable/function constructs per language.
_FUNC_TYPES: dict[str, frozenset[str]] = {
    "Python":     frozenset({"function_definition"}),
    "Rust":       frozenset({"function_item"}),
    "Go":         frozenset({"function_declaration", "method_declaration"}),
    "TypeScript": frozenset({"function_declaration", "method_definition"}),
    "JavaScript": frozenset({"function_declaration", "method_definition"}),
    "Java":       frozenset({"method_declaration", "constructor_declaration"}),
    "C":          frozenset({"function_definition"}),
    "C++":        frozenset({"function_definition"}),
    "Swift":      frozenset({"function_declaration"}),
    "Lua":        frozenset({"function_declaration"}),
}

# Languages that also expose arrow functions via variable_declarator.
_ARROW_LANGS: frozenset[str] = frozenset({"JavaScript", "TypeScript"})


def _build_parsers() -> dict[str, Parser]:
    raw: dict[str, object] = {
        "Python":     tspy.language(),
        "Rust":       tsrust.language(),
        "Go":         tsgo.language(),
        "TypeScript": tsts.language_typescript(),
        "TSX":        tsts.language_tsx(),
        "JavaScript": tsjs.language(),
        "Java":       tsjava.language(),
        "C":          tsc.language(),
        "C++":        tscpp.language(),
        "Swift":      tsswift.language(),
        "Lua":        tslua.language(),
    }
    return {name: Parser(Language(lang)) for name, lang in raw.items()}


_PARSERS: dict[str, Parser] = _build_parsers()


def _parser_key(path: str, lang: str) -> str:
    return "TSX" if path.endswith(".tsx") else lang


def _get_name(node: Node, lang: str) -> str:
    """Extract the function name from a function AST node."""
    if lang in ("C", "C++"):
        # function_definition -> function_declarator -> identifier (chained declarators)
        declarator = node.child_by_field_name("declarator")
        while declarator is not None:
            if declarator.type in ("identifier", "field_identifier"):
                return declarator.text.decode("utf-8")
            declarator = declarator.child_by_field_name("declarator")
        return "<anonymous>"
    name_node = node.child_by_field_name("name")
    if name_node is not None:
        return name_node.text.decode("utf-8")
    return "<anonymous>"


def _is_arrow_declarator(node: Node) -> bool:
    """True if node is a variable_declarator whose value is an arrow_function."""
    if node.type != "variable_declarator":
        return False
    value = node.child_by_field_name("value")
    return value is not None and value.type == "arrow_function"


def _walk(
    node: Node,
    lang: str,
    func_types: frozenset[str],
    results: list[tuple[str, int, int]],
) -> None:
    is_func = node.type in func_types
    is_arrow = lang in _ARROW_LANGS and _is_arrow_declarator(node)
    if is_func or is_arrow:
        name = _get_name(node, lang)
        results.append((name, node.start_point[0], node.end_point[0]))
    for child in node.children:
        _walk(child, lang, func_types, results)


def analyze_source(
    source: bytes, path: str, lang: str
) -> tuple[int, list[tuple[str, int, int]]]:
    """Parse *source* bytes and return (total_lines, [(name, start_row, end_row)]).

    Rows are 0-indexed; caller computes line count as ``end_row - start_row + 1``.
    *path* is used only to select the TSX grammar for ``.tsx`` files.
    """
    parser = _PARSERS[_parser_key(path, lang)]
    func_types = _FUNC_TYPES[lang]
    tree = parser.parse(source)
    # Count logical lines from the source bytes to match standard line-counting
    # behaviour (tree-sitter adds a phantom line after a trailing newline).
    line_count = len(source.splitlines()) if source else 0
    results: list[tuple[str, int, int]] = []
    _walk(tree.root_node, lang, func_types, results)
    return line_count, results


def analyze_file(path: str, lang: str) -> tuple[int, list[tuple[str, int, int]]]:
    """Read *path* from disk and delegate to :func:`analyze_source`."""
    try:
        with open(path, "rb") as fh:
            source = fh.read()
    except OSError:
        return 0, []
    return analyze_source(source, path, lang)

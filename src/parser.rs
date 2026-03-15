use std::path::Path;
use tree_sitter::{Language, Node, Parser};

pub type Functions = Vec<(String, usize, usize)>;

fn language_for(key: &str) -> Option<Language> {
    match key {
        "Python" => Some(Language::new(tree_sitter_python::LANGUAGE)),
        "Rust" => Some(Language::new(tree_sitter_rust::LANGUAGE)),
        "Go" => Some(Language::new(tree_sitter_go::LANGUAGE)),
        "JavaScript" => Some(Language::new(tree_sitter_javascript::LANGUAGE)),
        "TypeScript" => Some(Language::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)),
        "TSX" => Some(Language::new(tree_sitter_typescript::LANGUAGE_TSX)),
        "Java" => Some(Language::new(tree_sitter_java::LANGUAGE)),
        "C" => Some(Language::new(tree_sitter_c::LANGUAGE)),
        "C++" => Some(Language::new(tree_sitter_cpp::LANGUAGE)),
        "Swift" => Some(Language::new(tree_sitter_swift::LANGUAGE)),
        "Lua" => Some(Language::new(tree_sitter_lua::LANGUAGE)),
        "ObjC" => Some(Language::new(tree_sitter_objc::LANGUAGE)),
        "Zig" => Some(Language::new(tree_sitter_zig::LANGUAGE)),
        _ => None,
    }
}

fn func_types(lang: &str) -> &'static [&'static str] {
    match lang {
        "Python" => &["function_definition"],
        "Rust" => &["function_item"],
        "Go" => &["function_declaration", "method_declaration"],
        "TypeScript" | "TSX" | "JavaScript" => &["function_declaration", "method_definition"],
        "Java" => &["method_declaration", "constructor_declaration"],
        "C" | "C++" => &["function_definition"],
        "Swift" => &["function_declaration"],
        "Lua" => &["function_declaration"],
        "ObjC" => &["function_definition", "method_definition"],
        "Zig" => &["function_declaration"],
        _ => &[],
    }
}

fn has_arrow_funcs(lang: &str) -> bool {
    matches!(lang, "JavaScript" | "TypeScript" | "TSX")
}

fn get_name<'a>(node: Node<'a>, source: &[u8], lang: &str) -> String {
    if lang == "C" || lang == "C++" || (lang == "ObjC" && node.kind() == "function_definition") {
        let mut decl = node.child_by_field_name("declarator");
        while let Some(d) = decl {
            if d.kind() == "identifier" || d.kind() == "field_identifier" {
                return d.utf8_text(source).unwrap_or("<anonymous>").to_string();
            }
            decl = d.child_by_field_name("declarator");
        }
        return "<anonymous>".to_string();
    }
    if lang == "ObjC" && node.kind() == "method_definition" {
        return get_objc_selector(node, source);
    }
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<anonymous>".to_string())
}

/// Builds an ObjC selector string from a `method_definition` node.
///
/// The grammar represents selectors as alternating `identifier` and
/// `method_parameter` children:
///   `- (void)foo`                → identifier `foo`
///   `- (void)setName:(id)x`      → identifier `setName` + method_parameter → `"setName:"`
///   `- (void)foo:(id)x bar:(int)y` → `"foo:bar:"`
fn get_objc_selector(node: Node, source: &[u8]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for i in 0..node.child_count() {
        let Some(child) = node.child(i as u32) else {
            continue;
        };
        match child.kind() {
            "identifier" => {
                if let Ok(t) = child.utf8_text(source) {
                    parts.push(t.to_string());
                }
            }
            "method_parameter" => {
                // Append colon to the preceding keyword identifier.
                if let Some(last) = parts.last_mut() {
                    if !last.ends_with(':') {
                        last.push(':');
                    }
                }
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        "<anonymous>".to_string()
    } else {
        parts.join("")
    }
}

fn is_arrow_declarator(node: Node) -> bool {
    node.kind() == "variable_declarator"
        && node
            .child_by_field_name("value")
            .map(|v| v.kind() == "arrow_function")
            .unwrap_or(false)
}

fn walk(
    node: Node,
    source: &[u8],
    lang: &str,
    ftypes: &[&str],
    arrows: bool,
    results: &mut Functions,
) {
    if ftypes.contains(&node.kind()) || (arrows && is_arrow_declarator(node)) {
        let name = get_name(node, source, lang);
        results.push((name, node.start_position().row, node.end_position().row));
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk(child, source, lang, ftypes, arrows, results);
        }
    }
}

/// Counts source lines matching Python's `str.splitlines()` semantics.
fn count_lines(source: &[u8]) -> usize {
    if source.is_empty() {
        return 0;
    }
    let n = source.split(|&b| b == b'\n').count();
    if source.ends_with(b"\n") {
        n - 1
    } else {
        n
    }
}

/// Parses `source` bytes for `lang`, returning `(total_lines, functions)`.
///
/// Each function entry is `(name, start_row, end_row)` with 0-based rows;
/// callers compute line span as `end_row - start_row + 1`.
/// `path` is used only to select the TSX grammar for `.tsx` files.
pub fn analyze_source(source: &[u8], path: &str, lang: &str) -> (usize, Functions) {
    let lang_key = if path.ends_with(".tsx") { "TSX" } else { lang };
    let total_lines = count_lines(source);

    let Some(grammar) = language_for(lang_key) else {
        // No tree-sitter grammar available; report file length only.
        return (total_lines, Vec::new());
    };

    let ftypes = func_types(lang_key);
    let arrows = has_arrow_funcs(lang_key);

    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .expect("Failed to set language");

    let tree = parser.parse(source, None).expect("Failed to parse");
    let mut results = Functions::new();
    walk(
        tree.root_node(),
        source,
        lang_key,
        ftypes,
        arrows,
        &mut results,
    );
    (total_lines, results)
}

/// Reads `path` from disk and delegates to [`analyze_source`].
pub fn analyze_file(path: &Path, lang: &str) -> (usize, Functions) {
    match std::fs::read(path) {
        Ok(source) => analyze_source(&source, path.to_str().unwrap_or(""), lang),
        Err(_) => (0, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn funcs(source: &str, lang: &str, path: &str) -> Functions {
        analyze_source(source.as_bytes(), path, lang).1
    }

    fn names(source: &str, lang: &str, path: &str) -> Vec<String> {
        funcs(source, lang, path)
            .into_iter()
            .map(|(n, _, _)| n)
            .collect()
    }

    fn line_count(source: &str, lang: &str) -> usize {
        analyze_source(source.as_bytes(), "f.x", lang).0
    }

    // --- Python ---

    #[test]
    fn python_simple_function() {
        assert_eq!(
            names("def foo(x):\n    return x\n", "Python", "f.py"),
            vec!["foo"]
        );
    }

    #[test]
    fn python_async_function() {
        assert_eq!(
            names("async def bar():\n    pass\n", "Python", "f.py"),
            vec!["bar"]
        );
    }

    #[test]
    fn python_method_in_class() {
        assert_eq!(
            names(
                "class C:\n    def method(self):\n        pass\n",
                "Python",
                "f.py"
            ),
            vec!["method"]
        );
    }

    #[test]
    fn python_line_span() {
        let fs = funcs("def foo():\n    x = 1\n    return x\n", "Python", "f.py");
        assert_eq!(fs[0].2 - fs[0].1 + 1, 3);
    }

    #[test]
    fn python_empty_source() {
        assert!(funcs("", "Python", "f.py").is_empty());
    }

    #[test]
    fn python_line_count() {
        assert_eq!(line_count("x = 1\ny = 2\nz = 3\n", "Python"), 3);
    }

    // --- Rust ---

    #[test]
    fn rust_simple_fn() {
        assert_eq!(
            names("fn hello() -> i32 {\n    42\n}\n", "Rust", "f.rs"),
            vec!["hello"]
        );
    }

    #[test]
    fn rust_pub_async_fn() {
        assert_eq!(
            names("pub async fn fetch() {}\n", "Rust", "f.rs"),
            vec!["fetch"]
        );
    }

    #[test]
    fn rust_nested_fn() {
        let ns = names("fn outer() {\n    fn inner() {}\n}\n", "Rust", "f.rs");
        assert!(ns.contains(&"outer".to_string()));
        assert!(ns.contains(&"inner".to_string()));
    }

    #[test]
    fn rust_line_span() {
        let fs = funcs("fn foo() {\n    let x = 1;\n    x\n}\n", "Rust", "f.rs");
        assert_eq!(fs[0].2 - fs[0].1 + 1, 4);
    }

    // --- Go ---

    #[test]
    fn go_function() {
        let ns = names(
            "package main\nfunc Add(a, b int) int { return a + b }\n",
            "Go",
            "f.go",
        );
        assert!(ns.contains(&"Add".to_string()));
    }

    #[test]
    fn go_method() {
        let src = "package main\ntype T struct{}\nfunc (t T) Method() {}\n";
        assert!(names(src, "Go", "f.go").contains(&"Method".to_string()));
    }

    // --- JavaScript ---

    #[test]
    fn js_function_declaration() {
        assert_eq!(
            names(
                "function greet(name) { return 'hi ' + name; }\n",
                "JavaScript",
                "f.js"
            ),
            vec!["greet"]
        );
    }

    #[test]
    fn js_arrow_function() {
        let ns = names(
            "const add = (a, b) => { return a + b; };\n",
            "JavaScript",
            "f.js",
        );
        assert!(ns.contains(&"add".to_string()));
    }

    #[test]
    fn js_mixed() {
        let src = "function foo() {}\nconst bar = () => {};\n";
        let ns = names(src, "JavaScript", "f.js");
        assert!(ns.contains(&"foo".to_string()));
        assert!(ns.contains(&"bar".to_string()));
    }

    // --- TypeScript ---

    #[test]
    fn ts_function_declaration() {
        let src = "function greet(name: string): string { return name; }\n";
        assert_eq!(names(src, "TypeScript", "f.ts"), vec!["greet"]);
    }

    #[test]
    fn tsx_grammar_selected() {
        let src = "function App(): JSX.Element { return <div/>; }\n";
        let ns = names(src, "TypeScript", "f.tsx");
        assert!(ns.contains(&"App".to_string()));
    }

    #[test]
    fn ts_arrow_function() {
        let src = "const fn = (x: number): number => { return x; };\n";
        assert!(names(src, "TypeScript", "f.ts").contains(&"fn".to_string()));
    }

    // --- Java ---

    #[test]
    fn java_method() {
        let src = "class C {\n    public int add(int a, int b) { return a + b; }\n}\n";
        assert!(names(src, "Java", "f.java").contains(&"add".to_string()));
    }

    #[test]
    fn java_constructor() {
        let src = "class C {\n    public C() {}\n}\n";
        assert!(names(src, "Java", "f.java").contains(&"C".to_string()));
    }

    // --- C ---

    #[test]
    fn c_simple_function() {
        assert_eq!(
            names("int add(int a, int b) { return a + b; }\n", "C", "f.c"),
            vec!["add"]
        );
    }

    #[test]
    fn c_multiple_functions() {
        let ns = names(
            "int a(void) { return 1; }\nint b(void) { return 2; }\n",
            "C",
            "f.c",
        );
        assert!(ns.contains(&"a".to_string()));
        assert!(ns.contains(&"b".to_string()));
    }

    // --- C++ ---

    #[test]
    fn cpp_function() {
        assert_eq!(
            names("int square(int x) { return x * x; }\n", "C++", "f.cpp"),
            vec!["square"]
        );
    }

    #[test]
    fn cpp_method() {
        let src = "class Foo {\npublic:\n    void bar() {}\n};\n";
        assert!(names(src, "C++", "f.cpp").contains(&"bar".to_string()));
    }

    // --- Swift ---

    #[test]
    fn swift_simple_func() {
        let src = "func greet(name: String) -> String {\n    return \"Hello \" + name\n}\n";
        assert_eq!(names(src, "Swift", "f.swift"), vec!["greet"]);
    }

    // --- Lua ---

    #[test]
    fn lua_global_function() {
        let src = "function greet(name)\n  return 'hi ' .. name\nend\n";
        assert_eq!(names(src, "Lua", "f.lua"), vec!["greet"]);
    }

    #[test]
    fn lua_line_span() {
        let src = "function foo()\n  local x = 1\n  return x\nend\n";
        let fs = funcs(src, "Lua", "f.lua");
        assert_eq!(fs[0].2 - fs[0].1 + 1, 4);
    }

    // --- Objective-C ---

    #[test]
    fn objc_c_function() {
        let src = "void greet(const char *name) {\n    return;\n}\n";
        assert_eq!(names(src, "ObjC", "f.m"), vec!["greet"]);
    }

    #[test]
    fn objc_simple_method() {
        let src = "@implementation Foo\n- (void)bar {\n    int x = 1;\n}\n@end\n";
        assert!(names(src, "ObjC", "f.m").contains(&"bar".to_string()));
    }

    #[test]
    fn objc_keyword_method() {
        let src = "@implementation Foo\n- (void)setName:(NSString *)name {\n    self->name = name;\n}\n@end\n";
        assert!(names(src, "ObjC", "f.m").contains(&"setName:".to_string()));
    }

    #[test]
    fn objc_multi_keyword_method() {
        let src =
            "@implementation Foo\n- (void)doFoo:(int)a withBar:(int)b {\n    return;\n}\n@end\n";
        assert!(names(src, "ObjC", "f.m").contains(&"doFoo:withBar:".to_string()));
    }

    // --- Zig ---

    #[test]
    fn zig_simple_fn() {
        let src = "pub fn add(a: i32, b: i32) i32 {\n    return a + b;\n}\n";
        assert_eq!(names(src, "Zig", "f.zig"), vec!["add"]);
    }

    #[test]
    fn zig_private_fn() {
        let src = "fn helper() void {\n}\n";
        assert_eq!(names(src, "Zig", "f.zig"), vec!["helper"]);
    }

    #[test]
    fn zig_line_span() {
        let src = "pub fn foo() void {\n    const x = 1;\n    _ = x;\n}\n";
        let fs = funcs(src, "Zig", "f.zig");
        assert_eq!(fs[0].2 - fs[0].1 + 1, 4);
    }

    // --- line counting ---

    #[test]
    fn count_lines_empty() {
        assert_eq!(count_lines(b""), 0);
    }

    #[test]
    fn count_lines_no_trailing_newline() {
        assert_eq!(count_lines(b"a\nb"), 2);
    }

    #[test]
    fn count_lines_trailing_newline() {
        assert_eq!(count_lines(b"a\nb\n"), 2);
    }
}

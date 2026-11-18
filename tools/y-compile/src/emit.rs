//! Assembles the Rust files of a lowered package: the library's root, the module, its tests and `main`. The Rust is
//! typed; `bare.rs` strips it into bare bodies and constraint files.
use crate::lower::{const_type, Ctx, Form, Lowerer};
use crate::model::{FnInfo, Mode, Module, TypeDef};
use crate::ty::{pascal, upper, Ty};
use y_parse::Node;

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) if !s.starts_with("`") && !s.chars().nth(1).is_some_and(|x| x.is_uppercase()) => f.to_lowercase().collect::<String>() + c.as_str(),
        _ => s.to_string(),
    }
}

/// An entry's text without its type: `The game, as game.` is `the game.`
fn entry_text(t: &str) -> String {
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = match t.rfind(", as ") {
        Some(k) if t.ends_with('.') => format!("{}.", &t[..k]),
        _ => t,
    };
    lower_first(&t)
}

/// The documentation of a declaration as `///` lines: its prose, then its roles as a list.
fn doc_lines(d: &Node, f: Option<&FnInfo>) -> Vec<String> {
    let mut out = vec![];
    let Some(doc) = d.node("doc") else {
        if let Some(c) = d.str("comment") {
            for l in c.lines() {
                out.push(format!("/// {}", l.trim_start_matches("--").trim()));
            }
        }
        return out;
    };
    let text = doc.str("text").unwrap_or("");
    let entry_lines: Vec<usize> = doc.nodes("entries").iter().map(|e| e.line - doc.line).collect();
    for (k, l) in text.lines().enumerate() {
        if entry_lines.iter().any(|e| *e == k + 1) || l.trim().is_empty() && !out.is_empty() && entry_lines.iter().any(|e| *e <= k + 2) {
            break;
        }
        if !l.trim().is_empty() {
            out.push(format!("/// {}", l.trim()));
        }
    }
    let Some(f) = f else { return out };
    let mut items = vec![];
    let injected: Vec<String> = f.params.iter().filter(|p| p.injected).map(|p| format!("`{}`", p.name)).collect();
    for e in doc.nodes("entries") {
        let name = e.str("name").unwrap_or("");
        let text = entry_text(e.str("text").unwrap_or(""));
        if let Some(p) = f.params.iter().find(|p| p.name == name) {
            if !p.injected {
                items.push(format!("- `{name}`: {text}"));
            }
        } else if f.result.as_ref().is_some_and(|r| r.0 == name) {
            items.push(format!("- Returns {text}"));
        } else if let Some(case) = name.strip_prefix("fails ") {
            match case.split_once('.') {
                Some((_, c)) => items.push(format!("- Fails `{}`: {text}", pascal(c))),
                None => items.push(format!("- Fails with a `{}`: {text}", crate::ty::external(case).map(|e| e.0.to_string()).unwrap_or_else(|| pascal(case)))),
            }
        }
    }
    if !injected.is_empty() {
        let k = items.iter().position(|i| i.starts_with("- Returns") || i.starts_with("- Fails")).unwrap_or(items.len());
        let what = if injected.len() == 1 { "the Y `injected` parameter" } else { "the Y `injected` parameters" };
        items.insert(k, format!("- {}: {what}.", injected.join(", ")));
    }
    if !items.is_empty() {
        out.push("///".into());
        out.extend(items.into_iter().map(|i| format!("/// {i}")));
    }
    out
}

pub struct Emitted {
    pub copies: Vec<(usize, String, String)>,
    pub lib: String,
    pub module: String,
    pub tests: String,
    pub main: String,
}

/// The code of a file without its comments and the text of its string literals.
fn code_only(code: &str) -> String {
    let mut out = String::new();
    for l in code.lines() {
        let mut in_str = false;
        let mut prev = ' ';
        let mut kept = String::new();
        let cs: Vec<char> = l.chars().collect();
        let mut k = 0;
        while k < cs.len() {
            let c = cs[k];
            if !in_str && c == '/' && cs.get(k + 1) == Some(&'/') {
                break;
            }
            if c == '"' && prev != '\\' {
                in_str = !in_str;
                kept.push('"');
            } else if !in_str {
                kept.push(c);
            }
            prev = c;
            k += 1;
        }
        out.push_str(&kept);
        out.push('\n');
    }
    out
}

/// The imports a file's code needs, read from the names it uses.
pub fn imports_for(code: &str) -> std::collections::BTreeSet<String> {
    let code = code_only(code);
    let words: std::collections::BTreeSet<String> = code.split(|c: char| !(c.is_alphanumeric() || c == '_')).map(str::to_string).collect();
    let mut out = std::collections::BTreeSet::new();
    for (name, path) in [
        ("YString", "y_string::YString"),
        ("Rune", "y_string::Rune"),
        ("List", "y_collections::List"),
        ("Set", "y_collections::Set"),
        ("Console", "y_runtime::Console"),
        ("ConsoleError", "y_runtime::ConsoleError"),
        ("Forever", "y_runtime::Forever"),
        ("Random", "y_runtime::Random"),
        ("System", "y_runtime::System"),
        ("Clock", "y_runtime::Clock"),
        ("require", "y_runtime::require"),
        ("ensure", "y_runtime::ensure"),
        ("invariant", "y_runtime::invariant"),
        ("not_reachable", "y_runtime::not_reachable"),
        ("not_implemented", "y_runtime::not_implemented"),
    ] {
        if words.contains(name) && (name.chars().next().unwrap().is_uppercase() || code.contains(&format!("{name}("))) {
            out.insert(path.to_string());
        }
    }
    if code.contains("list!") {
        out.insert("y_collections::list".into());
    }
    if code.contains("list::") {
        out.insert("y_runtime::list".into());
    }
    out
}

fn header(uses: &std::collections::BTreeSet<String>) -> Vec<String> {
    // group by crate: `use y_runtime::{list, Console};`
    let mut by: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for u in uses {
        let (k, n) = u.split_once("::").unwrap();
        by.entry(k.to_string()).or_default().push(n.to_string());
    }
    by.into_iter()
        .map(|(k, mut ns)| {
            ns.sort_by(|a, b| {
                let ka = (a.chars().next().is_some_and(char::is_uppercase), a.clone());
                let kb = (b.chars().next().is_some_and(char::is_uppercase), b.clone());
                ka.cmp(&kb)
            });
            if ns.len() == 1 { format!("use {k}::{};", ns[0]) } else { format!("use {k}::{{{}}};", ns.join(", ")) }
        })
        .collect()
}

impl<'a> Lowerer<'a> {
    fn fn_signature(&mut self, f: &FnInfo, vis: &str) -> String {
        let mut ps = vec![];
        for p in &f.params {
            let t = self.rust_type(&p.ty);
            let t = match p.mode {
                Mode::Ref => format!("&{t}"),
                Mode::Mut => format!("&mut {t}"),
                Mode::Value => t,
            };
            ps.push(format!("{}: {t}", p.name));
        }
        let ret = f.result.as_ref().map(|r| r.1.clone());
        let rt = match (&ret, &f.fails) {
            (r, Some(e)) => {
                let r = r.clone().unwrap_or(Ty::Unit);
                format!(" -> Result<{}, {}>", self.rust_type(&r), self.rust_type(e))
            }
            (Some(r), None) => format!(" -> {}", self.rust_type(r)),
            (None, None) => String::new(),
        };
        format!("{vis}fn {}({}){rt}", f.name, ps.join(", "))
    }

    pub fn function_item(&mut self, d: &Node, f: &FnInfo, vis: &str) {
        for l in doc_lines(d, Some(f)) {
            self.line(l);
        }
        let sig = self.fn_signature(f, vis);
        self.line(format!("{sig} {{"));
        self.ind += 1;
        self.reset_scopes();
        self.ctx = Ctx {
            module: self.m.name.clone(),
            name: f.name.clone(),
            result: f.result.as_ref().map(|r| r.1.clone()),
            fails: f.fails.clone(),
            branches: vec![],
            section: None,
        };
        for p in &f.params {
            let form = match p.mode {
                Mode::Ref => Form::Ref,
                Mode::Mut => Form::Mut,
                Mode::Value => Form::Place,
            };
            self.bind(&p.name, p.ty.clone(), form);
        }
        for w in d.nodes("where") {
            self.use_("y_runtime::require");
            let c = self.expr(w.node("condition").unwrap());
            self.line(format!("require({}, \"{}::{}#{}\");", c.code, self.m.name, f.name, w.str("name").unwrap()));
        }
        if f.rule {
            let v = self.expr(d.node("value").unwrap());
            self.line(v.code);
        } else {
            let body = d.nodes("body");
            let ensures = d.nodes("ensures");
            if ensures.is_empty() {
                self.block(&body, true);
            } else {
                // `ensures name = p`: the result is bound to its Y name, checked, then given
                let r = f.result.as_ref().map(|r| r.0.clone()).unwrap_or_else(|| "result".into());
                self.push_scope();
                self.block_open_tail(&body);
                let start = self.last_start;
                let ind = "    ".repeat(self.ind);
                self.out[start] = format!("{ind}let {r} = {};", self.out[start].trim_start());
                let end = self.out.len() - 1;
                if !self.out[end].ends_with(';') {
                    self.out[end].push(';');
                }
                self.bind(&r, f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unknown), Form::Place);
                for w in ensures {
                    self.use_("y_runtime::ensure");
                    let c = self.expr(w.node("condition").unwrap());
                    self.line(format!("ensure({}, \"{}::{}#{}\");", c.code, self.m.name, f.name, w.str("name").unwrap()));
                }
                self.line(r);
                self.pop_scope();
            }
            if f.fails.is_some() && f.result.is_none() && !crate::lower::diverges(&body) {
                self.line("Ok(())");
            }
        }
        self.ind -= 1;
        self.line("}");
    }

    fn type_item(&mut self, d: &Node, facet: bool) {
        let name = d.str("name").unwrap();
        let m = self.m;
        let info = &m.types[name];
        for l in doc_lines(d, None) {
            self.line(l);
        }
        let copy = crate::model::is_copy(m, &Ty::Named(name.to_string()));
        let eq = true;
        let mut derives = vec!["Clone"];
        if copy {
            derives.push("Copy");
        }
        derives.push("Debug");
        derives.push("PartialEq");
        if eq {
            derives.push("Eq");
        }
        self.line(format!("#[derive({})]", derives.join(", ")));
        let vis = if facet { "pub " } else { "" };
        match &info.def {
            TypeDef::Struct { fields, .. } => {
                self.line(format!("{vis}struct {} {{", pascal(name)));
                self.ind += 1;
                let fvis = if facet { "pub(super) " } else { "" };
                for (f, t) in fields {
                    let t = self.rust_type(t);
                    self.line(format!("{fvis}{f}: {t},"));
                }
                self.ind -= 1;
                self.line("}");
            }
            TypeDef::Enum { cases } => {
                self.line(format!("{vis}enum {} {{", pascal(name)));
                self.ind += 1;
                for (c, fs) in cases {
                    if fs.is_empty() {
                        self.line(format!("{},", pascal(c)));
                    } else {
                        let parts: Vec<String> = fs.iter().map(|(f, t)| format!("{f}: {}", self.rust_type(t))).collect();
                        self.line(format!("{} {{ {} }},", pascal(c), parts.join(", ")));
                    }
                }
                self.ind -= 1;
                self.line("}");
                // a closed type shown in an interpolation shows as its constant's value
                if self.display.contains(name) && d.kind == "constants" {
                    let en = pascal(name);
                    self.line("");
                    self.line(format!("impl std::fmt::Display for {en} {{"));
                    self.ind += 1;
                    self.line("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
                    self.ind += 1;
                    self.line("match self {");
                    self.ind += 1;
                    for i in d.nodes("items") {
                        let v = i.node("value").and_then(|v| v.str("value")).unwrap_or("");
                        self.line(format!("{en}::{} => f.write_str(\"{v}\"),", pascal(i.str("name").unwrap())));
                    }
                    self.ind -= 1;
                    self.line("}");
                    self.ind -= 1;
                    self.line("}");
                    self.ind -= 1;
                    self.line("}");
                }
                // `or fail` on a failure that one case carries converts through that case (Y § 3.4)
                for (c, fs) in cases {
                    if let [(f, Ty::Named(t))] = &fs[..] {
                        if crate::ty::external(t).is_some() && t.ends_with("_error") {
                            let (et, en) = (self.rust_type(&Ty::Named(t.clone())), pascal(name));
                            self.line("");
                            self.line(format!("/// Y's `or fail` on a `{et}` converts through `{en}`'s one case that carries it (Y § 3.4)."));
                            self.line(format!("impl From<{et}> for {en} {{"));
                            self.ind += 1;
                            self.line(format!("fn from({f}: {et}) -> {en} {{"));
                            self.ind += 1;
                            self.line(format!("{en}::{} {{ {f} }}", pascal(c)));
                            self.ind -= 1;
                            self.line("}");
                            self.ind -= 1;
                            self.line("}");
                        }
                    }
                }
            }
        }
    }

    fn const_item(&mut self, d: &Node, facet: bool) {
        let name = d.str("name").unwrap();
        for l in doc_lines(d, None) {
            self.line(l);
        }
        let vis = if facet { "pub " } else { "" };
        self.line(format!("{vis}mod {name} {{"));
        self.ind += 1;
        let saved = std::mem::take(&mut self.uses);
        for (k, i) in d.nodes("items").iter().enumerate() {
            if k > 0 {
                self.line("");
            }
            for l in doc_lines(i, None) {
                self.line(l);
            }
            let ty = i.node("type").map(Ty::from_node);
            let v = i.node("value").unwrap();
            let t = const_type(ty.as_ref(), v);
            let code = self.const_value(v);
            let rt = self.rust_type(&t);
            self.line(format!("pub const {}: {rt} = {code};", upper(i.str("name").unwrap())));
        }
        let inner = std::mem::replace(&mut self.uses, saved);
        if !inner.is_empty() {
            let at = self.out.iter().rposition(|l| l.ends_with(&format!("mod {name} {{"))).unwrap() + 1;
            let ind = "    ".repeat(self.ind);
            for (k, u) in header(&inner).into_iter().enumerate() {
                self.out.insert(at + k, format!("{ind}{u}"));
            }
            self.out.insert(at + inner.len(), String::new());
        }
        self.ind -= 1;
        self.line("}");
    }

    /// A constant's value: a list is an array, at every depth.
    fn const_value(&mut self, v: &Node) -> String {
        if v.kind == "list" {
            let items: Vec<String> = v.nodes("items").iter().map(|x| self.const_value(x)).collect();
            format!("[{}]", items.join(", "))
        } else {
            self.expr(v).code
        }
    }

    fn decl(&mut self, d: &Node, facet: bool) {
        match d.kind {
            "function" | "rule" => {
                let f = self.m.fns[d.str("name").unwrap()].clone();
                self.function_item(d, &f, if facet { "pub " } else { "" });
            }
            "structure" | "type_decl" => self.type_item(d, facet),
            "constants" => {
                if self.m.types.contains_key(d.str("name").unwrap()) {
                    self.type_item(d, facet)
                } else {
                    self.const_item(d, facet)
                }
            }
            _ => {}
        }
    }
}

/// The module's file: its imports, its facets as Rust modules, and the declarations outside every facet.
pub fn module_file(m: &Module, y_path: &str) -> String {
    module_out(m, y_path).0
}

/// The module's file, and the copies its lowering made.
pub fn module_out(m: &Module, y_path: &str) -> (String, Vec<(usize, String, String)>) {
    // a first pass finds the closed types an interpolation shows, which the second gives a `Display`
    let shown = module_pass(m, y_path, Default::default()).shown;
    let l = module_pass(m, y_path, shown);
    let copies = l.copies.clone();
    (module_text(m, y_path, l), copies)
}

fn module_pass<'a>(m: &'a Module<'a>, _y_path: &str, display: std::collections::BTreeSet<String>) -> Lowerer<'a> {
    let mut l = Lowerer::new(m, None);
    l.display = display;
    let mut first = true;
    for d in m.tree.nodes("decls") {
        match d.kind {
            "module" | "import" => continue,
            _ => {}
        }
        if !first {
            l.line("");
        }
        first = false;
        if d.kind == "facet" {
            let name = d.str("name").unwrap();
            let doc = d.str("comment").map(|c| c.trim_start_matches("--").trim().to_string()).unwrap_or_default();
            let doc = doc.trim_end_matches('.').to_string();
            // `-- Moves: reading them, …` above `facet moves`: the facet's name is said once
            let doc = match doc.split_once(": ") {
                Some((head, rest)) if head.to_lowercase().replace(' ', "_") == name => rest.to_string(),
                _ => doc,
            };
            l.line(format!("/// The Y facet `{name}`: {}.", lower_first(&doc)));
            l.line(format!("pub mod {name} {{"));
            l.ind += 1;
            l.line("use super::*;");
            for (k, x) in d.nodes("decls").iter().enumerate() {
                l.line("");
                let _ = k;
                l.decl(x, true);
            }
            l.ind -= 1;
            l.line("}");
        } else {
            l.decl(d, false);
        }
    }
    l
}

fn module_text(m: &Module, y_path: &str, l: Lowerer) -> String {
    let mut out = vec![format!("//! Lowered from `{y_path}`, the Y module `{}`.", m.name)];
    out.push("//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet.".into());
    out.push("//! Each named decision of the Y source is marked `// y: #name` where its condition is tested.".into());
    out.push(String::new());
    let mut body = l.out.join("\n");
    // the facets' own `use` lines name nothing
    body = body.replace("use super::*;", "");
    let mut uses = imports_for(&body);
    // a type the module declares is its own, not the runtime's
    uses.retain(|u| !m.types.keys().any(|t| u.ends_with(&format!("::{}", pascal(t)))));
    out.extend(header(&uses));
    if !m.facets.is_empty() {
        let fs: Vec<String> = { let mut v: Vec<String> = m.facets.iter().map(|f| format!("{}::*", f.0)).collect(); v.sort(); v };
        out.push(format!("use self::{{{}}};", fs.join(", ")));
    }
    out.push(String::new());
    out.push("#[cfg(test)]".into());
    out.push("mod tests;".into());
    out.push(String::new());
    out.extend(l.out);
    out.join("\n") + "\n"
}

fn comment_above(l: &Lowerer, line: usize) -> Option<String> {
    let mut k = line - 1;
    let mut texts = vec![];
    while let Some(t) = l.comments.get(&k) {
        texts.push(t.trim_start_matches("--").trim().to_string());
        k -= 1;
    }
    texts.reverse();
    (!texts.is_empty()).then(|| texts.join(" "))
}

/// The tests' file: each `test` a `#[test]` function.
pub fn tests_file(m: &Module, test_tree: &Node, test_module: &Module, y_path: &str) -> String {
    let lib: &Module = m;
    let mut l = Lowerer::new(test_module, Some(lib));
    // the test file's own functions, which its tests call
    for d in test_tree.nodes("decls").into_iter().filter(|d| d.kind == "function") {
        l.line("");
        let f = test_module.fns[d.str("name").unwrap()].clone();
        l.function_item(d, &f, "");
    }
    for t in test_tree.nodes("decls").into_iter().filter(|d| d.kind == "test") {
        let name = t.str("name").unwrap();
        l.line("");
        l.line("#[test]");
        l.line(format!("fn {name}() {{"));
        l.ind += 1;
        if let Some(c) = t.node("covers") {
            l.line(format!("// y: covers {}", c.str("text").unwrap()));
        }
        l.reset_scopes();
        l.ctx = Ctx { module: m.name.clone(), name: name.to_string(), result: None, fails: None, branches: vec![], section: None };
        for s in t.nodes("sections") {
            let section = s.str("name").unwrap();
            l.ctx.section = Some(section.to_string());
            for st in s.nodes("body") {
                if section == "then" && st.kind == "let" {
                    let c = l.expr(st.node("value").unwrap());
                    let label = st.str("name").unwrap();
                    let msg = match comment_above(&l, st.line) {
                        Some(c) => format!("{label}: {c}"),
                        None => label.to_string(),
                    };
                    l.line(format!("assert!({}, \"{msg}\");", c.code));
                } else {
                    // a statement of `given`, `inject` or `when`, in the test's one scope
                    l.stmt_in_test(st);
                }
            }
        }
        l.ind -= 1;
        l.line("}");
    }
    let mut out = vec![format!("//! Lowered from `{y_path}`. Each test says, in a comment, the path its Y source declares it covers.")];
    out.push("//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test.".into());
    out.push(String::new());
    out.push("use super::*;".to_string());
    // what the module's file imports, its tests have through `super::*`
    let module_code = module_file(m, "");
    let module_imports = imports_for(&module_code.replace("use super::*;", ""));
    let mut found = imports_for(&l.out.join("\n"));
    found.retain(|u| !m.types.keys().any(|t| u.ends_with(&format!("::{}", pascal(t)))) && !module_imports.contains(u));
    out.extend(header(&found));
    out.extend(l.out);
    out.join("\n") + "\n"
}

impl<'a> Lowerer<'a> {
    /// A statement of a test, whose names stay in scope for the sections after it.
    pub fn stmt_in_test(&mut self, st: &Node) {
        // `block` opens a scope; a test's statements share one, so lower each as a block of one and rebind
        self.stmt_pub(st);
    }
}

/// `main`: the entry point, which makes the root capability.
pub fn main_file(main: &Module, lib: &Module, lib_crate: &str, y_path: &str) -> String {
    let mut l = Lowerer::new(main, Some(lib));
    let mut imports: Vec<String> = vec![];
    for d in main.tree.nodes("decls").into_iter().filter(|d| d.kind == "import") {
        let from = d.str("from").unwrap_or("");
        let mut by_facet: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        if let Some(y_parse::Value::List(ns)) = d.get("names") {
            for n in ns {
                if let y_parse::Value::Str(n) = n {
                    let (facet, item) = n.rsplit_once('.').unwrap_or(("", n));
                    let item = if lib.types.contains_key(item) { pascal(item) } else { item.to_string() };
                    by_facet.entry(facet.replace('.', "::")).or_default().push(item);
                }
            }
        }
        for (facet, mut items) in by_facet {
            items.sort_by(|a, b| (a.chars().next().is_some_and(char::is_uppercase), a).cmp(&(b.chars().next().is_some_and(char::is_uppercase), b)));
            let path = if facet.is_empty() { format!("{lib_crate}::{from}") } else { format!("{lib_crate}::{from}::{facet}") };
            if items.len() == 1 {
                imports.push(format!("use {path}::{};", items[0]));
            } else {
                imports.push(format!("use {path}::{{{}}};", items.join(", ")));
            }
        }
    }
    for d in main.tree.nodes("decls").into_iter().filter(|d| d.kind == "function") {
        let f = main.fns[d.str("name").unwrap()].clone();
        for line in doc_lines(d, Some(&f)) {
            l.line(line);
        }
        let rt = match &f.fails {
            Some(e) => format!(" -> Result<(), {}>", l.rust_type(e)),
            None => String::new(),
        };
        l.line(format!("fn main(){rt} {{"));
        l.ind += 1;
        l.reset_scopes();
        l.ctx = Ctx { module: main.name.clone(), name: "main".into(), result: None, fails: f.fails.clone(), branches: vec![], section: None };
        for p in f.params.iter().filter(|p| p.injected) {
            l.use_("y_runtime::System");
            l.line(format!("let {} = System::root();", p.name));
            l.bind(&p.name, p.ty.clone(), Form::Place);
        }
        l.block(&d.nodes("body"), false);
        if f.fails.is_some() {
            l.line("Ok(())");
        }
        l.ind -= 1;
        l.line("}");
    }
    let mut out = vec![format!("//! Lowered from `{y_path}`, the Y module `main`: the entry point."), String::new()];
    imports.sort();
    let mut all: Vec<String> = imports;
    // the module's own names that main uses are imported above; the runtime's here
    let mut uses = imports_for(&l.out.join("\n"));
    uses.retain(|u| !all.iter().any(|a| a.contains(u.rsplit("::").next().unwrap())));
    for h in header(&uses) {
        if !all.iter().any(|a| a == &h) {
            all.push(h);
        }
    }
    out.extend(all);
    out.push(String::new());
    out.extend(l.out);
    out.join("\n") + "\n"
}

pub fn lib_file(module: &str) -> String {
    format!("//! Lowered from the Y package in `y/`: its one library module, `{module}`. The entry point is `src/main.rs`.\n\npub mod {module};\n")
}

pub fn emit(m: &Module, test_tree: &Node, test_module: &Module, main: &Module, lib_crate: &str) -> Emitted {
    let name = m.name.clone();
    let (module, copies) = module_out(m, &format!("y/src/{name}.y"));
    Emitted {
        copies,
        lib: lib_file(&name),
        module,
        tests: tests_file(m, test_tree, test_module, &format!("y/src/{name}.test.y")),
        main: main_file(main, m, lib_crate, "y/src/main.y"),
    }
}

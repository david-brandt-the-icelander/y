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

/// The documentation of a declaration as `///` lines, for the other lowerings.
pub fn doc_lines_of(d: &Node, f: Option<&FnInfo>) -> Vec<String> {
    doc_lines(d, f)
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
    pub invariant_sites: Vec<(String, String, String, String)>,
    pub regions: Regions,
    pub main_regions: Regions,
    pub main_copies: Vec<(usize, String, String)>,
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
        ("ConstraintViolation", "y_runtime::ConstraintViolation"),
        ("Window", "y_window::Window"),
        ("Colour", "y_window::Colour"),
        ("Key", "y_window::Key"),
        ("Point", "y_window::Point"),
        ("Picture", "y_window::Picture"),
        ("Duration", "std::time::Duration"),
        ("Instant", "std::time::Instant"),
        ("block_on", "y_window::block_on"),
        ("SystemWindow", "y_window::SystemWindow"),
        ("SystemHttp", "y_http::SystemHttp"),
        ("Http", "y_http::Http"),
        ("HttpError", "y_http::HttpError"),
        ("Within", "y_runtime::Within"),
        ("Deadline", "y_runtime::Deadline"),
        ("Expired", "y_runtime::Expired"),
        ("PictureError", "y_window::PictureError"),
        ("Frame", "y_window::Frame"),
        ("time", "y_runtime::time"),
        ("Ordering", "std::cmp::Ordering"),
        ("LazyLock", "std::sync::LazyLock"),
        ("Decimal", "y_runtime::decimal::Decimal"),
        ("Integer", "y_runtime::decimal::Integer"),
        ("Rational", "y_runtime::decimal::Rational"),
        ("Rounding", "y_runtime::decimal::Rounding"),
        ("Big", "y_runtime::decimal::Big"),
        ("I256", "y_runtime::decimal::I256"),
        ("Inexact", "y_runtime::decimal::Inexact"),
        ("UNBOUNDED", "y_runtime::decimal::UNBOUNDED"),
        ("AsciiString", "y_string::AsciiString"),
        ("Bag", "y_collections::Bag"),
        ("Queue", "y_collections::Queue"),
        ("SortedMap", "y_collections::SortedMap"),
        ("Shared", "y_runtime::shared::Shared"),
        ("Tx", "y_runtime::shared::Tx"),
        ("Step", "y_runtime::shared::Step"),
        ("Isolation", "y_runtime::shared::Isolation"),
        ("Aborted", "y_runtime::shared::Aborted"),
        ("Undelivered", "y_runtime::command::Undelivered"),
        ("Submit", "y_runtime::command::Submit"),
        ("Outbox", "y_runtime::command::Outbox"),
        ("Command", "y_runtime::command::Command"),
        ("Policy", "y_runtime::command::Policy"),
        ("OnAbandon", "y_runtime::command::OnAbandon"),
        ("Sensitive", "y_runtime::claims::Sensitive"),
        ("Reveal", "y_runtime::claims::Reveal"),
        ("assertion", "y_runtime::assertion"),
        ("atomic", "y_runtime::shared::atomic"),
        ("atomic_with", "y_runtime::shared::atomic_with"),
        ("orelse", "y_runtime::shared::orelse"),
        ("Infallible", "std::convert::Infallible"),
        ("TaskBody", "y_runtime::task::TaskBody"),
        ("Supervision", "y_runtime::task::Supervision"),
        ("Restart", "y_runtime::task::Restart"),
        ("supervised", "y_runtime::task::supervised"),
        ("Cancellation", "y_runtime::task::Cancellation"),
        ("detach", "y_runtime::task::detach"),
        ("Arena", "y_runtime::arena::Arena"),
        ("Held", "y_runtime::arena::Held"),
        ("Index", "y_runtime::arena::Index"),
        ("Schema", "y_runtime::store::Schema"),
        ("Store", "y_runtime::store::Store"),
        ("StoreError", "y_runtime::store::StoreError"),
        ("require", "y_runtime::require"),
        ("ensure", "y_runtime::ensure"),
        ("invariant", "y_runtime::invariant"),
        ("not_reachable", "y_runtime::not_reachable"),
        ("not_implemented", "y_runtime::not_implemented"),
        ("narrowed", "y_runtime::narrowed"),
        ("narrowed_ok", "y_runtime::narrowed_ok"),
        ("narrowed_err", "y_runtime::narrowed_err"),
    ] {
        let used = match name {
            // a trait's method: `system.window()`
            "SystemWindow" => code.contains(".window()"),
            "SystemHttp" => code.contains(".http("),
            "time" => code.contains("time::whole_seconds"),
            _ => words.contains(name) && (name.chars().next().unwrap().is_uppercase() || code.contains(&format!("{name}("))),
        };
        if used {
            out.insert(path.to_string());
        }
    }
    if code.contains("list!") {
        out.insert("y_collections::list".into());
    }
    if code.contains("number::to_") {
        out.insert("y_runtime::number".into());
    }
    if code.contains("observe::") {
        out.insert("y_runtime::observe".into());
    }
    if code.contains("pattern::matches") {
        out.insert("y_runtime::pattern".into());
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
    pub fn signature(&mut self, f: &FnInfo, vis: &str) -> String {
        self.fn_signature(f, vis)
    }

    fn fn_signature(&mut self, f: &FnInfo, vis: &str) -> String {
        let mut ps = vec![];
        // a method's `me`: `&self`, or `&mut self` for a `mutable function` (Y § 3.6)
        if let Some((_, mutable)) = &f.me {
            ps.push(if *mutable { "&mut self".to_string() } else { "&self".to_string() });
        }
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
        let asynchronous = if self.m.suspending.contains(&f.name) { "async " } else { "" };
        format!("{vis}{asynchronous}fn {}({}){rt}", f.name, ps.join(", "))
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
            section: None, ..Default::default()
        };
        for p in &f.params {
            let form = match p.mode {
                Mode::Ref => Form::Ref,
                Mode::Mut => Form::Mut,
                Mode::Value => Form::Place,
            };
            self.bind(&p.name, p.ty.clone(), form);
        }
        self.bind_me(f);
        // `native because "…"`: the body is the module's native implementation, called with the same arguments
        if f.native.is_some() {
            let args: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
            self.line(format!("crate::{}::native::{}({}) // y: native because \"{}\"", self.m.name, f.name, args.join(", "), f.native.clone().unwrap_or_default()));
            self.ind -= 1;
            self.line("}");
            return;
        }
        // what `old(…)` reads in an `ensures`: the value when the function was called (Y § 3.11)
        self.bind_olds(d);
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
            let r = f.result.as_ref().map(|r| r.0.clone()).unwrap_or_else(|| "result".into());
            self.ctx.ensures = ensures.iter().map(|n| (*n).clone()).collect();
            self.ctx.result_name = r.clone();
            // a function without failures whose last statement gives its value: the value is bound to the result's
            // name, checked, then given; every other `return` checks it where it returns
            let tail_value = f.fails.is_none() && f.result.is_some() && body.last().is_some_and(|l| matches!(l.kind, "return" | "if" | "conditional" | "compare"));
            if ensures.is_empty() {
                self.block(&body, true);
            } else if f.fails.is_some() && body.last().is_some_and(|l| l.kind == "return") {
                // a function that may fail: the value it gives is checked, then given as its result
                self.push_scope();
                let (init, last) = body.split_at(body.len() - 1);
                self.block_open_tail(init);
                let want = f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unknown);
                let x = self.expr_want(last[0].node("value").unwrap(), &want);
                let v = self.owned(x, &want);
                self.line(format!("let {r} = {v};"));
                self.bind(&r, want, Form::Place);
                self.ensure_lines();
                self.line(format!("Ok({r})"));
                self.pop_scope();
                self.ind -= 1;
                self.line("}");
                self.ctx.ensures.clear();
                return;
            } else if tail_value {
                self.push_scope();
                self.block_open_tail(&body);
                let start = self.last_start;
                let ind = "    ".repeat(self.ind);
                let end = self.out.len() - 1;
                // the statement's first line, bound: on one line, ended; on several, ended at its last
                self.out[start] = if start == end { format!("{ind}let {r} = {};", self.out[start].trim_start()) } else { format!("{ind}let {r} = {}", self.out[start].trim_start()) };
                if !self.out[end].ends_with(';') {
                    self.out[end].push(';');
                }
                self.bind(&r, f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unknown), Form::Place);
                self.ensure_lines();
                self.line(r);
                self.pop_scope();
            } else {
                // each `return` checks what is ensured, and so does the end of a function that has no result
                self.ctx.ensure_tail = true;
                self.block(&body, true);
                if f.result.is_none() && !crate::lower::diverges(&body) {
                    self.ensure_lines();
                }
            }
            self.ctx.ensures.clear();
            if f.fails.is_some() && f.result.is_none() && !crate::lower::diverges(&body) {
                // a resource held to the end is released, and its release is the function's result
                match self.held.take() {
                    Some(h) => self.line(format!("{h}.end()")),
                    None => self.line("Ok(())"),
                }
            }
            self.held = None;
        }
        self.ind -= 1;
        self.line("}");
        // the lambdas and nested functions of the body, each a function of its own after it
        let after = std::mem::take(&mut self.after_fn);
        let base = "    ".repeat(self.ind);
        for l in after {
            self.out.push(if l.is_empty() { l } else { format!("{base}{l}") });
        }
        self.locals_fns.clear();
    }

    fn type_item(&mut self, d: &Node, facet: bool) {
        let name = d.str("name").unwrap();
        let m = self.m;
        let info = &m.types[name];
        // a unit is erased, and a trait is written by `trait_item`: neither is an item here
        if matches!(info.def, TypeDef::Unit { .. } | TypeDef::Trait) {
            return;
        }
        for l in doc_lines(d, None) {
            self.line(l);
        }
        if let TypeDef::Alias(base) = &info.def {
            let vis = if facet { "pub " } else { "" };
            let t = self.rust_type(base);
            self.line(format!("{vis}type {} = {t};", pascal(name)));
            return;
        }
        let vis = if facet { "pub " } else { "" };
        match &info.def {
            TypeDef::Enumeration { .. } => return self.enumeration_item(name, vis),
            TypeDef::State { .. } => return self.state_item(name, vis),
            TypeDef::Command { .. } => return self.command_item(name, vis),
            TypeDef::Native { .. } => return self.native_type_item(name, vis),
            // a kind of collection is the runtime's kind with the same constraints (Y § 3.13)
            TypeDef::Collection { kind, clauses } => {
                if kind.is_empty() {
                    self.error(format!("`type {name}`: no kind of the runtime keeps the constraints {}", clauses.join(", ")));
                }
                self.line(format!("/// The kind of collection whose constraints are: {}.", clauses.join("; ")));
                let params = if d.node("type").and_then(|t| t.str("name")) == Some("mapping") { "K, V" } else { "T" };
                self.line(format!("{vis}type {}<{params}> = y_collections::{kind}<{params}>;", pascal(name)));
                return;
            }
            // a unit is erased: its values are its base type's (Y § 3.3)
            TypeDef::Unit { .. } | TypeDef::Trait => return,
            _ => {}
        }
        let copy = crate::model::is_copy(m, &Ty::Named(name.to_string()));
        let eq = crate::model::has_eq(m, &Ty::Named(name.to_string()));
        let mut derives: Vec<&str> = vec!["Clone"];
        if copy {
            derives.push("Copy");
        }
        derives.push("Debug");
        derives.push("PartialEq");
        if eq {
            derives.push("Eq");
        }
        // a structure that holds an index into an arena is the arena's own, and derives nothing
        let indexed = matches!(&info.def, TypeDef::Struct { fields, .. } if fields.iter().any(|(_, t)| matches!(t, Ty::Index(_)) || matches!(t, Ty::Opt(x) if matches!(**x, Ty::Index(_)))));
        // a `transient` field is left out of equality (Y § 3.3): `PartialEq` is written out, without it
        let transient: Vec<String> = d.nodes("members").iter().filter(|x| x.flag("transient")).filter_map(|x| x.str("name").map(str::to_string)).collect();
        if !transient.is_empty() {
            derives.retain(|x| *x != "PartialEq" && *x != "Eq" && *x != "Hash");
        }
        if !indexed {
            self.line(format!("#[derive({})]", derives.join(", ")));
        }
        let vis = if facet { "pub " } else { "" };
        match &info.def {
            TypeDef::Struct { fields, derived, derived_values, .. } => {
                self.line(format!("{vis}struct {} {{", pascal(name)));
                self.ind += 1;
                let fvis = if m.exported.contains(name) { "pub " } else if facet { "pub(super) " } else { "" };
                for (f, t) in fields {
                    let t = if *t == Ty::StrLit { "&'static str".to_string() } else { self.rust_type(t) };
                    self.line(format!("{fvis}{f}: {t},"));
                }
                self.ind -= 1;
                self.line("}");
                if !transient.is_empty() {
                    let kept: Vec<String> = fields.iter().filter(|(f, _)| !transient.contains(f)).map(|(f, _)| format!("self.{f} == other.{f}")).collect();
                    self.line("");
                    self.line(format!("/// Equality leaves out the `transient` fields: {}.", transient.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ")));
                    self.line(format!("impl PartialEq for {} {{", pascal(name)));
                    self.line("    fn eq(&self, other: &Self) -> bool {");
                    self.line(format!("        {}", if kept.is_empty() { "true".to_string() } else { kept.join(" && ") }));
                    self.line("    }");
                    self.line("}");
                }
                if m.schemas.contains_key(name) {
                    self.schema_impl(name);
                }
                // `derived miss_count is … = misses.count()`: a method, recomputed on every read
                if !derived.is_empty() {
                    let fnames: Vec<String> = fields.iter().map(|f| f.0.clone()).collect();
                    self.line("");
                    self.line(format!("impl {} {{", pascal(name)));
                    self.ind += 1;
                    for (k, ((dn, dt), (_, v))) in derived.iter().zip(derived_values).enumerate() {
                        if k > 0 {
                            self.line("");
                        }
                        let t = self.rust_type(dt);
                        self.line(format!("pub fn {dn}(&self) -> {t} {{"));
                        self.ind += 1;
                        self.reset_scopes();
                        self.bind("self", Ty::Named(name.to_string()), Form::Ref);
                        let v = crate::lower::with_fields(v, &fnames, "self");
                        let x = self.expr(&v);
                        let code = self.owned(x, dt);
                        self.line(code);
                        self.ind -= 1;
                        self.line("}");
                    }
                    self.ind -= 1;
                    self.line("}");
                }
            }
            TypeDef::Newtype { base, wheres } => {
                let (en, bt) = (pascal(name), self.rust_type(base));
                self.line(format!("{vis}struct {en}({bt});"));
                self.line("");
                self.line(format!("impl {en} {{"));
                self.ind += 1;
                self.line(format!("/// Makes a `{name}`, checking each of its constraints in order."));
                self.line(format!("pub fn new(value: {bt}) -> Result<{en}, ConstraintViolation> {{"));
                self.ind += 1;
                self.reset_scopes();
                self.bind("value", base.clone(), Form::Place);
                for (w, _, c) in wheres {
                    let x = self.constraint_check(c);
                    self.line(format!("if !({x}) {{ // y: {en}#{w}"));
                    self.ind += 1;
                    self.line(format!("return Err(ConstraintViolation {{ constraint: \"{}::{en}#{w}\" }});", self.m.name));
                    self.ind -= 1;
                    self.line("}");
                }
                self.line(format!("Ok({en}(value))"));
                self.ind -= 1;
                self.line("}");
                self.line("");
                self.line(format!("/// The `{name}` as the value it refines."));
                self.line(format!("pub fn as_text(&self) -> &{bt} {{"));
                self.ind += 1;
                self.line("&self.0");
                self.ind -= 1;
                self.line("}");
                self.ind -= 1;
                self.line("}");
                self.line("");
                self.line(format!("impl std::fmt::Display for {en} {{"));
                self.ind += 1;
                self.line("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
                self.ind += 1;
                self.line("std::fmt::Display::fmt(&self.0, f)");
                self.ind -= 1;
                self.line("}");
                self.ind -= 1;
                self.line("}");
            }
            TypeDef::Alias(_) | TypeDef::Enumeration { .. } | TypeDef::State { .. } | TypeDef::Command { .. } | TypeDef::Unit { .. } | TypeDef::Native { .. } | TypeDef::Collection { .. } | TypeDef::Trait => {}
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
                // `or fail` on a failure that one case carries converts through that case (Y § 3.4); a wait within a
                // deadline fails with `Within`, which two cases carry between them
                let carried = |t: &str| cases.iter().find(|(_, fs)| matches!(&fs[..], [(_, Ty::Named(x))] if x == t)).map(|(c, fs)| (c.clone(), fs[0].0.clone()));
                let within = carried("http_error").zip(carried("expired"));
                if let Some(((failed, ff), (expired, ef))) = &within {
                    let en = pascal(name);
                    self.line("");
                    self.line(format!("/// Y's `or fail` on a wait within a deadline converts through `{en}`'s `{}` and `{}` cases (Y § 3.4).", pascal(failed), pascal(expired)));
                    self.line(format!("impl From<Within<HttpError>> for {en} {{"));
                    self.line(format!("    fn from(failed: Within<HttpError>) -> {en} {{"));
                    self.line("        match failed {");
                    self.line(format!("            Within::Failed({ff}) => {en}::{} {{ {ff} }},", pascal(failed)));
                    self.line(format!("            Within::Expired({ef}) => {en}::{} {{ {ef} }},", pascal(expired)));
                    self.line("        }");
                    self.line("    }");
                    self.line("}");
                }
                let failures: std::collections::BTreeSet<String> = self.m.fns.values().chain(self.lib.map(|l| l.fns.values()).into_iter().flatten()).filter_map(|f| match &f.fails { Some(Ty::Named(t)) => Some(t.clone()), _ => None }).collect();
                for (c, fs) in cases {
                    if let [(f, Ty::Named(t))] = &fs[..] {
                        let external_error = crate::ty::external(t).is_some() && t.ends_with("_error") && !(within.is_some() && t == "http_error");
                        if external_error || (failures.contains(t) && t != name) {
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
        self.in_group = Some(name.to_string());
        // a value that names anything but a literal reads the module around it
        let plain = |v: &Node| { let mut p = true; v.walk(&mut |n: &Node| if !matches!(n.kind, "literal" | "list") || (n.kind == "literal" && !matches!(n.str("type"), Some("integer" | "string" | "boolean" | "character"))) { p = false; }); p };
        if !d.nodes("items").iter().all(|i| i.node("value").is_none_or(plain)) {
            self.line("use super::*;");
            self.line("");
        }
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
            // a value a Rust constant cannot hold (a list of structures, a bag) is made by a function
            if crate::model::needs_fn(self.module(), v) {
                let want = ty.clone().unwrap_or(Ty::Unknown);
                self.reset_scopes();
                self.ctx.module = self.m.name.clone();
                self.ctx.name = name.to_string();
                let saved = self.in_group.take();
                let x = self.expr_want(v, &want);
                let code = self.owned(x, &want);
                self.in_group = saved;
                let rt = self.rust_type(&want);
                self.line(format!("pub fn {}() -> {rt} {{", i.str("name").unwrap()));
                self.line(format!("    {code}"));
                self.line("}");
                continue;
            }
            let code = if v.kind == "list" {
                self.const_value(v)
            } else {
                let x = self.expr(v);
                self.owned(x, &t)
            };
            let rt = if t == Ty::StrLit { "&str".to_string() } else { self.rust_type(&t) };
            self.line(format!("pub const {}: {rt} = {code};", upper(i.str("name").unwrap())));
        }
        self.in_group = None;
        let _ = std::mem::replace(&mut self.uses, saved);
        let inner: std::collections::BTreeSet<String> = Default::default();
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

    /// A newtype's constraint as Rust: `value.matches(pattern"^[a-z]*$")` is a test of each rune.
    fn constraint_check(&mut self, c: &Node) -> String {
        if c.kind == "call" {
            let callee = c.node("callee").unwrap();
            if callee.str("name") == Some("matches") {
                let obj = self.expr(callee.node("object").unwrap());
                let pat = c.nodes("args")[0].node("value").unwrap().str("value").unwrap_or("").to_string();
                return match pat.as_str() {
                    "^[a-z]*$" => format!("{}.runes().all(|letter| letter.is_ascii_lowercase())", obj.code),
                    "^[A-Z]*$" => format!("{}.runes().all(|letter| letter.is_ascii_uppercase())", obj.code),
                    _ => self.pattern_match(&pat, obj),
                };
            }
        }
        self.expr(c).code
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
            "function" | "rule" | "transition" | "handler" | "workflow" => {
                let f = self.m.fns[d.str("name").unwrap()].clone();
                if f.kind == "workflow" {
                    self.workflow_check(&f);
                }
                self.function_item(d, &f, if facet { "pub " } else { "" });
            }
            "structure" | "type_decl" | "enumeration" | "state" | "command" | "unit" => self.type_item(d, facet),
            "trait" => {
                for l in doc_lines(d, None) {
                    self.line(l);
                }
                self.trait_item(d, if facet { "pub " } else { "" })
            }
            "implementation" => self.implementation_item(d),
            // `allow(code) because "…"` above a declaration: a claim, kept as a marker
            "allow" => self.line(format!("// y: allow({}) because \"{}\"", d.str("code").unwrap_or(""), d.str("because").unwrap_or(""))),
            // a schema is its latest version's structure; the earlier versions are read by its `Schema` impl
            "schema" if self.m.schemas.get(d.str("name").unwrap()).is_some_and(|s| s.versions.iter().map(|v| v.0).max() == d.str("version").and_then(|v| v.trim_start_matches('v').parse().ok())) => self.type_item(d, facet),
            "constants" | "service" => {
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
/// Each region: its function, its name, its Y line and its constraints as written.
pub type Regions = Vec<(String, String, usize, Vec<String>)>;

/// Each copy the lowering made: its Y line, what it copies, and why.
pub type Copies = Vec<(usize, String, String)>;

/// Each structure invariant checked after a write: the type, the invariant, the function and the field written.
pub type InvariantSites = Vec<(String, String, String, String)>;

pub fn module_out(m: &Module, y_path: &str) -> (String, Copies, InvariantSites, Regions) {
    // a first pass finds the closed types an interpolation shows, which the second gives a `Display`
    let shown = module_pass(m, y_path, Default::default()).shown;
    let l = module_pass(m, y_path, shown);
    let copies = l.copies.clone();
    let sites = l.invariant_sites.clone();
    let regions = l.regions.clone();
    (module_text(m, y_path, l), copies, sites, regions)
}

fn module_pass<'a>(m: &'a Module<'a>, _y_path: &str, display: std::collections::BTreeSet<String>) -> Lowerer<'a> {
    let mut l = Lowerer::new(m, None);
    l.mark_copies = true;
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
            let doc = d.str("comment").map(|c| c.lines().map(|l| l.trim_start_matches("--").trim()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
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

fn module_text(m: &Module, y_path: &str, mut l: Lowerer) -> String {
    for e in l.rendered.clone() {
        l.rendered_helper(&e);
    }
    let mut out = vec![format!("//! Lowered from `{y_path}`, the Y module `{}`.", m.name)];
    out.push("//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet.".into());
    out.push("//! Each named decision of the Y source is marked `// y: #name` where its condition is tested.".into());
    out.push(String::new());
    let mut body = [l.statics.join("\n"), l.out.join("\n"), l.compile_tests.join("\n")].join("\n");
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
    // the module's native implementations (Y § 3.18), in the package's folder for its backend
    let native = { let mut n = false; m.tree.walk(&mut |x: &Node| if x.str("native").is_some() { n = true; }); n };
    if native {
        out.push(format!("/// The native implementations of the module's `native` declarations, written by hand (Y § 3.18)."));
        out.push(format!("#[path = \"../native/rust/{}.rs\"]", m.name));
        out.push("mod native;".into());
        out.push(String::new());
    }
    out.extend(l.statics.clone());
    out.extend(l.out.clone());
    compiled_tests(&mut out, &l.compile_tests);
    out.join("\n") + "\n"
}

/// The tests generated for a file's `compile`s: each call, made at run time, gives the value the build wrote down.
fn compiled_tests(out: &mut Vec<String>, tests: &[String]) {
    if tests.is_empty() {
        return;
    }
    out.push(String::new());
    out.push("/// Generated for every `compile`: the run-time call gives the value the build wrote down (Y § 3.17).".into());
    out.push("#[cfg(test)]".into());
    out.push("mod compiled {".into());
    out.push("    use super::*;".into());
    for t in tests {
        out.push(String::new());
        out.extend(t.lines().map(|l| format!("    {l}")));
    }
    out.push("}".into());
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
    l.file = y_path.to_string();
    l.block_on = true;
    // the test file's own constants and functions, which its tests use
    for d in test_tree.nodes("decls").into_iter().filter(|d| d.kind == "constants") {
        l.line("");
        l.const_item(d, false);
    }
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
        l.ctx = Ctx { module: m.name.clone(), name: name.to_string(), result: None, fails: None, branches: vec![], section: None, ..Default::default() };
        for s in t.nodes("sections") {
            let section = s.str("name").unwrap();
            l.ctx.section = Some(section.to_string());
            if section == "faults" {
                // what the test forces where the code under test runs (Y § 3.14), through the runtime's stand-in
                for f in s.nodes("body") {
                    let at = f.str("at").unwrap_or("");
                    let times = f.node("times").and_then(|t| t.str("text")).unwrap_or("1");
                    let kind = match f.str("fault") {
                        Some("conflict") => "Conflict",
                        Some("expire") => "Expire",
                        Some(other) => {
                            l.copy_line = f.line;
                            l.error(format!("`{other} …`: y-compile's runtime stand-in forces `conflict at` and `expire at` an atomic region, and not `{other}` (y_runtime::faults)"));
                            continue;
                        }
                        None => continue,
                    };
                    l.line(format!("y_runtime::faults::force(y_runtime::faults::Kind::{kind}, \"{}::{at}\", {times}); // y: faults {} at {at}", m.name, f.str("fault").unwrap_or("")));
                }
                continue;
            }
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
    main_out(main, lib, lib_crate, y_path).0
}

/// `main`'s file, and its regions.
pub fn main_out(main: &Module, lib: &Module, lib_crate: &str, y_path: &str) -> (String, Regions) {
    let mut l = Lowerer::new(main, Some(lib));
    l.file = y_path.to_string();
    let mut imports: Vec<String> = vec![];
    for d in main.tree.nodes("decls").into_iter().filter(|d| d.kind == "import") {
        let from = d.str("from").unwrap_or("");
        // the standard library's names are the runtime's, imported where the code uses them
        if from.starts_with("standard.") {
            continue;
        }
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
    for d in main.tree.nodes("decls").into_iter().filter(|d| matches!(d.kind, "type_decl" | "structure")) {
        l.type_item(d, false);
        l.line("");
    }
    for d in main.tree.nodes("decls").into_iter().filter(|d| d.kind == "constants") {
        l.const_item(d, false);
        l.line("");
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
        // a window opens before any code runs: `main` opens it with the constants `system.window(…)` names, and runs
        // the entry point, which may suspend, on the window's executor
        let mut window_args = None;
        d.walk(&mut |n: &Node| {
            if n.kind == "call" && n.node("callee").is_some_and(|c| c.kind == "member" && c.str("name") == Some("window")) {
                window_args = Some(n.nodes("args").iter().map(|a| a.node("value").unwrap().clone()).collect::<Vec<Node>>());
            }
        });
        if let Some(args) = &window_args {
            let parts: Vec<String> = args.iter().map(|a| l.expr(a).code).collect();
            l.line("/// The window opens before any code runs, so the title and size that `system.window(…)` names open it here,");
            l.line("/// and the entry point runs on the window's executor.");
            l.line("fn main() {");
            l.line(format!("    y_window::run({}, entry());", parts.join(", ")));
            l.line("}");
            l.line("");
            let at = l.out.len() - 6;
            let docs: Vec<String> = l.out.drain(..at).collect();
            l.out.extend(docs);
            l.line(format!("async fn entry(){rt} {{"));
            l.use_("y_window::SystemWindow");
        } else {
            l.line(format!("fn main(){rt} {{"));
        }
        l.ind += 1;
        l.reset_scopes();
        l.ctx = Ctx { module: main.name.clone(), name: "main".into(), result: None, fails: f.fails.clone(), branches: vec![], section: None, ..Default::default() };
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
    let mut uses = imports_for(&[l.out.join("\n"), l.statics.join("\n"), l.compile_tests.join("\n")].join("\n"));
    uses.retain(|u| !all.iter().any(|a| a.contains(u.rsplit("::").next().unwrap())));
    // the module's types a `compile`'s value is built as, which `main` names only there
    let statics = l.statics.join("\n");
    for (t, info) in &lib.types {
        let rust = pascal(t);
        let named = statics.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == rust);
        if named && !all.iter().any(|a| a.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == rust)) {
            let path = match &info.facet {
                Some(f) => format!("{lib_crate}::{}::{f}::{rust}", lib.name),
                None => format!("{lib_crate}::{}::{rust}", lib.name),
            };
            all.push(format!("use {path};"));
        }
    }
    for h in header(&uses) {
        if !all.iter().any(|a| a == &h) {
            all.push(h);
        }
    }
    out.extend(all);
    out.push(String::new());
    let regions = l.regions.clone();
    out.extend(l.statics.clone());
    out.extend(l.out.clone());
    compiled_tests(&mut out, &l.compile_tests);
    (out.join("\n") + "\n", regions)
}

pub fn lib_file(module: &str) -> String {
    format!("//! Lowered from the Y package in `y/`: its one library module, `{module}`. The entry point is `src/main.rs`.\n\npub mod {module};\n")
}

pub fn emit(m: &Module, test_tree: &Node, test_module: &Module, main: &Module, lib_crate: &str) -> Emitted {
    let name = m.name.clone();
    let (module, copies, invariant_sites, regions) = module_out(m, &format!("y/src/{name}.y"));
    let (main_text, main_regions) = main_out(main, m, lib_crate, "y/src/main.y");
    Emitted {
        regions,
        copies,
        invariant_sites,
        main_copies: vec![],
        lib: lib_file(&name),
        module,
        tests: tests_file(m, test_tree, test_module, &format!("y/src/{name}.test.y")),
        main: main_text,
        main_regions,
    }
}

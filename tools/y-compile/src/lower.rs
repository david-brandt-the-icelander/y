//! Lowering of Y statements and expressions to Rust text, with the types the lowering infers on the way.
use crate::model::{is_copy, FnInfo, Mode, Module, Param, TypeDef};
use crate::ty::{external, pascal, upper, Ty};
use std::collections::{BTreeMap, BTreeSet};
use y_parse::Node;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Form {
    /// A name or a path that owns its value.
    Place,
    /// A name bound to a shared borrow.
    Ref,
    /// A name bound to an exclusive borrow.
    Mut,
    /// A value computed in place.
    Temp,
    /// A place inside a borrowed value, `game.heaps` of `game: &Game`: borrowed to read, copied to keep.
    Borrowed,
}

#[derive(Clone, Debug)]
pub struct Var {
    pub ty: Ty,
    pub form: Form,
    /// The arm of a `match` on a result that this name is bound in: `Some(true)` for `Ok`, `Some(false)` for `Err`.
    pub arm: Option<bool>,
    /// The Rust name the Y name is read by: in an atomic region, a shared name's value read in the transaction.
    pub alias: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Ex {
    pub code: String,
    pub ty: Ty,
    pub form: Form,
    pub prec: u8,
}

const ATOM: u8 = 100;

fn ex(code: impl Into<String>, ty: Ty, form: Form, prec: u8) -> Ex {
    Ex { code: code.into(), ty, form, prec }
}

fn wrap(e: &Ex, min: u8) -> String {
    if e.prec < min {
        format!("({})", e.code)
    } else {
        e.code.clone()
    }
}

#[derive(Default)]
pub struct Ctx {
    pub module: String,
    pub name: String,
    pub result: Option<Ty>,
    pub fails: Option<Ty>,
    pub branches: Vec<String>,
    /// In a test: `or fail` lowers to `expect` with the section's name.
    pub section: Option<String>,
    /// The function's `ensures` clauses, checked where it gives its result.
    pub ensures: Vec<Node>,
    /// Whether a `return` in the function's tail checks them too, or the function checks its tail's value itself.
    pub ensure_tail: bool,
    /// The function's result's Y name.
    pub result_name: String,
    /// The names the function returns as its result as they are (`return r = name`): an empty collection bound to one
    /// takes the result's type.
    pub returned: Vec<String>,
}

pub struct Lowerer<'a> {
    pub m: &'a Module<'a>,
    /// The library module a `main` or a test file uses.
    pub lib: Option<&'a Module<'a>>,
    pub out: Vec<String>,
    pub ind: usize,
    pub(crate) scopes: Vec<BTreeMap<String, Var>>,
    pub uses: BTreeSet<String>,
    pub ctx: Ctx,
    /// Comments of the file being lowered, by line.
    pub comments: BTreeMap<usize, String>,
    /// The closed types shown in an interpolation, which implement `Display`.
    pub shown: BTreeSet<String>,
    /// The copies the lowering made: the Y line, what is copied, and why.
    pub copies: Vec<(usize, String, String)>,
    /// The closed types to give a `Display`, found by an earlier pass.
    pub display: BTreeSet<String>,
    /// The statements after it in its block.
    pub rest: Vec<Node>,
    /// The statements after it in its block and the blocks around it, up to the function's.
    pub(crate) rest_outer: Vec<Node>,
    /// The loops it is in, whose bodies run again.
    pub(crate) loops: Vec<Node>,
    /// Where the output of the last statement lowered starts.
    pub last_start: usize,
    /// The function whose call is being lowered.
    pub(crate) calling: Option<String>,
    /// The constant group being lowered.
    pub in_group: Option<String>,
    /// In a test: a call that may suspend is run to its end with `block_on`.
    pub block_on: bool,
    /// Markers for the next line written: the copies its statement makes.
    pub(crate) pending: Vec<String>,
    /// Each region: its function, its name, its Y line and its constraints as written.
    pub regions: Vec<(String, String, usize, Vec<String>)>,
    /// The decimal type a value is wanted as, for a rounding that names only its scale.
    pub want_decimal: Option<(u32, u32)>,
    /// The arenas' element types whose `rendered()` the module calls.
    pub rendered: BTreeSet<String>,
    /// In an atomic region: the shared names it changes, written back before it gives its value.
    pub atomic_result: Option<Vec<String>>,
    /// Whether a copy of a borrowed value is marked and listed: in the module's own code, not in its tests.
    pub mark_copies: bool,
    /// The Y line of the statement being lowered, where a copy it makes is listed.
    pub copy_line: usize,
    /// A resource held until the function ends (`batch = filed.batch()`), whose release is the function's result.
    pub held: Option<String>,
    /// Each structure invariant checked after a write: the type, the invariant, the function and the field written.
    pub invariant_sites: Vec<(String, String, String, String)>,
    /// What the source asks for that the lowering cannot give, each with its Y line: compile errors.
    pub errors: Vec<(usize, String)>,
    /// The Y file being lowered, for the errors: `y/src/hangman.y`.
    pub file: String,
    /// The constants and statics that hold the values of the file's `compile`s, and the tests that check each against
    /// its call at run time.
    pub statics: Vec<String>,
    pub compile_tests: Vec<String>,
    /// The name the statement being lowered binds, and the names `compile`s have bound in the function.
    pub binding: Option<String>,
    pub compile_bound: Vec<String>,
    /// The functions a function's lambdas and nested functions lower to, written after it.
    pub after_fn: Vec<String>,
    /// The lambdas and nested functions in scope: what each is, and the names it captures.
    pub locals_fns: BTreeMap<String, (FnInfo, Vec<(String, bool)>)>,
    /// What `old(e)` reads in the function's `ensures`: by `e`'s text, the local the value was kept in.
    pub olds: Vec<(String, String, Ty)>,
}

thread_local! {
    /// Each `compile` of the lowering under way, for the build step that evaluates them (`crate::evaluate`).
    pub static SITES: std::cell::RefCell<Vec<crate::evaluate::Site>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The values the build step gave the `compile`s, by site, once it has run: Rust data.
    pub static COMPILED: std::cell::RefCell<Option<BTreeMap<String, String>>> = const { std::cell::RefCell::new(None) };
    /// The compile errors of the lowering under way, `file:line: message`, which `crate::compile` reports.
    pub static ERRORS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// What a condition shows of a name, where it holds or fails: that an optional is there, or that a result succeeded
/// or failed (Y § 3.3, "A condition narrows").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Narrow {
    Some,
    Ok,
    Err,
}

/// The names a condition narrows where it has the value `truth`: `x != optional.none` and `x == result.ok` where they
/// hold, `x == optional.none` where it does not; through `and` where it holds, `or` where it does not, and `not`.
pub fn narrowings(e: &Node, truth: bool) -> Vec<(String, Narrow)> {
    match (e.kind, e.str("op")) {
        ("unary", Some("not")) => narrowings(e.node("operand").unwrap(), !truth),
        ("binary", Some("and")) if truth => {
            let mut v = narrowings(e.node("left").unwrap(), true);
            v.extend(narrowings(e.node("right").unwrap(), true));
            v
        }
        ("binary", Some("or")) if !truth => {
            let mut v = narrowings(e.node("left").unwrap(), false);
            v.extend(narrowings(e.node("right").unwrap(), false));
            v
        }
        ("binary", Some(op @ ("==" | "!="))) => {
            let (l, r) = (e.node("left").unwrap(), e.node("right").unwrap());
            let Some(x) = name_of(l) else { return vec![] };
            let holds = truth == (op == "==");
            let case = |o: &str, n: &str| r.kind == "member" && is_member(r, o, n) || r.kind == "call" && r.node("callee").is_some_and(|c| is_member(c, o, n));
            if is_member(r, "optional", "none") && !holds {
                return vec![(x.to_string(), Narrow::Some)];
            }
            if is_member(r, "result", "ok") {
                return vec![(x.to_string(), if holds { Narrow::Ok } else { Narrow::Err })];
            }
            if is_member(r, "result", "err") {
                return vec![(x.to_string(), if holds { Narrow::Err } else { Narrow::Ok })];
            }
            // a case with its payload: only where it holds
            if holds && case("result", "ok") {
                return vec![(x.to_string(), Narrow::Ok)];
            }
            if holds && case("result", "err") {
                return vec![(x.to_string(), Narrow::Err)];
            }
            vec![]
        }
        _ => vec![],
    }
}

fn find_forever(e: &Node) -> Option<String> {
    let mut found = None;
    e.walk(&mut |n: &Node| {
        if n.kind == "forever" {
            found = n.str("because").map(str::to_string);
        }
    });
    found
}

fn is_member(e: &Node, obj: &str, name: &str) -> bool {
    e.kind == "member" && e.str("name") == Some(name) && e.node("object").is_some_and(|o| o.kind == "name" && o.str("name") == Some(obj))
}

fn name_of(e: &Node) -> Option<&str> {
    (e.kind == "name").then(|| e.str("name")).flatten()
}

/// Whether a block of statements ends every path through it with `return`, `fail`, `continue` or `break`.
pub fn diverges(stmts: &[&Node]) -> bool {
    let Some(last) = stmts.last() else { return false };
    match last.kind {
        "return" | "fail" | "continue" | "break" => true,
        "if" => diverges(&last.nodes("body")) && last.node("else").is_some_and(|e| diverges(&e.nodes("body"))),
        "conditional" | "compare" => {
            last.nodes("conditions").iter().all(|c| diverges(&c.nodes("body"))) && last.node("else").is_none_or(|e| diverges(&e.nodes("body")))
        }
        _ => false,
    }
}

impl<'a> Lowerer<'a> {
    pub fn new(m: &'a Module<'a>, lib: Option<&'a Module<'a>>) -> Lowerer<'a> {
        let mut comments = BTreeMap::new();
        for c in m.tree.nodes("comments") {
            if c.flag("alone") {
                comments.insert(c.line, c.str("text").unwrap_or("").to_string());
            }
        }
        Lowerer {
            m,
            lib,
            out: vec![],
            ind: 0,
            scopes: vec![BTreeMap::new()],
            uses: BTreeSet::new(),
            ctx: Ctx { module: m.name.clone(), name: String::new(), result: None, fails: None, branches: vec![], section: None, ..Default::default() },
            comments,
            shown: BTreeSet::new(),
            copies: vec![],
            display: BTreeSet::new(),
            rest: vec![],
            rest_outer: vec![],
            loops: vec![],
            last_start: 0,
            calling: None,
            in_group: None,
            block_on: false,
            pending: vec![],
            invariant_sites: vec![],
            regions: vec![],
            want_decimal: None,
            rendered: BTreeSet::new(),
            atomic_result: None,
            held: None,
            mark_copies: false,
            copy_line: 0,
            errors: vec![],
            file: format!("y/src/{}.y", m.name),
            statics: vec![],
            compile_tests: vec![],
            binding: None,
            compile_bound: vec![],
            after_fn: vec![],
            locals_fns: BTreeMap::new(),
            olds: vec![],
        }
    }

    pub fn line(&mut self, s: impl AsRef<str>) {
        let pending: String = std::mem::take(&mut self.pending).concat();
        let s = format!("{}{pending}", s.as_ref());
        let s = s.as_str();
        if s.is_empty() {
            self.out.push(String::new());
        } else {
            self.out.push(format!("{}{}", "    ".repeat(self.ind), s));
        }
    }

    /// A compile error at the statement being lowered.
    pub fn error(&mut self, msg: impl Into<String>) {
        let e = (self.copy_line, msg.into());
        if !self.errors.contains(&e) {
            let text = format!("{}:{}: {}", self.file, e.0, e.1);
            ERRORS.with(|es| if !es.borrow().contains(&text) { es.borrow_mut().push(text) });
            self.errors.push(e);
        }
    }

    pub fn use_(&mut self, path: &str) {
        self.uses.insert(path.to_string());
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn bind(&mut self, name: &str, ty: Ty, form: Form) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), Var { ty, form, arm: None, alias: None });
    }

    /// A Y name read by another Rust name.
    pub fn bind_alias(&mut self, name: &str, ty: Ty, alias: &str) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), Var { ty, form: Form::Place, arm: None, alias: Some(alias.to_string()) });
    }


    /// The Rust that reads a name a condition narrowed, and the type it is then: `narrowed(&x)`, `*narrowed(&x)` for a
    /// copied value, `narrowed_ok(&r)`.
    fn narrowed_read(&self, name: &str, kind: Narrow) -> Option<(String, Ty, Option<bool>)> {
        let v = self.var(name)?;
        let (helper, inner, arm) = match (kind, &v.ty) {
            (Narrow::Some, Ty::Opt(t)) => ("narrowed", (**t).clone(), None),
            (Narrow::Ok, Ty::Res(t, _)) => ("narrowed_ok", (**t).clone(), Some(true)),
            (Narrow::Err, Ty::Res(_, e)) => ("narrowed_err", (**e).clone(), Some(false)),
            _ => return None,
        };
        let code = v.alias.clone().unwrap_or_else(|| name.to_string());
        let arg = if matches!(v.form, Form::Ref | Form::Mut) { code } else { format!("&{code}") };
        Some((format!("{helper}({arg})"), inner, arm))
    }

    /// Binds the names a condition narrows, read through the runtime's `narrowed` where they are used: for the
    /// operands of a chain after the one that narrows, and the conditions of a `conditional` after one that did not hold.
    pub fn bind_narrowed_aliases(&mut self, ns: &[(String, Narrow)]) {
        for (n, k) in ns {
            if let Some((code, ty, arm)) = self.narrowed_read(n, *k) {
                let copied = self.is_copy(&ty) && arm.is_none();
                let (code, form) = if copied { (format!("(*{code})"), Form::Place) } else { (code, Form::Ref) };
                self.scopes.last_mut().unwrap().insert(n.clone(), Var { ty, form, arm, alias: Some(code) });
            }
        }
    }

    /// Binds the names a condition narrows at the start of the block it guards: `let x = narrowed(&x);`.
    pub fn bind_narrowed_lets(&mut self, ns: &[(String, Narrow)], body: &[&Node]) {
        let mut seen = BTreeSet::new();
        for (n, k) in ns {
            let read = body.iter().any(|b| { let mut u = false; b.walk(&mut |x: &Node| if x.kind == "name" && x.str("name") == Some(n.as_str()) { u = true; }); u });
            // the fact ends where the value may change: a name the block assigns or lends `mutable` is not narrowed
            let changed = body.iter().any(|b| {
                let mut c = false;
                b.walk(&mut |x: &Node| {
                    if (x.kind == "assign" && x.node("target").and_then(root_name).as_deref() == Some(n.as_str())) || (x.kind == "mutable" && x.node("value").and_then(root_name).as_deref() == Some(n.as_str())) {
                        c = true;
                    }
                });
                c
            });
            if !seen.insert(n.clone()) || !read || changed {
                continue;
            }
            if let Some((code, ty, arm)) = self.narrowed_read(n, *k) {
                let copied = self.is_copy(&ty) && arm.is_none();
                let (code, form) = if copied { (format!("*{code}"), Form::Place) } else { (code, Form::Ref) };
                self.line(format!("let {n} = {code};"));
                self.scopes.last_mut().unwrap().insert(n.clone(), Var { ty, form, arm, alias: None });
            }
        }
    }

    /// A branch's block, with the names its condition narrows bound at its start.
    fn branch_block_narrowed(&mut self, name: &str, body: &[&Node], tail: bool, ns: &[(String, Narrow)]) {
        self.ctx.branches.push(name.to_string());
        self.ind += 1;
        self.push_scope();
        self.bind_narrowed_lets(ns, body);
        self.stmts(body, tail);
        self.pop_scope();
        self.ind -= 1;
        self.ctx.branches.pop();
    }

    pub fn var(&self, name: &str) -> Option<&Var> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    pub fn reset_scopes(&mut self) {
        self.scopes = vec![BTreeMap::new()];
    }

    /// A function of the module, or of the library a `main` or a test file uses.
    pub fn function(&self, name: &str) -> Option<&'a FnInfo> {
        self.m.fns.get(name).or_else(|| self.lib.and_then(|l| l.fns.get(name)))
    }

    fn type_def(&self, name: &str) -> Option<&'a TypeDef> {
        self.m.types.get(name).or_else(|| self.lib.and_then(|l| l.types.get(name))).map(|t| &t.def)
    }

    pub fn module(&self) -> &'a Module<'a> {
        self.lib.unwrap_or(self.m)
    }

    pub fn rust_type(&mut self, t: &Ty) -> String {
        self.note_type(t);
        self.module().rust_type(t)
    }

    /// Records the imports a type needs.
    pub fn note_type(&mut self, t: &Ty) {
        match t {
            Ty::Str => self.use_("y_string::YString"),
            Ty::Rune => self.use_("y_string::Rune"),
            Ty::List(e) => {
                self.use_("y_collections::List");
                self.note_type(e)
            }
            Ty::Set(e) => {
                self.use_("y_collections::Set");
                self.note_type(e)
            }
            Ty::Opt(e) | Ty::Array(e, _) => self.note_type(e),
            Ty::Res(a, b) => {
                self.note_type(a);
                self.note_type(b)
            }
            Ty::Named(n) | Ty::Cap(n) => {
                if let Some((r, krate)) = external(n) {
                    self.use_(&format!("{krate}::{r}"));
                }
            }
            _ => {}
        }
    }

    fn is_copy(&self, t: &Ty) -> bool {
        is_copy(self.module(), t)
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Expressions

    /// An expression, lowered with a type it is wanted as, where the context gives one.
    pub fn expr_want(&mut self, e: &Node, want: &Ty) -> Ex {
        // `bag.of([…])`: its elements of the type wanted
        if let Ty::Bag(elem) = self.module().resolve(want) {
            if e.kind == "call" && e.node("callee").is_some_and(|c| c.kind == "member" && c.str("name") == Some("of") && c.node("object").and_then(|o| o.str("name")) == Some("bag")) {
                let items: Vec<String> = e.nodes("args")[0].node("value").unwrap().nodes("items").iter().map(|i| { let x = self.expr_want(i, &elem); self.owned(x, &elem) }).collect();
                return ex(format!("Bag::from_iter([{}])", items.join(", ")), want.clone(), Form::Temp, ATOM);
            }
        }
        // `shared.new(x)`: `x` of the type shared
        if let Ty::Shared(inner) = self.module().resolve(want) {
            if e.kind == "call" && e.node("callee").is_some_and(|c| c.kind == "member" && c.str("name") == Some("new") && c.node("object").and_then(|o| o.str("name")) == Some("shared")) {
                let x = self.expr_want(e.nodes("args")[0].node("value").unwrap(), &inner);
                let v = self.owned(x, &inner);
                return ex(format!("Shared::new({v})"), want.clone(), Form::Temp, ATOM);
            }
        }
        // `optional.some(x)`, `result.ok(x)`, `result.err(x)`: `x` of the type the optional or result holds
        let wrapped = |o: &str, n: &str| e.kind == "call" && e.node("callee").is_some_and(|c| is_member(c, o, n)) && e.nodes("args").len() == 1;
        if let Ty::Opt(inner) = self.module().resolve(want) {
            if wrapped("optional", "some") {
                let x = self.expr_want(e.nodes("args")[0].node("value").unwrap(), &inner);
                let v = self.owned(x, &inner);
                return ex(format!("Some({v})"), want.clone(), Form::Temp, ATOM);
            }
        }
        if let Ty::Res(ok, err) = self.module().resolve(want) {
            for (n, t, w) in [("ok", ok, "Ok"), ("err", err, "Err")] {
                if wrapped("result", n) && *t != Ty::Unknown {
                    let x = self.expr_want(e.nodes("args")[0].node("value").unwrap(), &t);
                    let v = self.owned(x, &t);
                    return ex(format!("{w}({v})"), want.clone(), Form::Temp, ATOM);
                }
            }
        }
        if let Ty::Decimal(p, s) = self.module().resolve(want) {
            let saved = self.want_decimal.replace((p, s));
            let x = self.expr_want_inner(e, want);
            self.want_decimal = saved;
            return x;
        }
        self.expr_want_inner(e, want)
    }

    fn expr_want_inner(&mut self, e: &Node, want: &Ty) -> Ex {
        // `list.count()` where a `uint32` or an `int32` is wanted: the runtime's count of that type
        if matches!(want, Ty::U32 | Ty::I32) && e.kind == "call" && e.nodes("args").is_empty() {
            let c = e.node("callee").unwrap();
            if c.kind == "member" && c.str("name") == Some("count") {
                let l = self.expr(c.node("object").unwrap());
                if matches!(l.ty, Ty::List(_)) {
                    self.use_("y_runtime::list");
                    let r = self.by_ref(l.clone(), &l.ty.clone());
                    return if *want == Ty::U32 {
                        ex(format!("list::count({r})"), Ty::U32, Form::Temp, ATOM)
                    } else {
                        ex(format!("list::count_int32({r})"), Ty::I32, Form::Temp, ATOM)
                    };
                }
            }
        }
        match e.kind {
            "list" if e.nodes("items").is_empty() => {
                self.use_("y_collections::List");
                ex("List::new()", want.clone(), Form::Temp, ATOM)
            }
            "list" => {
                let mut elem = want.elem();
                if elem == Ty::Unknown {
                    elem = match self.expr(e.nodes("items")[0]).ty {
                        Ty::StrLit | Ty::HostStr => Ty::Str,
                        Ty::Int => Ty::Unknown,
                        t => t,
                    };
                }
                let items: Vec<String> = e.nodes("items").iter().map(|i| {
                    let x = self.expr_want(i, &elem);
                    self.owned(x, &elem)
                }).collect();
                self.use_("y_collections::list");
                let ty = if matches!(want, Ty::List(_)) { want.clone() } else { Ty::List(Box::new(elem)) };
                ex(format!("list![{}]", items.join(", ")), ty, Form::Temp, ATOM)
            }
            _ => {
                let mut x = self.expr(e);
                if x.ty == Ty::Int && want.is_int() {
                    x.ty = want.clone();
                }
                if matches!(x.ty, Ty::List(ref el) | Ty::Bag(ref el) | Ty::Queue(ref el) if **el == Ty::Unknown) {
                    x.ty = want.clone();
                }
                x
            }
        }
    }

    /// The value of an expression as an owned value of the type wanted.
    pub fn owned(&mut self, x: Ex, want: &Ty) -> String {
        if let Some(c) = self.ware_owned(&x, want) {
            return c;
        }
        // a value in a known state where its state is not known: its state forgotten by its type
        if let (Ty::StateCase(st, _), Ty::Named(w)) = (&x.ty, want) {
            if st == w {
                return format!("{}.forget()", wrap(&x, ATOM));
            }
        }
        // a value where an optional one is wanted: the value, there
        if let Ty::Opt(inner) = want {
            if !matches!(x.ty, Ty::Opt(_) | Ty::Unknown) && x.code != "None" {
                let v = self.owned(x, inner);
                return format!("Some({v})");
            }
        }
        match (want, &x.ty) {
            (Ty::Str, Ty::StrLit) => {
                self.use_("y_string::YString");
                format!("YString::new({})", x.code)
            }
            (Ty::Str, Ty::HostStr) => {
                self.use_("y_string::YString");
                format!("YString::from({})", x.code)
            }
            (Ty::List(e), Ty::Array(a, _)) if **e == Ty::Str && **a == Ty::StrLit => {
                self.use_("y_collections::List");
                format!("List::from({}.map(YString::new))", x.code)
            }
            (Ty::List(_), Ty::Array(..)) => {
                self.use_("y_collections::List");
                format!("List::from({})", x.code)
            }
            _ if matches!(x.form, Form::Ref | Form::Mut) && self.is_copy(&x.ty) => format!("*{}", x.code),
            _ if matches!(x.form, Form::Ref | Form::Mut | Form::Borrowed) && x.ty != Ty::Unknown && !self.is_copy(&x.ty) => {
                // a value kept from one that is borrowed: a copy (Y § 3.2)
                let what = x.code.clone();
                let why = format!("{what} is borrowed, and a value is kept");
                // listed where the Y names it as the Rust does: a name, a member or an element
                let y_like = what.chars().all(|c| c.is_alphanumeric() || "_.[]".contains(c));
                if self.mark_copies && y_like {
                    self.pending.push(format!(" // y: a copy, because {why}"));
                    self.copies.push((self.copy_line, what.clone(), why));
                }
                format!("{}.clone()", wrap(&x, ATOM))
            }
            _ => x.code,
        }
    }

    /// A shared borrow of an expression, as a parameter of the type wanted.
    pub fn by_ref(&mut self, x: Ex, want: &Ty) -> String {
        if let Some(c) = self.ware_owned(&x, want) {
            return format!("&{c}");
        }
        // a value in a known state, lent where its state is not known: a copy of it, its state forgotten
        if let (Ty::StateCase(st, _), Ty::Named(w)) = (&x.ty, want) {
            if st == w {
                return if x.form == Form::Temp { format!("&{}.forget()", wrap(&x, ATOM)) } else { format!("&{}.clone().forget()", wrap(&x, ATOM)) };
            }
        }
        let converts = matches!((want, &x.ty), (Ty::Str, Ty::StrLit) | (Ty::Str, Ty::HostStr) | (Ty::List(_), Ty::Array(..)));
        if converts {
            return format!("&{}", self.owned(x, want));
        }
        match x.form {
            Form::Ref | Form::Mut => x.code,
            _ => format!("&{}", wrap(&x, 90)),
        }
    }

    fn mut_ref(&mut self, x: Ex) -> String {
        match x.form {
            Form::Mut => x.code,
            _ => format!("&mut {}", wrap(&x, 90)),
        }
    }

    /// Text as the host's `&str`, which the console and the runtime's text functions take.
    pub fn as_str(&mut self, x: Ex) -> String {
        match x.ty {
            Ty::StrLit => x.code,
            Ty::HostStr if x.code.starts_with("format!") => format!("&{}", x.code),
            _ => format!("{}.as_str()", wrap(&x, ATOM)),
        }
    }

    pub fn expr(&mut self, e: &Node) -> Ex {
        match e.kind {
            "literal" => self.literal(e),
            "name" => {
                let n = e.str("name").unwrap();
                if let Some(v) = self.var(n) {
                    return ex(v.alias.clone().unwrap_or_else(|| n.to_string()), v.ty.clone(), v.form, ATOM);
                }
                ex(n, Ty::Unknown, Form::Temp, ATOM)
            }
            "member" => self.member(e),
            "call" => self.call(e),
            "index" => {
                let o = self.expr(e.node("object").unwrap());
                let i = self.expr(e.node("index").unwrap());
                let form = if matches!(o.form, Form::Ref | Form::Mut | Form::Borrowed) { Form::Borrowed } else { Form::Place };
                ex(format!("{}[{}]", wrap(&o, ATOM), i.code), o.ty.elem(), form, ATOM)
            }
            "binary" => self.binary(e),
            "unary" => {
                let x = self.expr(e.node("operand").unwrap());
                let op = if e.str("op") == Some("not") { "!" } else { "-" };
                ex(format!("{op}{}", wrap(&x, 90)), x.ty.clone(), Form::Temp, 90)
            }
            "list" => self.expr_want(e, &Ty::List(Box::new(Ty::Unknown))),
            "or_fail" => {
                let x = self.expr(e.node("value").unwrap());
                let ok = match &x.ty {
                    Ty::Res(a, _) => (**a).clone(),
                    t => t.clone(),
                };
                match &self.ctx.section {
                    Some(s) => {
                        let s = s.clone();
                        ex(format!("{}.expect(\"{s}\")", wrap(&x, ATOM)), ok, Form::Temp, ATOM)
                    }
                    None => ex(format!("{}?", wrap(&x, ATOM)), ok, Form::Temp, ATOM),
                }
            }
            "forever" => self.expr(e.node("value").unwrap()),
            "mutable" => {
                let x = self.expr(e.node("value").unwrap());
                let code = self.mut_ref(x.clone());
                ex(code, x.ty, Form::Mut, 90)
            }
            "fake" => self.fake(e.node("value").unwrap()),
            // `compile f(…)`: the value the build step evaluated, written as data (Y § 3.17), or, before it has run,
            // the call
            "compile" => self.compiled(e),
            "old" => self.old_value(e),
            "submit" => self.submit(e),
            // `reveal(x) because "…"`: the value, through the `reveal` capability in scope (Y § 3.15)
            "reveal" => {
                let x = self.expr(e.node("value").unwrap());
                let inner = match &x.ty { Ty::Sensitive(t) => (**t).clone(), _ => Ty::Unknown };
                if !matches!(x.ty, Ty::Sensitive(_)) {
                    self.error("`reveal(…)` reveals a `sensitive` value");
                }
                let cap = match self.var("reveal").map(|v| v.form) {
                    Some(Form::Ref | Form::Mut) => "reveal".to_string(),
                    Some(_) => "&reveal".to_string(),
                    None => {
                        self.error("`reveal(…)` needs the `reveal` capability, an `injected` parameter (Y § 3.3, § 3.15)");
                        "reveal".to_string()
                    }
                };
                let arg = self.by_ref(x, &Ty::Unknown);
                let because = e.str("because").unwrap_or("");
                self.pending.push(format!(" // y: reveal because \"{because}\""));
                ex(format!("Reveal::reveal({cap}, {arg})"), inner, Form::Ref, ATOM)
            }
            k => ex(format!("/* {k} */"), Ty::Unknown, Form::Temp, ATOM),
        }
    }

    fn compiled(&mut self, e: &Node) -> Ex {
        let v = e.node("value").unwrap();
        let what = y_text(v);
        if v.kind != "call" || v.node("callee").is_none_or(|c| c.kind != "name") || v.node("callee").and_then(name_of).and_then(|f| self.function(f)).is_none() {
            self.error(format!("`compile {what}`: the operand of `compile` is one call of a function (Y § 3.17)"));
            return ex("()", Ty::Unknown, Form::Temp, ATOM);
        }
        let f = self.function(name_of(v.node("callee").unwrap()).unwrap()).unwrap();
        if !f.injected().is_empty() {
            self.error(format!("`compile {what}`: `{}` has `injected` parameters, and only a pure function is evaluated while building (Y § 3.17)", f.name));
        }
        for a in v.nodes("args") {
            let x = a.node("value").unwrap();
            if !crate::evaluate::known(x, &self.compile_bound) {
                self.error(format!("`compile {what}`: `{}` is not known while the program is built: an argument is a literal, a constant, or a name another `compile` bound (Y § 3.17)", y_text(x)));
            }
        }
        let x = self.expr(v);
        let (ok, fails) = match &x.ty {
            Ty::Res(a, _) => ((**a).clone(), true),
            t => (t.clone(), false),
        };
        let id = format!("{}:{}", self.file, e.line);
        if let Some(b) = self.binding.clone() {
            self.compile_bound.push(b);
        }
        let values = COMPILED.with(|c| c.borrow().clone());
        let Some(values) = values else {
            let site = crate::evaluate::Site { id: id.clone(), function: self.ctx.name.clone(), call: x.code.clone(), ty: ok.clone(), fails, binding: self.binding.clone() };
            SITES.with(|s| if !s.borrow().iter().any(|t| t.id == id) { s.borrow_mut().push(site) });
            let code = if fails { format!("{}.expect(\"y: compile {what}\")", wrap(&x, ATOM)) } else { x.code.clone() };
            return ex(code, ok, Form::Temp, ATOM);
        };
        let Some(value) = values.get(&id).cloned() else {
            self.error(format!("`compile {what}`: the build step gave it no value"));
            return ex("()", Ty::Unknown, Form::Temp, ATOM);
        };
        let Some(data) = crate::evaluate::data(self.module(), &ok, 0) else {
            self.error(format!("`compile {what}`: its value, a `{}`, is not one y-compile writes as data (numbers, truth values, strings, refined types over them, decimals, structures and closed types of these, and lists, sets and maps of these)", self.module().rust_type(&ok)));
            return ex("()", Ty::Unknown, Form::Temp, ATOM);
        };
        let base = self.binding.clone().unwrap_or_else(|| format!("value_{}", e.line));
        let n = if self.file.ends_with("main.y") { upper(&base) } else { format!("{}_{}", upper(&self.ctx.name), upper(&base)) };
        let rt = self.rust_type(&ok);
        self.statics.push(format!("/// The value of `compile {what}`, which the build evaluated (Y § 3.17): data."));
        let (code, form) = if data.plain {
            self.statics.push(format!("const {n}: {} = {value};", data.ty));
            (n.clone(), Form::Place)
        } else if self.file.ends_with("main.y") {
            self.statics.push(format!("const COMPILED_{n}: {} = {value};", data.ty));
            self.statics.push(String::new());
            self.statics.push(format!("/// The value of `compile {what}`, built once, on first use, from `COMPILED_{n}`."));
            self.statics.push(format!("static {n}: LazyLock<{rt}> = LazyLock::new(|| {});", data.build.replace("$D", &format!("COMPILED_{n}"))));
            (format!("&*{n}"), Form::Ref)
        } else {
            // in a library module, built from the data where it is read: a static built on first use is shared state,
            // which a module below `shared` does not hold (Y § 3.19)
            let f = n.to_lowercase();
            self.statics.push(format!("const COMPILED_{n}: {} = {value};", data.ty));
            self.statics.push(String::new());
            self.statics.push(format!("/// The value of `compile {what}`, built from `COMPILED_{n}`."));
            self.statics.push(format!("fn {f}() -> {rt} {{"));
            self.statics.push(format!("    {}", data.build.replace("$D", &format!("COMPILED_{n}"))));
            self.statics.push("}".into());
            (format!("{f}()"), Form::Temp)
        };
        self.statics.push(String::new());
        // the generated test: the call at run time gives the value the build wrote down (Y § 3.17)
        let value_code = if data.plain { n.clone() } else if self.file.ends_with("main.y") { format!("{n}.clone()") } else { format!("{}()", n.to_lowercase()) };
        let expected = if fails { format!("Ok({value_code})") } else { value_code };
        self.compile_tests.push(format!("#[test]\nfn compiled_{}_is_the_run_time_value() {{\n    assert!({} == {expected}, \"y: compile {what}\");\n}}", n.to_lowercase(), x.code));
        ex(code, ok, form, ATOM)
    }

    fn literal(&mut self, e: &Node) -> Ex {
        let text = e.str("text").unwrap_or("");
        match e.str("type").unwrap_or("") {
            "integer" => ex(text, Ty::Int, Form::Temp, ATOM),
            "boolean" => ex(text, Ty::Bool, Form::Temp, ATOM),
            "decimal" => {
                let scale = text.split('.').nth(1).map(|f| f.len() as u32).unwrap_or(0);
                ex(text, Ty::Decimal(38, scale), Form::Temp, ATOM)
            }
            "duration" => {
                let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
                let unit = &text[digits.len()..];
                let code = match unit {
                    "ms" => format!("Duration::from_millis({digits})"),
                    "s" => format!("Duration::from_secs({digits})"),
                    "min" => format!("Duration::from_secs({})", digits.parse::<u64>().unwrap_or(0) * 60),
                    "h" => format!("Duration::from_secs({})", digits.parse::<u64>().unwrap_or(0) * 3600),
                    _ => format!("Duration::from_millis({digits})"),
                };
                ex(code, Ty::Named("duration".into()), Form::Temp, ATOM)
            }
            "string" | "path" | "url" => {
                let v = e.str("value").unwrap_or("");
                if v.contains('{') {
                    self.interpolation(v)
                } else {
                    ex(format!("\"{v}\""), Ty::StrLit, Form::Temp, ATOM)
                }
            }
            // `12km`: a quantity of a unit the module declares; the unit is erased (Y § 3.3)
            "quantity" => {
                let sym = e.str("unit").unwrap_or("");
                let unit = self.m.unit_symbols.get(sym).or_else(|| self.lib.and_then(|l| l.unit_symbols.get(sym))).cloned();
                match unit {
                    Some(u) => ex(e.str("number").unwrap_or("0"), Ty::Named(u), Form::Temp, ATOM),
                    None => {
                        self.error(format!("`{text}`: no unit has the symbol `{sym}`"));
                        ex("0", Ty::Unknown, Form::Temp, ATOM)
                    }
                }
            }
            "character" => {
                self.use_("y_string::Rune");
                ex(format!("Rune::from_scalar('{}')", e.str("value").unwrap_or("")), Ty::Rune, Form::Temp, ATOM)
            }
            _ => ex(text, Ty::Unknown, Form::Temp, ATOM),
        }
    }

    /// `"{name}: {game.word}"`: a `format!`, with a name inline and any other expression as an argument.
    fn interpolation(&mut self, v: &str) -> Ex {
        let mut fmt = String::new();
        let mut args = vec![];
        let mut rest = v;
        while let Some(open) = rest.find('{') {
            fmt.push_str(&rest[..open]);
            let close = rest[open..].find('}').map(|c| c + open).unwrap_or(rest.len() - 1);
            let inner = rest[open + 1..close].trim();
            let node = parse_expression(inner);
            let mut x = self.expr(&node);
            if let Ty::Named(n) = &x.ty {
                self.shown.insert(n.clone());
            }
            if let Ty::List(_) = &x.ty {
                x = ex(format!("format!(\"[{{}}]\", {}.iter().map(ToString::to_string).collect::<Vec<String>>().join(\", \"))", wrap(&x, ATOM)), Ty::HostStr, Form::Temp, ATOM);
            }
            if x.code.chars().all(|c| c.is_alphanumeric() || c == '_') {
                fmt.push_str(&format!("{{{}}}", x.code));
            } else {
                fmt.push_str("{}");
                args.push(x.code);
            }
            rest = &rest[close + 1..];
        }
        fmt.push_str(rest);
        let code = if args.is_empty() { format!("format!(\"{fmt}\")") } else { format!("format!(\"{fmt}\", {})", args.join(", ")) };
        ex(code, Ty::HostStr, Form::Temp, ATOM)
    }

    fn member(&mut self, e: &Node) -> Ex {
        // a field of a case a condition narrowed to, bound by the arm's pattern: `loaded.failure.id`
        if let Some(v) = self.var(&y_text(e)) {
            return ex(v.alias.clone().unwrap_or_default(), v.ty.clone(), v.form, ATOM);
        }
        let name = e.str("name").unwrap();
        let obj = e.node("object").unwrap();
        if let Some(o) = name_of(obj).filter(|o| self.var(o).is_none()) {
            if o == "optional" && name == "none" {
                return ex("None", Ty::Opt(Box::new(Ty::Unknown)), Form::Temp, ATOM);
            }
            if let Some(TypeDef::Enum { .. } | TypeDef::Enumeration { .. }) = self.type_def(o) {
                return ex(format!("{}::{}", pascal(o), pascal(name)), Ty::Named(o.to_string()), Form::Temp, ATOM);
            }
            if o == "key" {
                return ex(format!("Key::{}", pascal(name)), Ty::Named("key".into()), Form::Temp, ATOM);
            }
            let group = self.m.consts.get(o).or_else(|| self.lib.and_then(|l| l.consts.get(o)));
            if let Some(g) = group {
                let (_, ty, value) = g.items.iter().find(|(n, _, _)| n == name).cloned().unwrap_or((String::new(), None, Node::new("literal", 0)));
                let ty = const_type(ty.as_ref(), &value);
                if crate::model::needs_fn(self.module(), &value) {
                    let t = g.items.iter().find(|(n, _, _)| n == name).and_then(|(_, t, _)| t.clone()).unwrap_or(Ty::Unknown);
                    return ex(format!("{o}::{name}()"), t, Form::Temp, ATOM);
                }
                // a constant of the group being lowered is named alone
                if self.in_group.as_deref() == Some(o) {
                    return ex(upper(name), ty, Form::Temp, ATOM);
                }
                return ex(format!("{o}::{}", upper(name)), ty, Form::Temp, ATOM);
            }
        }
        let o = self.expr(obj);
        if let Some(Var { arm: Some(ok), .. }) = name_of(obj).and_then(|n| self.var(n)) {
            if *ok && name == "value" || !*ok && name == "failure" {
                return o;
            }
        }
        // a field of a value in a state: shared, or the state's own, `pending.charge` read as `pending.state.charge`
        if let Ty::StateCase(st, c) = &o.ty {
            if let Some(TypeDef::State { fields, cases }) = self.type_def(st) {
                let form = match o.form { Form::Temp => Form::Temp, Form::Ref | Form::Mut | Form::Borrowed => Form::Borrowed, Form::Place => Form::Place };
                if let Some((_, t)) = fields.iter().find(|(f, _)| f == name) {
                    return ex(format!("{}.{name}", wrap(&o, ATOM)), t.clone(), form, ATOM);
                }
                if let Some((_, t)) = cases.iter().find(|(x, _)| x == c).and_then(|(_, fs)| fs.iter().find(|(f, _)| f == name)) {
                    return ex(format!("{}.state.{name}", wrap(&o, ATOM)), t.clone(), form, ATOM);
                }
                self.error(format!("`{}.{name}`: `{st}.{c}` has no field `{name}`", y_text(obj)));
            }
        }
        if let Ty::Named(st) = &o.ty {
            if let Some(TypeDef::State { fields, .. }) = self.type_def(st) {
                if let Some((_, t)) = fields.iter().find(|(f, _)| f == name) {
                    let form = match o.form { Form::Temp => Form::Temp, Form::Ref | Form::Mut | Form::Borrowed => Form::Borrowed, Form::Place => Form::Place };
                    return ex(format!("{}.{name}", wrap(&o, ATOM)), t.clone(), form, ATOM);
                }
            }
            if let Some(TypeDef::Command { fields, .. }) = self.type_def(st) {
                if let Some((_, t)) = fields.iter().find(|(f, _)| f == name) {
                    let form = match o.form { Form::Temp => Form::Temp, Form::Ref | Form::Mut | Form::Borrowed => Form::Borrowed, Form::Place => Form::Place };
                    return ex(format!("{}.{name}", wrap(&o, ATOM)), t.clone(), form, ATOM);
                }
            }
        }
        let field = self.module().field_ty(&o.ty, name);
        match field {
            Some((t, true)) => ex(format!("{}.{name}()", wrap(&o, ATOM)), t, Form::Temp, ATOM),
            Some((t, false)) => {
                let form = match o.form {
                    Form::Temp => Form::Temp,
                    Form::Ref | Form::Mut | Form::Borrowed => Form::Borrowed,
                    Form::Place => Form::Place,
                };
                ex(format!("{}.{name}", wrap(&o, ATOM)), t, form, ATOM)
            }
            None => ex(format!("{}.{name}", wrap(&o, ATOM)), Ty::Unknown, Form::Place, ATOM),
        }
    }

    fn args<'n>(&self, e: &'n Node) -> Vec<(Option<&'n str>, &'n Node)> {
        e.nodes("args").into_iter().map(|a| (a.str("name"), a.node("value").unwrap())).collect()
    }

    fn call(&mut self, e: &Node) -> Ex {
        let callee = e.node("callee").unwrap();
        let args = self.args(e);
        if let Some(n) = name_of(callee) {
            // a lambda or a nested function: its own arguments, and the closure passes what it captures
            if let Some((f, captures)) = self.locals_fns.get(n).cloned() {
                let mut parts = vec![];
                for (k, p) in f.params.iter().enumerate() {
                    let arg = args.iter().find(|(an, _)| *an == Some(p.name.as_str())).or_else(|| args.get(k).filter(|(an, _)| an.is_none()));
                    if let Some((_, a)) = arg {
                        parts.push(self.pass(a, p));
                    }
                }
                let ret = f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unit);
                let ty = match &f.fails { Some(e) => Ty::Res(Box::new(ret), Box::new(e.clone())), None => ret };
                let _ = captures;
                return ex(format!("{n}({})", parts.join(", ")), ty, Form::Temp, ATOM);
            }
            // a command's value, `charge(order: o, amount: a)`, where its arguments name its fields
            if let Some(TypeDef::Command { fields, .. }) = self.type_def(n) {
                let fields = fields.clone();
                let named = !args.is_empty() && args.iter().all(|(an, _)| an.is_some_and(|an| fields.iter().any(|(f, _)| f == an)));
                if named {
                    return self.construct(&pascal(n), &fields, &args, Ty::Named(n.to_string()));
                }
            }
            if self.var(n).is_none() {
                if let Some(f) = self.function(n) {
                    return self.call_fn(f, &args);
                }
                if let Some(TypeDef::Struct { fields, .. }) = self.type_def(n) {
                    let fields = fields.clone();
                    return self.construct(&pascal(n), &fields, &args, Ty::Named(n.to_string()));
                }
            }
            if n == "point" {
                let parts: Vec<String> = args.iter().map(|(k, a)| format!("{}: {}", k.unwrap_or("x"), self.expr(a).code)).collect();
                return ex(format!("Point {{ {} }}", parts.join(", ")), Ty::Named("point".into()), Form::Temp, ATOM);
            }
            if n == "frame" {
                if args.is_empty() {
                    return ex("Frame::default()", Ty::Named("frame".into()), Form::Temp, ATOM);
                }
                let mut keys = "Vec::new()".to_string();
                let mut clicks = "Vec::new()".to_string();
                for (k, a) in &args {
                    let items: Vec<String> = a.nodes("items").iter().map(|i| self.expr(i).code).collect();
                    let v = format!("vec![{}]", items.join(", "));
                    if *k == Some("keys") { keys = v } else { clicks = v }
                }
                return ex(format!("Frame {{ keys: {keys}, clicks: {clicks} }}"), Ty::Named("frame".into()), Form::Temp, ATOM);
            }
            if n == "colour" {
                let parts: Vec<String> = args.iter().map(|(_, a)| self.expr(a).code).collect();
                return ex(format!("Colour::rgb({})", parts.join(", ")), Ty::Named("colour".into()), Form::Temp, ATOM);
            }
            let parts: Vec<String> = args.iter().map(|(_, a)| self.expr(a).code).collect();
            return ex(format!("{n}({})", parts.join(", ")), Ty::Unknown, Form::Temp, ATOM);
        }
        let method = callee.str("name").unwrap();
        let obj = callee.node("object").unwrap();
        // `list.count().to_uint32()`: the count of a list as a `uint32`, from the runtime's list operations
        if (method == "to_uint32" || method == "to_int32") && obj.kind == "call" {
            let inner = obj.node("callee").unwrap();
            if inner.kind == "member" && inner.str("name") == Some("count") {
                let l = self.expr(inner.node("object").unwrap());
                if matches!(l.ty, Ty::List(_)) {
                    self.use_("y_runtime::list");
                    let r = self.by_ref(l.clone(), &l.ty.clone());
                    return if method == "to_uint32" {
                        ex(format!("list::count({r})"), Ty::U32, Form::Temp, ATOM)
                    } else {
                        ex(format!("list::count_int32({r})"), Ty::I32, Form::Temp, ATOM)
                    };
                }
            }
        }
        if let Some(o) = name_of(obj).filter(|o| self.var(o).is_none()) {
            match (o, method) {
                ("list", "repeated") => {
                    self.use_("y_runtime::list");
                    let v = self.expr(args[0].1);
                    let n = self.expr_want(args[1].1, &Ty::U32);
                    let n = self.owned(n, &Ty::U32);
                    let ty = v.ty.clone();
                    return ex(format!("List::from(list::repeated({}, {n}))", v.code), Ty::List(Box::new(ty)), Form::Temp, ATOM);
                }
                ("bag", "new") | ("queue", "new") | ("sorted_map", "new") => {
                    let t = pascal(o);
                    let ty = match o { "bag" => Ty::Bag(Box::new(Ty::Unknown)), "queue" => Ty::Queue(Box::new(Ty::Unknown)), _ => Ty::SortedMap(Box::new(Ty::Unknown), Box::new(Ty::Unknown)) };
                    return ex(format!("{t}::new()"), ty, Form::Temp, ATOM);
                }
                (k, "new") if crate::ty::coll_kind(k).is_some_and(|c| c.1 == k) && args.is_empty() => {
                    let (r, ..) = crate::ty::coll_kind(k).unwrap();
                    return ex(format!("y_collections::{r}::new()"), Ty::Coll(r, vec![]), Form::Temp, ATOM);
                }
                ("shared", "new") => {
                    let x = self.expr(args[0].1);
                    let t = x.ty.clone();
                    let v = self.owned(x, &t);
                    return ex(format!("Shared::new({v})"), Ty::Shared(Box::new(t)), Form::Temp, ATOM);
                }
                ("system", "store") => {
                    let p = self.arg_str(&args, 0);
                    return ex(format!("system.store({p})"), Ty::Store(Box::new(Ty::Unknown)), Form::Temp, ATOM);
                }
                ("set", "new") => {
                    self.use_("y_collections::Set");
                    return ex("Set::new()", Ty::Set(Box::new(Ty::Unknown)), Form::Temp, ATOM);
                }
                ("set", "of") => {
                    self.use_("y_collections::Set");
                    let items: Vec<String> = args[0].1.nodes("items").iter().map(|i| { let x = self.expr(i); let t = x.ty.clone(); self.owned(x, &t) }).collect();
                    let elem = args[0].1.nodes("items").first().map(|i| self.expr(i).ty).unwrap_or(Ty::Unknown);
                    return ex(format!("Set::from_iter([{}])", items.join(", ")), Ty::Set(Box::new(elem)), Form::Temp, ATOM);
                }
                // `sensitive.new(x)`: a value that is not shown (Y § 3.3)
                ("sensitive", "new") => {
                    let x = self.expr(args[0].1);
                    let t = match x.ty { Ty::StrLit | Ty::HostStr => Ty::Str, ref t => t.clone() };
                    let v = self.owned(x, &t);
                    self.use_("y_runtime::claims::Sensitive");
                    return ex(format!("Sensitive::new({v})"), Ty::Sensitive(Box::new(t)), Form::Temp, ATOM);
                }
                // `submit.to(charge)`: where commands are delivered to the handler `charge` (Y § 3.10)
                ("submit", "to") => {
                    let h = args.first().and_then(|(_, a)| name_of(a)).unwrap_or("").to_string();
                    return self.submit_to(&h);
                }
                // `enqueue.new()`: an outbox, held in shared state, for commands an atomic region enqueues
                ("enqueue", "new") => {
                    self.use_("y_runtime::command::Outbox");
                    return ex("Shared::new(Outbox::new())", Ty::Shared(Box::new(Ty::Outbox(Box::new(Ty::Unknown)))), Form::Temp, ATOM);
                }
                // `rational.new(numerator: 1, denominator: 3)`: an exact fraction, in lowest terms
                ("rational", "new") => {
                    let mut parts = vec![];
                    for k in ["numerator", "denominator"] {
                        let a = args.iter().find(|(n, _)| *n == Some(k)).or_else(|| args.get(if k == "numerator" { 0 } else { 1 })).map(|(_, a)| *a);
                        let Some(a) = a else {
                            self.error("`rational.new` takes a `numerator` and a `denominator`");
                            continue;
                        };
                        let x = self.expr(a);
                        parts.push(match x.ty {
                            Ty::Integer(..) => format!("{}.to_i128()", wrap(&x, ATOM)),
                            _ => x.code,
                        });
                    }
                    return ex(format!("Rational::<Big, UNBOUNDED>::new({})", parts.join(", ")), Ty::Rational(crate::ty::UNBOUNDED), Form::Temp, ATOM);
                }
                ("list", "new") => {
                    self.use_("y_collections::List");
                    return ex("List::new()", Ty::List(Box::new(Ty::Unknown)), Form::Temp, ATOM);
                }
                ("result", "err") => {
                    let x = self.expr(args[0].1);
                    let ty = x.ty.clone();
                    return ex(format!("Err({})", x.code), Ty::Res(Box::new(Ty::Unknown), Box::new(ty)), Form::Temp, ATOM);
                }
                ("result", "ok") => {
                    let x = self.expr(args[0].1);
                    let ty = x.ty.clone();
                    return ex(format!("Ok({})", x.code), Ty::Res(Box::new(ty), Box::new(Ty::Unknown)), Form::Temp, ATOM);
                }
                ("optional", "some") => {
                    let x = self.expr(args[0].1);
                    let ty = x.ty.clone();
                    let v = self.owned(x, &ty);
                    return ex(format!("Some({v})"), Ty::Opt(Box::new(ty)), Form::Temp, ATOM);
                }
                _ => {}
            }
            if let Some(TypeDef::Newtype { base, .. }) = self.type_def(o) {
                if method == "new" {
                    let base = base.clone();
                    let x = self.expr_want(args[0].1, &base);
                    let v = self.owned(x, &base);
                    self.use_("y_runtime::ConstraintViolation");
                    return ex(format!("{}::new({v})", pascal(o)), Ty::Res(Box::new(Ty::Named(o.to_string())), Box::new(Ty::Named("constraint_violation".into()))), Form::Temp, ATOM);
                }
            }
            // `order.created(id: 1, total: 5)`: a value in a state, with every field; `order.paid(receipt: r)`, the
            // state's own fields alone, which `becomes` takes
            if let Some(TypeDef::State { fields, cases }) = self.type_def(o) {
                if let Some((_, own)) = cases.iter().find(|(c, _)| c == method) {
                    let (fields, own) = (fields.clone(), own.clone());
                    let state = if own.is_empty() { format!("{o}::{}", pascal(method)) } else { self.construct(&format!("{o}::{}", pascal(method)), &own, &args, Ty::Unknown).code };
                    if args.iter().any(|(an, _)| an.is_some_and(|an| fields.iter().any(|(f, _)| f == an))) {
                        let mut s = self.construct(&pascal(o), &fields, &args, Ty::Unknown).code;
                        s.truncate(s.len() - 2);
                        let sep = if fields.is_empty() { "" } else { ", " };
                        return ex(format!("{s}{sep}state: {state} }}"), Ty::StateCase(o.to_string(), method.to_string()), Form::Temp, ATOM);
                    }
                    return ex(state, Ty::Named(format!("{o}.{method}:state")), Form::Temp, ATOM);
                }
            }
            if let Some(TypeDef::Enum { cases }) = self.type_def(o) {
                if let Some((_, fields)) = cases.iter().find(|(c, _)| c == method) {
                    let fields = fields.clone();
                    return self.construct(&format!("{}::{}", pascal(o), pascal(method)), &fields, &args, Ty::Named(o.to_string()));
                }
            }
            if let Some((r, krate)) = external(o) {
                self.use_(&format!("{krate}::{r}"));
                let mut parts = vec![];
                for (_, a) in &args {
                    let x = self.expr(a);
                    parts.push(if x.ty.is_text() { self.as_str(x) } else { x.code });
                }
                let ty = if (o, method) == ("picture", "from_svg") {
                    Ty::Res(Box::new(Ty::Named("picture".into())), Box::new(Ty::Named("picture_error".into())))
                } else {
                    Ty::Named(o.to_string())
                };
                return ex(format!("{r}::{method}({})", parts.join(", ")), ty, Form::Temp, ATOM);
            }
        }
        if let Some(Var { arm: Some(false), .. }) = name_of(obj).and_then(|n| self.var(n)) {
            if method == "failure" {
                return self.expr(obj);
            }
        }
        let recv = self.expr(obj);
        self.method(recv, method, &args)
    }

    /// The pattern for a case with fields that a condition compares a label with, `shape.circle` or
    /// `result.err(customer_error.not_found)`: the case with the fields its block reads bound, `CustomerError::NotFound
    /// { id: loaded_id, .. }`, each field then read by its Y path (`loaded.failure.id`, `s.radius`). `None` for a case
    /// without fields.
    fn case_pattern(&mut self, case: &Node, label: &str, body: &[&Node], form: Form) -> Option<String> {
        let (wrapper, via, inner) = match case.kind {
            "call" if case.node("callee").is_some_and(|c| is_member(c, "result", "ok")) => ("Ok", "value", case.nodes("args").first()?.node("value")?),
            "call" if case.node("callee").is_some_and(|c| is_member(c, "result", "err")) => ("Err", "failure", case.nodes("args").first()?.node("value")?),
            "member" => ("", "", case),
            _ => return None,
        };
        let t = name_of(inner.node("object")?)?;
        let c = inner.str("name")?;
        // `undelivered.failed` and `undelivered.abandoned`: the runtime's cases, with their payloads
        if t == "undelivered" {
            let field = if c == "failed" { "error" } else { "abandonment" };
            let path = format!("{label}.{via}.{field}");
            let used = body.iter().any(|b| { let mut u = false; b.walk(&mut |n: &Node| if n.kind == "member" && y_text(n) == path { u = true; }); u });
            let bind = if used {
                self.scopes.last_mut().unwrap().insert(path, Var { ty: Ty::Unknown, form, arm: None, alias: Some(format!("{label}_{field}")) });
                format!("{field}: {label}_{field}")
            } else {
                "..".to_string()
            };
            self.use_("y_runtime::command::Undelivered");
            return Some(format!("{wrapper}(Undelivered::{} {{ {bind} }})", pascal(c)));
        }
        let Some(TypeDef::Enum { cases }) = self.type_def(t) else { return None };
        let fields = cases.iter().find(|(n, _)| n == c)?.1.clone();
        if fields.is_empty() {
            return None;
        }
        let prefix = if via.is_empty() { label.to_string() } else { format!("{label}.{via}") };
        let mut parts = vec![];
        for (f, ft) in &fields {
            let path = format!("{prefix}.{f}");
            let used = body.iter().any(|b| { let mut u = false; b.walk(&mut |n: &Node| if n.kind == "member" && y_text(n) == path { u = true; }); u });
            if used {
                let local = format!("{label}_{f}");
                parts.push(format!("{f}: {local}"));
                let fform = form;
                self.scopes.last_mut().unwrap().insert(path, Var { ty: ft.clone(), form: fform, arm: None, alias: Some(local) });
            }
        }
        parts.push("..".into());
        let p = format!("{}::{} {{ {} }}", pascal(t), pascal(c), parts.join(", "));
        Some(if wrapper.is_empty() { p } else { format!("{wrapper}({p})") })
    }

    /// A structure's value, as Rust.
    pub fn construct_value(&mut self, path: &str, fields: &[(String, Ty)], args: &[(Option<&str>, &Node)]) -> String {
        self.construct(path, fields, args, Ty::Unknown).code
    }

    fn construct(&mut self, path: &str, fields: &[(String, Ty)], args: &[(Option<&str>, &Node)], ty: Ty) -> Ex {
        let mut parts = vec![];
        for (k, (f, fty)) in fields.iter().enumerate() {
            let arg = args.iter().find(|(n, _)| *n == Some(f.as_str())).or_else(|| args.get(k).filter(|(n, _)| n.is_none()));
            let Some((_, a)) = arg else { continue };
            let x = self.expr_want(a, fty);
            let v = self.owned(x, fty);
            parts.push(if &v == f { v } else { format!("{f}: {v}") });
        }
        ex(format!("{path} {{ {} }}", parts.join(", ")), ty, Form::Temp, ATOM)
    }

    pub fn pass(&mut self, a: &Node, p: &Param) -> String {
        if a.kind == "mutable" {
            let x = self.expr(a.node("value").unwrap());
            return self.mut_ref(x);
        }
        // a value in a state given to what takes it: moved, and a name that is read afterwards is an error (Y § 3.11)
        if let Ty::StateCase(st, c) = &p.ty {
            let x = self.expr_want(a, &p.ty);
            if let Some(root) = root_name(a) {
                if self.still_read(&root) {
                    self.error(format!("`{root}` is given, in the state `{st}.{c}`, to `{}`, which takes it: it cannot be used afterwards (Y § 3.11)", self.calling.clone().unwrap_or_default()));
                }
            }
            return x.code;
        }
        let x = self.expr_want(a, &p.ty);
        match p.mode {
            Mode::Ref => self.by_ref(x, &p.ty),
            Mode::Mut => self.mut_ref(x),
            Mode::Value if matches!(x.form, Form::Place | Form::Ref | Form::Borrowed) && !self.is_copy(&x.ty) && x.ty != Ty::Unknown => {
                // a value the callee keeps, from a place that is still read: a copy (Y § 3.2)
                let root = root_name(a);
                let borrowed = x.form == Form::Borrowed || root.as_deref().and_then(|r| self.var(r)).is_some_and(|v| matches!(v.form, Form::Ref | Form::Mut));
                if borrowed || root.as_deref().is_some_and(|r| self.still_read(r)) || !name_like(&x.code) {
                    let what = y_text(a);
                    let why = format!("{} keeps its {}, and {} is still read", self.calling.clone().unwrap_or_default(), p.name, root.unwrap_or_else(|| what.clone()));
                    self.pending.push(format!(" // y: a copy, because {why}"));
                    self.copies.push((a.line, what, why));
                    format!("{}.clone()", wrap(&x, ATOM))
                } else {
                    self.owned(x, &p.ty)
                }
            }
            Mode::Value => self.owned(x, &p.ty),
        }
    }

    fn call_fn(&mut self, f: &FnInfo, args: &[(Option<&str>, &Node)]) -> Ex {
        let saved = self.calling.replace(f.name.clone());
        let mut parts = vec![];
        let plain: Vec<&Param> = f.params.iter().filter(|p| !p.injected).collect();
        for (k, p) in plain.iter().enumerate() {
            let arg = args.iter().find(|(n, _)| *n == Some(p.name.as_str())).or_else(|| args.get(k).filter(|(n, _)| n.is_none()));
            if let Some((_, a)) = arg {
                parts.push(self.pass(a, p));
            }
        }
        for p in f.injected() {
            let v = self.var(&p.name).cloned();
            let code = match (v.map(|v| v.form), p.mode) {
                (Some(Form::Ref | Form::Mut), _) => p.name.clone(),
                (_, Mode::Mut) => format!("&mut {}", p.name),
                _ => format!("&{}", p.name),
            };
            parts.push(code);
        }
        self.calling = saved;
        let ret = f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unit);
        let ty = match &f.fails {
            Some(e) => Ty::Res(Box::new(ret), Box::new(e.clone())),
            None => ret,
        };
        let call = format!("{}({})", f.name, parts.join(", "));
        if self.module().suspending.contains(&f.name) {
            // a function that may suspend is awaited (Y § 3.7); a test runs it to its end
            if self.block_on {
                return ex(format!("block_on({call})"), ty, Form::Temp, ATOM);
            }
            return ex(format!("{call}.await"), ty, Form::Temp, ATOM);
        }
        ex(call, ty, Form::Temp, ATOM)
    }

    fn arg_str(&mut self, args: &[(Option<&str>, &Node)], k: usize) -> String {
        let x = self.expr(args[k].1);
        self.as_str(x)
    }

    fn method(&mut self, recv: Ex, name: &str, args: &[(Option<&str>, &Node)]) -> Ex {
        if let Some(x) = self.ware_method(&recv, name, args) {
            return x;
        }
        if let Some(x) = self.kind_method(&recv, name, args) {
            return x;
        }
        if let Ty::Named(n) = &recv.ty {
            if let Some(TypeDef::Newtype { base, .. }) = self.type_def(n) {
                let inner = ex(format!("{}.as_text()", wrap(&recv, ATOM)), base.clone(), Form::Ref, ATOM);
                return self.method(inner, name, args);
            }
        }
        let r = wrap(&recv, ATOM);
        let console_error = || Ty::Named("console_error".into());
        self.note_type(&recv.ty);
        match (&recv.ty, name) {
            (Ty::Cap(c), "write_line" | "write") if c == "console" => {
                let s = self.arg_str(args, 0);
                ex(format!("{r}.{name}({s})"), Ty::Res(Box::new(Ty::Unit), Box::new(console_error())), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "read_line") if c == "console" => {
                let fv = match self.var("forever").map(|v| v.form) {
                    Some(Form::Ref | Form::Mut) => "forever".to_string(),
                    _ => "&forever".to_string(),
                };
                ex(format!("{r}.read_line({fv})"), Ty::Res(Box::new(Ty::Opt(Box::new(Ty::HostStr))), Box::new(console_error())), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "written") if c == "console" => ex(format!("{r}.written()"), Ty::List(Box::new(Ty::HostStr)), Form::Temp, ATOM),
            (Ty::Cap(c), "next_frame") if c == "window" => {
                let fv = match self.var("forever").map(|v| v.form) {
                    Some(Form::Ref | Form::Mut) => "forever".to_string(),
                    _ => "&forever".to_string(),
                };
                ex(format!("{r}.next_frame({fv}).await"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::Cap(c), "keys_pressed") if c == "window" => {
                self.use_("y_collections::List");
                ex(format!("List::from({r}.keys_pressed())"), Ty::List(Box::new(Ty::Named("key".into()))), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "clicks") if c == "window" => {
                self.use_("y_collections::List");
                ex(format!("List::from({r}.clicks())"), Ty::List(Box::new(Ty::Named("point".into()))), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "close_requested") if c == "window" => ex(format!("{r}.close_requested()"), Ty::Bool, Form::Temp, ATOM),
            (Ty::Cap(c), "written") if c == "window" => ex(format!("{r}.written()"), Ty::List(Box::new(Ty::HostStr)), Form::Temp, ATOM),
            (Ty::Cap(c), m) if c == "window" => {
                // drawing: each argument in the order the Y names them, text as the host's
                let mut parts = vec![];
                for (n, a) in args {
                    let x = self.expr(a);
                    let code = if x.ty.is_text() || *n == Some("text") {
                        self.as_str(x)
                    } else if !self.is_copy(&x.ty) && x.ty != Ty::Unknown {
                        self.by_ref(x.clone(), &x.ty.clone())
                    } else {
                        let t = x.ty.clone();
                        self.owned(x, &t)
                    };
                    parts.push(code);
                }
                ex(format!("{r}.{m}({})", parts.join(", ")), Ty::Unknown, Form::Temp, ATOM)
            }
            (Ty::Cap(c), "get") if c == "http" => {
                let p = self.arg_str(args, 0);
                ex(format!("{r}.get({p}, &deadline).await"), Ty::Res(Box::new(Ty::HostStr), Box::new(Ty::Named("within_http".into()))), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "chosen") if c == "random" => {
                self.use_("y_collections::List");
                let from = self.expr(args[0].1);
                let elem = match from.ty.elem() { Ty::StrLit => Ty::Str, t => t };
                let l = match &from.ty {
                    Ty::Array(e, _) if **e == Ty::StrLit => format!("&{}.map(YString::new)", from.code),
                    _ => self.by_ref(from.clone(), &from.ty.clone()),
                };
                let n = self.expr(args[1].1).code;
                ex(format!("List::from({r}.chosen({l}, {n}))"), Ty::List(Box::new(elem)), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "one_of") if c == "random" => {
                let from = self.expr(args[0].1);
                let t = from.ty.clone();
                let l = self.by_ref(from, &t);
                ex(format!("{r}.one_of({l})"), t.elem(), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "shuffled") if c == "random" => {
                self.use_("y_collections::List");
                let from = self.expr(args[0].1);
                let t = from.ty.clone();
                let l = self.by_ref(from, &t);
                ex(format!("List::from({r}.shuffled({l}))"), t, Form::Temp, ATOM)
            }
            (Ty::Named(t), "whole_seconds") if t == "duration" => {
                self.use_("y_runtime::time");
                let v = self.owned(recv.clone(), &recv.ty.clone());
                ex(format!("time::whole_seconds({v})"), Ty::U32, Form::Temp, ATOM)
            }
            (Ty::Cap(c), "now") if c == "clock" => ex(format!("{r}.now()"), Ty::Named("instant".into()), Form::Temp, ATOM),
            (Ty::Cap(c), "store") if c == "system" => {
                let p = self.arg_str(args, 0);
                ex(format!("{r}.store({p})"), Ty::Store(Box::new(Ty::Unknown)), Form::Temp, ATOM)
            }
            (Ty::Cap(c), "http") if c == "system" => {
                let svc = name_of(args[0].1).unwrap_or("service").to_string();
                ex(format!("{r}.http({svc}::ADDRESS)"), Ty::Cap("http".into()), Form::Temp, ATOM)
            }
            (Ty::Cap(c), cap) if c == "system" => {
                self.note_type(&Ty::Cap(cap.to_string()));
                ex(format!("{r}.{cap}()"), Ty::Cap(cap.to_string()), Form::Temp, ATOM)
            }
            (Ty::List(e), "count") if **e == Ty::HostStr => ex(format!("{r}.len()"), Ty::Count, Form::Temp, ATOM),
            (Ty::List(e), "last") if **e == Ty::HostStr => ex(format!("{r}.last().map(String::as_str)"), Ty::Opt(Box::new(Ty::StrLit)), Form::Temp, ATOM),
            (Ty::List(e), "contains") if **e == Ty::HostStr => {
                let s = self.arg_str(args, 0);
                ex(format!("{r}.iter().any(|line| line == {s})"), Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::List(_), "count") => ex(format!("{r}.count()"), Ty::Count, Form::Temp, ATOM),
            (Ty::List(e), "without_last") => {
                self.use_("y_runtime::list");
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                ex(format!("list::without_last({l})"), Ty::List(e.clone()), Form::Temp, ATOM)
            }
            (Ty::List(e), "insert_first") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.owned(v, &e);
                ex(format!("{r}.push_front({v})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::List(e), "remove_last") => ex(format!("{r}.take_back()"), Ty::Opt(e.clone()), Form::Temp, ATOM),
            (Ty::List(e), "remove_first") => ex(format!("{r}.take_front()"), Ty::Opt(e.clone()), Form::Temp, ATOM),
            (Ty::List(e), "first") => ex(format!("{r}.first().copied()"), Ty::Opt(e.clone()), Form::Temp, ATOM),
            (Ty::List(e), "at_number") if !self.is_copy(e) => {
                // a value that is not copied is borrowed from the list
                self.use_("y_runtime::list");
                let e = (**e).clone();
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                let n = self.expr_want(args[0].1, &Ty::U32).code;
                ex(format!("list::at_number_ref({l}, {n})"), e, Form::Ref, ATOM)
            }
            // a sorted copy
            (Ty::List(_), "sorted") => {
                self.use_("y_runtime::list");
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                ex(format!("List::from(list::sorted({l}))"), recv.ty.clone(), Form::Temp, ATOM)
            }
            (Ty::List(_), "extend") => {
                let v = self.expr(args[0].1);
                ex(format!("{r}.extend({})", v.code), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::List(e), "at_number") => {
                self.use_("y_runtime::list");
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                let n = self.expr_want(args[0].1, &Ty::U32).code;
                ex(format!("list::at_number({l}, {n})"), (**e).clone(), Form::Temp, ATOM)
            }
            (Ty::List(e), "set_number") => {
                self.use_("y_runtime::list");
                let e = (**e).clone();
                let l = self.mut_ref(recv.clone());
                let n = self.expr_want(args[0].1, &Ty::U32).code;
                let v = self.expr_want(args[1].1, &e);
                let v = self.owned(v, &e);
                ex(format!("list::set_number({l}, {n}, {v})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::List(e), "number_of") => {
                self.use_("y_runtime::list");
                let e = (**e).clone();
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                let v = self.expr_want(args[0].1, &e);
                let v = self.by_ref(v, &e);
                ex(format!("list::number_of({l}, {v})"), Ty::U32, Form::Temp, ATOM)
            }
            (Ty::List(e), "largest" | "smallest" | "last") => {
                self.use_("y_runtime::list");
                let e = (**e).clone();
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                ex(format!("list::{name}({l})"), e, Form::Temp, ATOM)
            }
            (Ty::List(e), "push") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let elem = match (&e, &v.ty) {
                    (Ty::Unknown, Ty::HostStr | Ty::StrLit) => Ty::Str,
                    (Ty::Unknown, t) => t.clone(),
                    _ => e,
                };
                let v = self.owned(v, &elem);
                // `mutable found = list.new()`: the list's type is known from what goes into it
                if let Some(var) = self.scopes.iter_mut().rev().find_map(|s| s.get_mut(&recv.code)) {
                    if var.ty.elem() == Ty::Unknown {
                        var.ty = Ty::List(Box::new(elem));
                    }
                }
                ex(format!("{r}.push({v})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::List(e), "join") if **e != Ty::Rune => {
                self.use_("y_string::YString");
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                let s = self.arg_str(args, 0);
                ex(format!("YString::joined({l}, {s})"), Ty::Str, Form::Temp, ATOM)
            }
            (Ty::List(_), "contains") => {
                let e = recv.ty.elem();
                let v = self.expr_want(args[0].1, &e);
                let v = self.by_ref(v, &e);
                ex(format!("{r}.contains({v})"), Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::Str, "split_whitespace") => {
                self.use_("y_collections::List");
                ex(format!("List::from({r}.split_whitespace())"), Ty::List(Box::new(Ty::Str)), Form::Temp, ATOM)
            }
            (Ty::Str, "to_lowercase") => ex(format!("YString::from({r}.as_str().to_lowercase())"), Ty::Str, Form::Temp, ATOM),
            (Ty::Str, "single_rune") => {
                self.use_("y_string::Rune");
                let a = self.by_ref(recv.clone(), &Ty::Str);
                ex(format!("Rune::single({a})"), Ty::Opt(Box::new(Ty::Rune)), Form::Temp, ATOM)
            }
            (Ty::Str, "split") => {
                let sep = self.arg_str(args, 0);
                ex(format!("{r}.as_str().split({sep}).map(YString::new)"), Ty::Iter(Box::new(Ty::Str)), Form::Temp, ATOM)
            }
            (Ty::Str, "runes") => ex(format!("{r}.runes()"), Ty::Iter(Box::new(Ty::Rune)), Form::Temp, ATOM),
            (Ty::Str, "is_empty") => ex(format!("{r}.is_empty()"), Ty::Bool, Form::Temp, ATOM),
            (Ty::Str | Ty::StrLit | Ty::HostStr, "matches") if args.len() == 1 && args[0].1.str("type") == Some("pattern") => {
                let pat = args[0].1.str("value").unwrap_or("").to_string();
                let code = self.pattern_match(&pat, recv.clone());
                ex(code, Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::Str, "to_string") => {
                let t = recv.ty.clone();
                let v = self.owned(recv, &t);
                ex(v, Ty::Str, Form::Temp, ATOM)
            }
            // `word.runes().contains(letter)`: the string's own test
            (Ty::Iter(e), "contains") if r.ends_with(".runes()") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.by_ref(v, &e);
                ex(format!("{}.contains({v})", r.trim_end_matches(".runes()")), Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::Iter(e), "to_set") => ex(r, Ty::Set(e.clone()), Form::Temp, ATOM),
            // `word.runes().to_set().is_subset_of(hits)`: every rune is in the other set
            (Ty::Set(_), "is_subset_of") if r.ends_with(".runes()") => {
                let o = self.expr(args[0].1);
                let o = wrap(&o, ATOM);
                ex(format!("{r}.all(|letter| {o}.contains(&letter))"), Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::Set(e), "contains") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.by_ref(v, &e);
                ex(format!("{r}.contains({v})"), Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::Set(e), "insert" | "add") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.owned(v, &e);
                ex(format!("{r}.add({v})"), Ty::Bool, Form::Temp, ATOM)
            }
            (Ty::Set(_) | Ty::List(_), "is_empty") => ex(format!("{r}.is_empty()"), Ty::Bool, Form::Temp, ATOM),
            (Ty::Set(_), "count") => ex(format!("{r}.count()"), Ty::Count, Form::Temp, ATOM),
            (Ty::List(e), "join") if **e == Ty::Rune => {
                self.use_("y_string::YString");
                let l = self.by_ref(recv.clone(), &recv.ty.clone());
                let s = self.arg_str(args, 0);
                ex(format!("YString::from_runes({l}, {s})"), Ty::Str, Form::Temp, ATOM)
            }
            (Ty::Cap(c), "index_below") if c == "random" => {
                let n = self.expr(args[0].1).code;
                ex(format!("{r}.index_below({n})"), Ty::Count, Form::Temp, ATOM)
            }
            (Ty::Str, "trim") => ex(format!("{r}.trimmed()"), Ty::Str, Form::Temp, ATOM),
            (Ty::Str, "parse_uint32") => ex(format!("{r}.parse_uint32()"), Ty::Opt(Box::new(Ty::U32)), Form::Temp, ATOM),
            (Ty::Str, "count") => ex(format!("{r}.count()"), Ty::Count, Form::Temp, ATOM),
            (Ty::StrLit, "repeated") => {
                self.use_("y_runtime::list");
                let n = self.expr_want(args[0].1, &Ty::U32);
                let n = self.owned(n, &Ty::U32);
                ex(format!("list::repeated({r}, {n}).concat()"), Ty::HostStr, Form::Temp, ATOM)
            }
            (t, "xor") if t.is_int() => {
                let o = self.expr_want(args[0].1, t);
                let o = self.owned(o, &t.clone());
                ex(format!("{} ^ {o}", wrap(&recv, 61)), recv.ty.clone(), Form::Temp, 60)
            }
            (Ty::U32, "to_uint32") => recv,
            (Ty::I32, "to_uint32") => {
                self.use_("y_runtime::number");
                ex(format!("number::to_uint32({})", recv.code), Ty::U32, Form::Temp, ATOM)
            }
            (Ty::U32, "to_int32") => {
                self.use_("y_runtime::number");
                ex(format!("number::to_int32({})", recv.code), Ty::I32, Form::Temp, ATOM)
            }
            (t, "divided_rounding_up") if t.is_int() => {
                let n = self.expr_want(args[0].1, t).code;
                ex(format!("{r}.div_ceil({n})"), recv.ty.clone(), Form::Temp, ATOM)
            }
            (t, "max" | "min") if t.is_int() => {
                let o = self.expr_want(args[0].1, t);
                let t = t.clone();
                let o = self.owned(o, &t);
                ex(format!("{r}.{name}({o})"), t, Form::Temp, ATOM)
            }
            (Ty::Res(_, e), "failure") => ex(format!("{r}.expect_err(\"a failure\")"), (**e).clone(), Form::Temp, ATOM),
            _ => self.library_method(recv, name, args),
        }
    }

    /// A method that no rule of the lowering has: the method of that name on the receiver's Rust type, as the runtime
    /// crate gives it (`crate::library`), or a compile error that names it.
    fn library_method(&mut self, recv: Ex, name: &str, args: &[(Option<&str>, &Node)]) -> Ex {
        let r = wrap(&recv, ATOM);
        let ty = self.module().resolve(&recv.ty);
        let Some((owner, y_name)) = crate::library::owner_of(&ty) else {
            if let Ty::Named(n) = &ty {
                if self.type_def(n).is_some() {
                    self.error(format!("`{}.{name}(…)`: the type `{n}` has no method `{name}`", y_text_of(&recv.code)));
                }
            }
            let parts: Vec<String> = args.iter().map(|(_, a)| self.expr(a).code).collect();
            return ex(format!("{r}.{name}({})", parts.join(", ")), Ty::Unknown, Form::Temp, ATOM);
        };
        let lib = crate::library::library();
        let Some(m) = lib.method(owner, name).cloned() else {
            let offered = lib.offered(owner);
            self.error(format!("`{}.{name}(…)`: `{y_name}` has no method `{name}`; {owner} offers {}", y_text_of(&recv.code), offered.join(", ")));
            return ex(format!("{r}.{name}()"), Ty::Unknown, Form::Temp, ATOM);
        };
        if m.params.len() != args.len() {
            self.error(format!("`{}.{name}(…)`: {owner}::{name} takes {} arguments, and the call gives {}", y_text_of(&recv.code), m.params.len(), args.len()));
        }
        let elem = ty.elem();
        let named: Vec<(&str, Ty)> = match &ty {
            Ty::SortedMap(k, v) => vec![("K", (**k).clone()), ("V", (**v).clone())],
            Ty::Coll(_, a) if a.len() == 2 => vec![("K", a[0].clone()), ("V", a[1].clone())],
            _ => vec![],
        };
        let mut parts = vec![];
        for (k, (_, a)) in args.iter().enumerate() {
            let pt = m.params.get(k).map(|p| p.1.clone()).unwrap_or_default();
            let want = crate::library::y_type_of(&pt, &elem, &named);
            let x = self.expr_want(a, &want);
            parts.push(if pt.starts_with("ref<") { self.by_ref(x, &want) } else { self.owned(x, &want) });
        }
        // a method call borrows its receiver as the method takes it
        let call = format!("{r}.{name}({})", parts.join(", "));
        match m.result.as_deref() {
            None => ex(call, Ty::Unit, Form::Temp, ATOM),
            // a borrowed element, which may be absent: copied out, as a Y value is
            Some(t) if t.starts_with("ref<") && t.ends_with('?') => ex(format!("{call}.cloned()"), Ty::Opt(Box::new(crate::library::y_type_of(&t[..t.len() - 1], &elem, &named))), Form::Temp, ATOM),
            Some(t) if t.starts_with("ref<") => ex(call, crate::library::y_type_of(t, &elem, &named), Form::Ref, ATOM),
            Some(t) => ex(call, crate::library::y_type_of(t, &elem, &named), Form::Temp, ATOM),
        }
    }

    /// `text.matches(pattern"…")`: the runtime's matcher (`y_runtime::pattern`), on a pattern the compiler has checked
    /// is in the subset it takes; one that is not is a compile error.
    pub fn pattern_match(&mut self, pattern: &str, text: Ex) -> String {
        if let Err(e) = y_runtime::pattern::check(pattern) {
            self.error(e);
        }
        self.use_("y_runtime::pattern");
        let t = self.as_str(text);
        format!("pattern::matches({pattern:?}, {t})")
    }

    fn binary(&mut self, e: &Node) -> Ex {
        let op = e.str("op").unwrap();
        let (l, r) = (e.node("left").unwrap(), e.node("right").unwrap());
        match op {
            "and" | "or" => {
                let (rop, p) = if op == "and" { ("&&", 40) } else { ("||", 30) };
                let a = self.expr(l);
                // what the left operand shows holds where the right one is evaluated
                self.push_scope();
                let ns = narrowings(l, op == "and");
                self.bind_narrowed_aliases(&ns);
                let b = self.expr(r);
                self.pop_scope();
                ex(format!("{} {rop} {}", wrap(&a, p), wrap(&b, p + 1)), Ty::Bool, Form::Temp, p)
            }
            "==" | "!=" => {
                let neg = op == "!=";
                if is_member(r, "optional", "none") {
                    let a = self.expr(l);
                    return ex(format!("{}.{}()", wrap(&a, ATOM), if neg { "is_some" } else { "is_none" }), Ty::Bool, Form::Temp, ATOM);
                }
                if is_member(r, "result", "ok") {
                    let a = self.expr(l);
                    return ex(format!("{}.{}()", wrap(&a, ATOM), if neg { "is_err" } else { "is_ok" }), Ty::Bool, Form::Temp, ATOM);
                }
                if is_member(r, "result", "err") {
                    let a = self.expr(l);
                    return ex(format!("{}.{}()", wrap(&a, ATOM), if neg { "is_ok" } else { "is_err" }), Ty::Bool, Form::Temp, ATOM);
                }
                self.compare(l, r, op)
            }
            "<" | ">" | "<=" | ">=" => self.compare(l, r, op),
            "+" | "-" | "*" | "/" | "mod" | "%" => {
                let p = if op == "+" || op == "-" { 70 } else { 80 };
                let rop = if op == "mod" { "%" } else { op };
                let a = self.expr(l);
                let b = self.expr_want(r, &a.ty);
                if let Some(x) = self.decimal_binary(rop, a.clone(), b.clone()) {
                    return x;
                }
                if let Some(t) = self.unit_result(rop, &a.ty, &b.ty) {
                    return ex(format!("{} {rop} {}", paren(&a.code, a.prec, p), paren(&b.code, b.prec, p + 1)), t, Form::Temp, p);
                }
                let a = if a.ty == Ty::Int && b.ty != Ty::Int { ex(a.code, b.ty.clone(), a.form, a.prec) } else { a };
                let ty = a.ty.clone();
                let ac = a.code.clone();
                let bc = b.code.clone();
                ex(format!("{} {rop} {}", paren(&ac, a.prec, p), paren(&bc, b.prec, p + 1)), ty, Form::Temp, p)
            }
            _ => ex(format!("/* {op} */"), Ty::Unknown, Form::Temp, ATOM),
        }
    }

    fn compare(&mut self, l: &Node, r: &Node, op: &str) -> Ex {
        // `list.count()` against a `uint32`: the count as one
        let counted = l.kind == "call" && l.nodes("args").is_empty() && l.node("callee").is_some_and(|c| c.kind == "member" && c.str("name") == Some("count"));
        let a = if counted {
            let other = self.expr(r).ty;
            self.expr_want(l, &other)
        } else {
            self.expr(l)
        };
        if a.ty == Ty::List(Box::new(Ty::HostStr)) && r.kind == "list" {
            let items: Vec<String> = r.nodes("items").iter().map(|i| self.expr(i).code).collect();
            return ex(format!("{} {op} [{}]", wrap(&a, 51), items.join(", ")), Ty::Bool, Form::Temp, 50);
        }
        let b = self.expr_want(r, &a.ty);
        // a list and a constant's array: the array as a list
        let b = if matches!(a.ty, Ty::List(_)) && matches!(b.ty, Ty::Array(..)) {
            let t = a.ty.clone();
            let code = self.owned(b, &t);
            ex(code, t, Form::Temp, ATOM)
        } else {
            b
        };
        let a = if a.ty == Ty::Int { let t = b.ty.clone(); Ex { ty: t, ..a } } else { a };
        // an optional and a value, as where a condition has narrowed one of two optionals: the value, there
        let (a, b) = match (&a.ty, &b.ty) {
            (Ty::Opt(x), y) if **x == *y && matches!(op, "==" | "!=") => {
                let t = a.ty.clone();
                let v = self.owned(b, &t);
                (a, ex(v, t, Form::Temp, ATOM))
            }
            (x, Ty::Opt(y)) if **y == *x && matches!(op, "==" | "!=") => {
                let t = b.ty.clone();
                let v = self.owned(a, &t);
                (ex(v, t, Form::Temp, ATOM), b)
            }
            _ => (a, b),
        };
        // a decimal and a number as written: the number as a decimal of the same type
        let _ = self.unit_result("==", &a.ty, &b.ty);
        // two decimals or two integers of different constraints: each in one type that holds both
        if let Some((ac, bc)) = self.compare_numbers(&a, &b) {
            return ex(format!("{ac} {op} {bc}"), Ty::Bool, Form::Temp, 50);
        }
        let b = match (&a.ty, &b.ty) {
            (Ty::Decimal(p, s), _) if crate::ware::is_literal_number(&b) => ex(crate::ware::decimal_literal(&b.code, *p, *s), a.ty.clone(), Form::Temp, ATOM),
            // a count and an integer with bounds: the integer as a count
            (Ty::Count, Ty::Integer(..)) => ex(format!("{}.to_usize()", wrap(&b, ATOM)), Ty::Count, Form::Temp, ATOM),
            _ => b,
        };
        // a borrowed name meets an owned value: compare the values
        let (ac, bc) = match (a.form, b.form) {
            (Form::Ref | Form::Mut, Form::Temp | Form::Place | Form::Borrowed) if self.is_copy(&a.ty) && name_like(&a.code) => (format!("*{}", a.code), b.code.clone()),
            (Form::Temp | Form::Place | Form::Borrowed, Form::Ref | Form::Mut) if self.is_copy(&b.ty) && name_like(&b.code) => (a.code.clone(), format!("*{}", b.code)),
            _ => (wrap(&a, 51), wrap(&b, 51)),
        };
        ex(format!("{ac} {op} {bc}"), Ty::Bool, Form::Temp, 50)
    }

    fn fake(&mut self, v: &Node) -> Ex {
        // `fake console(input: [...], read_fails: true)`, `fake forever`
        // `fake enqueue<command_type=c>`: an outbox no region has added to
        if v.kind == "type" && v.str("name") == Some("enqueue") {
            let t = Ty::from_node(v);
            self.use_("y_runtime::shared::Shared");
            self.use_("y_runtime::command::Outbox");
            return ex("Shared::new(Outbox::new())", t, Form::Temp, ATOM);
        }
        let (cap, args): (&str, Vec<(Option<&str>, &Node)>) = match v.kind {
            "call" => (name_of(v.node("callee").unwrap()).unwrap_or(""), self.args(v)),
            _ => (name_of(v).unwrap_or(""), vec![]),
        };
        let ty = Ty::Cap(cap.to_string());
        self.note_type(&ty);
        let arg = |k: &str| args.iter().find(|(n, _)| *n == Some(k)).map(|(_, a)| *a);
        let code = match cap {
            "console" => {
                let input: Vec<String> = arg("input").map(|l| l.nodes("items").iter().map(|i| format!("\"{}\"", i.str("value").unwrap_or(""))).collect()).unwrap_or_default();
                let lines = format!("&[{}]", input.join(", "));
                if arg("failing_after_input").is_some() {
                    format!("Console::fake_failing_reads({lines})")
                } else if let Some(n) = arg("failing_after_writes") {
                    format!("Console::fake_failing_writes({lines}, {})", n.str("text").unwrap_or("0"))
                } else {
                    let fails = arg("read_fails").and_then(|b| b.str("text")).unwrap_or("false");
                    format!("Console::fake({lines}, {fails})")
                }
            }
            "forever" => "Forever::fake()".to_string(),
            "window" => {
                let frames = arg("frames");
                match frames {
                    Some(l) if l.kind == "list" && l.nodes("items").iter().all(|f| f.kind == "list") => {
                        let fs: Vec<String> = l.nodes("items").iter().map(|f| {
                            let keys: Vec<String> = f.nodes("items").iter().map(|k| self.expr(k).code).collect();
                            format!("&[{}]", keys.join(", "))
                        }).collect();
                        format!("Window::fake(&[{}])", fs.join(", "))
                    }
                    // frames made with `frame(…)`: the fake's own frames
                    Some(l) if l.kind == "list" => {
                        let fs: Vec<String> = l.nodes("items").iter().map(|f| self.expr(f).code).collect();
                        format!("Window::fake_frames(vec![{}])", fs.join(", "))
                    }
                    Some(l) => format!("Window::fake_frames({}.into_vec())", self.expr(l).code),
                    None => "Window::fake(&[])".to_string(),
                }
            }
            "store" => {
                let lines: Vec<String> = arg("lines").map(|l| l.nodes("items").iter().map(|i| { let x = self.expr(i); self.as_str(x) }).collect()).unwrap_or_default();
                return ex(format!("Store::fake(&[{}])", lines.join(", ")), Ty::Store(Box::new(Ty::Unknown)), Form::Temp, ATOM);
            }
            "http" => {
                let text = arg("text").map(|t| { let x = self.expr(t); self.as_str(x) });
                match (text, arg("texts"), arg("status"), arg("expiring"), arg("answers"), arg("then_status")) {
                    (Some(t), _, _, _, Some(n), Some(st)) => format!("Http::fake_failing_after({t}, {}, {})", self.expr(n).code, self.expr(st).code),
                    (Some(t), ..) => format!("Http::fake({t})"),
                    (_, Some(ts), ..) => {
                        let items: Vec<String> = ts.nodes("items").iter().map(|i| { let x = self.expr(i); self.as_str(x) }).collect();
                        format!("Http::fake_texts(&[{}])", items.join(", "))
                    }
                    (_, _, Some(st), ..) => format!("Http::fake_failing({})", self.expr(st).code),
                    (_, _, _, Some(_), ..) => "Http::fake_expiring()".to_string(),
                    _ => "Http::fake(\"\")".to_string(),
                }
            }
            "clock" => {
                let tick = arg("tick").map(|t| self.expr(t).code).unwrap_or_else(|| "Duration::from_millis(0)".into());
                format!("Clock::fake({tick})")
            }
            "random" => format!("Random::seeded({})", arg("seed").and_then(|s| s.str("text")).unwrap_or("0")),
            other => format!("{}::fake()", pascal(other)),
        };
        ex(code, ty, Form::Temp, ATOM)
    }

    /// The invariants of a structure a statement may break, checked after it: `game.misses.push(letter)` is followed by
    /// `invariant(game.misses.count() <= …)` when `game`'s structure has an invariant that reads `misses`.
    fn invariants_after(&mut self, st: &Node) {
        let Some(v) = st.node("value").filter(|v| v.kind == "call") else { return };
        let callee = v.node("callee").unwrap();
        if callee.kind != "member" {
            return;
        }
        let place = callee.node("object").unwrap();
        if place.kind != "member" {
            return;
        }
        let field = place.str("name").unwrap().to_string();
        let owner = place.node("object").unwrap();
        let ot = self.expr(owner).ty;
        let Ty::Named(t) = ot else { return };
        let Some(TypeDef::Struct { fields, invariants, .. }) = self.type_def(&t) else { return };
        let fnames: Vec<String> = fields.iter().map(|f| f.0.clone()).collect();
        for (name, _, cond) in invariants {
            let reads = { let mut r = false; cond.walk(&mut |n: &Node| if n.kind == "name" && n.str("name") == Some(field.as_str()) { r = true; }); r };
            if !reads {
                continue;
            }
            let c = with_fields(cond, &fnames, &y_text(owner));
            let x = self.expr(&c);
            self.use_("y_runtime::invariant");
            self.line(format!("invariant({}, \"{}::{}#{name}\");", x.code, self.ctx.module, pascal(&t)));
            self.invariant_sites.push((t.clone(), name.clone(), self.ctx.name.clone(), field.clone()));
        }
    }

    /// Whether a name is read by the statements after the current one, or by a loop around it that runs again.
    pub(crate) fn still_read(&self, name: &str) -> bool {
        let reads = |n: &Node| {
            let mut found = false;
            n.walk(&mut |x: &Node| {
                if x.kind == "name" && x.str("name") == Some(name) {
                    found = true;
                }
            });
            found
        };
        // a loop's own name is bound afresh on each pass
        let own = |l: &&Node| l.kind == "for" && l.str("name") == Some(name);
        self.rest_outer.iter().any(reads) || self.loops.iter().filter(|l| !own(l)).any(reads) || matches!(self.var(name).map(|v| v.form), Some(Form::Ref))
    }

    /// Why a copy is made: the function the copy is passed to `mutable`, which changes it.
    fn why_copied(&self, copy: &str, of: &str) -> String {
        let mut changer = None;
        for n in &self.rest {
            n.walk(&mut |x: &Node| {
                if changer.is_none() && x.kind == "call" {
                    let passes = x.nodes("args").iter().any(|a| a.node("value").is_some_and(|v| v.kind == "mutable" && v.node("value").and_then(name_of) == Some(copy)));
                    if passes {
                        changer = x.node("callee").and_then(name_of).map(str::to_string);
                    }
                }
            });
        }
        match changer {
            Some(f) => format!("{f} changes it, and {of} is still read"),
            None => format!("it changes, and {of} is still read"),
        }
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Statements

    /// Statements in the current scope, the last of them the function's value.
    pub fn block_open_tail(&mut self, stmts: &[&Node]) {
        self.stmts(stmts, true);
    }

    /// Statements in the current scope.
    pub fn block_open_tail_or(&mut self, stmts: &[&Node], tail: bool) {
        self.stmts(stmts, tail);
    }

    pub fn block(&mut self, stmts: &[&Node], tail: bool) {
        self.push_scope();
        self.stmts(stmts, tail);
        self.pop_scope();
    }

    fn stmts(&mut self, stmts: &[&Node], tail: bool) {
        for (k, s) in stmts.iter().enumerate() {
            let saved = std::mem::replace(&mut self.rest, stmts[k + 1..].iter().map(|n| (*n).clone()).collect());
            let outer = self.rest_outer.clone();
            self.rest_outer.extend(stmts[k + 1..].iter().map(|n| (*n).clone()));
            let start = self.out.len();
            self.stmt(s, tail && k + 1 == stmts.len(), &stmts[k + 1..]);
            self.last_start = start;
            self.rest_outer = outer;
            self.rest = saved;
        }
    }

    fn claim(&self, st: &Node) -> String {
        if let Some(v) = st.node("value").filter(|v| v.kind == "compile") {
            return format!(" // y: compile {}", y_text(v.node("value").unwrap()));
        }
        st.node("value").and_then(find_forever).map(|b| format!(" // y: forever because \"{b}\"")).unwrap_or_default()
    }

    /// A statement that is not in the function's tail.
    pub fn stmt_pub(&mut self, st: &Node) {
        self.stmt(st, false, &[]);
    }

    /// A statement, in the tail or not.
    pub fn stmt_tail(&mut self, st: &Node, tail: bool) {
        self.stmt(st, tail, &[]);
    }

    /// A statement; `tail` when its value is the function's.
    fn stmt(&mut self, st: &Node, tail: bool, _rest: &[&Node]) {
        self.copy_line = st.line;
        if self.kind_stmt(st) {
            return;
        }
        // `name = lambda name(…):` (Y § 3.1)
        if st.kind == "let" && st.node("value").is_some_and(|v| v.kind == "lambda") {
            let name = st.str("name").unwrap().to_string();
            let f = st.node("value").unwrap().clone();
            self.local_function(&f, Some(&name));
            return;
        }
        match st.kind {
            "let" => {
                let name = st.str("name").unwrap();
                let v = st.node("value").unwrap();
                self.binding = Some(name.to_string());
                let injected = if self.ctx.section.as_deref() == Some("inject") {
                    self.module().fns.values().flat_map(|f| f.params.iter()).find(|p| p.injected && p.name == name).map(|p| p.ty.clone())
                } else {
                    None
                };
                let x = match &injected {
                    Some(t) => self.expr_want(v, t),
                    None => self.expr(v),
                };
                self.binding = None;
                let claim = self.claim(st);
                let x = if x.ty == Ty::HostStr && v.kind != "or_fail" {
                    let code = self.owned(x, &Ty::Str);
                    ex(code, Ty::Str, Form::Temp, ATOM)
                } else {
                    x
                };
                let mut marker = String::new();
                let (code, form) = match x.form {
                    Form::Ref | Form::Mut if self.is_copy(&x.ty) && name_like(&x.code) => (format!("*{}", x.code), Form::Place),
                    // `mutable after = game` while `game` is still read: a copy (Y § 3.2)
                    Form::Ref | Form::Mut | Form::Place if st.flag("mutable") && v.kind != "call" && !self.is_copy(&x.ty) && self.still_read(&x.code) => {
                        let why = self.why_copied(name, &x.code);
                        marker = format!(" // y: a copy, because {why}");
                        self.copies.push((st.line, crate::lower::y_text(v), why));
                        (format!("{}.clone()", wrap(&x, ATOM)), Form::Place)
                    }
                    Form::Ref | Form::Mut => (x.code.clone(), x.form),
                    // a member of a borrowed value: copied to change it, borrowed to read it
                    Form::Borrowed if !self.is_copy(&x.ty) && st.flag("mutable") => {
                        let why = self.why_copied(name, &x.code);
                        marker = format!(" // y: a copy, because {why}");
                        self.copies.push((st.line, crate::lower::y_text(v), why));
                        (format!("{}.clone()", wrap(&x, ATOM)), Form::Place)
                    }
                    Form::Borrowed if !self.is_copy(&x.ty) => (format!("&{}", x.code), Form::Ref),
                    _ => (x.code.clone(), Form::Place),
                };
                if matches!(&x.ty, Ty::Named(n) if n.starts_with("batch:")) {
                    self.held = Some(name.to_string());
                }
                // `mutable named = map.new()`, returned as the result: its elements are the result's
                let mut x = x;
                let mut annotation = String::new();
                if let (true, Some(rt)) = (self.ctx.returned.iter().any(|r| r == name), self.ctx.result.clone()) {
                    let rt = self.module().resolve(&rt);
                    let empty = matches!((&x.ty, &rt), (Ty::Coll(a, e), Ty::Coll(b, _)) if a == b && e.is_empty())
                        || matches!((&x.ty, &rt), (Ty::Set(e), Ty::Set(_)) | (Ty::List(e), Ty::List(_)) if **e == Ty::Unknown)
                        || matches!((&x.ty, &rt), (Ty::SortedMap(k, _), Ty::SortedMap(..)) if **k == Ty::Unknown);
                    if empty {
                        annotation = format!(": {}", self.rust_type(&rt));
                        x.ty = rt;
                    }
                }
                self.line(format!("let {name}{annotation} = {code};{claim}{marker}"));
                if x.ty == Ty::HostStr && v.kind == "or_fail" {
                    self.line(format!("let {name} = YString::from({name});"));
                    self.bind(name, Ty::Str, Form::Place);
                    return;
                }
                self.bind(name, x.ty, form);
            }
            "assign" => {
                let mut target = self.expr(st.node("target").unwrap());
                // a number the function was lent to change: `*left = …`
                if target.form == Form::Mut && name_like(&target.code) && self.is_copy(&target.ty) {
                    target.code = format!("*{}", target.code);
                }
                let v = st.node("value").unwrap();
                let tt = target.ty.clone();
                // `x := x + e` is `x += e`; `x := x.xor(e)` is `x ^= e`
                if v.kind == "binary" && matches!(v.str("op"), Some("+" | "-")) && same(v.node("left").unwrap(), st.node("target").unwrap()) {
                    let b = self.expr_want(v.node("right").unwrap(), &tt);
                    self.line(format!("{} {}= {};", target.code, v.str("op").unwrap(), b.code));
                    return;
                }
                if v.kind == "call" {
                    let c = v.node("callee").unwrap();
                    if c.kind == "member" && c.str("name") == Some("xor") && same(c.node("object").unwrap(), st.node("target").unwrap()) {
                        let b = self.expr_want(v.nodes("args")[0].node("value").unwrap(), &tt);
                        self.line(format!("{} ^= {};", target.code, b.code));
                        return;
                    }
                }
                let x = self.expr_want(v, &tt);
                let val = self.owned(x, &tt);
                let claim = self.claim(st);
                self.line(format!("{} = {val};{claim}", target.code));
            }
            "expr" => {
                let x = self.expr(st.node("value").unwrap());
                let claim = self.claim(st);
                self.line(format!("{};{claim}", x.code));
                self.invariants_after(st);
            }
            "return" if self.atomic_result.is_some() => {
                // a region's value: the shared state it changed is written, then the value is given
                let v = st.node("value").unwrap();
                let want = self.ctx.result.clone().unwrap_or(Ty::Unknown);
                let x = self.expr_want(v, &want);
                let val = self.owned(x, &want);
                for n in self.atomic_result.clone().unwrap_or_default() {
                    self.line(format!("tx.set({n}, held_{n});"));
                }
                self.line(format!("{}Ok(Step::Done({val})){}", if tail { "" } else { "return " }, if tail { "" } else { ";" }));
            }
            "return" if !self.ctx.ensures.is_empty() && (!tail || self.ctx.ensure_tail) => self.return_checked(st, tail),
            "return" => {
                // a function without a result: its `return` takes no value
                let Some(v) = st.node("value") else {
                    if self.ctx.fails.is_some() {
                        self.ret("Ok(())".into(), tail);
                    } else if !tail {
                        self.line("return;");
                    }
                    return;
                };
                // `return r = f(…) or fail`: a way out of its own, so the call is bound as `r`, with its `?` where a path
                // trace sees it
                if let (true, Some(name), Some(_)) = (v.kind == "or_fail", st.str("name"), &self.ctx.fails) {
                    let want = self.ctx.result.clone().unwrap_or(Ty::Unknown);
                    let saved = self.want_decimal;
                    if let Ty::Decimal(p, s) = self.module().resolve(&want) {
                        self.want_decimal = Some((p, s));
                    }
                    let x = self.expr_want(v, &want);
                    self.want_decimal = saved;
                    let val = self.owned(x, &want);
                    self.line(format!("let {name} = {val};"));
                    self.ret(format!("Ok({name})"), tail);
                    return;
                }
                let code = self.result_value(v);
                self.ret(code, tail);
            }
            "fail" => {
                let v = st.node("value").unwrap();
                if let Some(what) = ["not_reachable", "not_implemented"].into_iter().find(|w| name_of(v) == Some(*w)) {
                    self.use_(&format!("y_runtime::{what}"));
                    let label = format!("{}::{}#{}", self.ctx.module, self.ctx.name, self.ctx.branches.last().cloned().unwrap_or_default());
                    self.line(format!("{what}(\"{label}\"){}", if tail { "" } else { ";" }));
                    return;
                }
                let fails = self.ctx.fails.clone().unwrap_or(Ty::Unknown);
                let x = self.expr_want(v, &fails);
                let val = self.owned(x, &fails);
                self.ret(format!("Err({val})"), tail);
            }
            "continue" => self.line("continue;"),
            "break" => self.line("break;"),
            "if" => self.if_(st, tail),
            "conditional" => self.conditional(st, tail),
            "compare" => self.compare_stmt(st, tail),
            "while" => self.loop_(st),
            "for" => self.loop_(st),
            "region" => self.region(st, tail),
            "arena" => self.arena(st, tail),
            "inject" => {
                self.line("{");
                self.ind += 1;
                let text: Vec<String> = st.nodes("bindings").iter().map(|b| format!("{} = {}", b.str("name").unwrap(), y_text(b.node("value").unwrap()))).collect();
                self.line(format!("// y: inject({})", text.join(", ")));
                self.push_scope();
                for b in st.nodes("bindings") {
                    let x = self.expr(b.node("value").unwrap());
                    let name = b.str("name").unwrap();
                    self.line(format!("let {name} = {};", x.code));
                    self.bind(name, x.ty, Form::Place);
                }
                self.block(&st.nodes("body"), false);
                self.pop_scope();
                self.ind -= 1;
                self.line("}");
            }
            k => self.line(format!("/* {k} */")),
        }
    }

    /// `region name is c₁, c₂:` and its statements: the marker `// y: region name is …` where it starts, on the line of
    /// its one statement or above several, and `// y: end region name` after them when statements follow it.
    fn region(&mut self, st: &Node, tail: bool) {
        let words: Vec<&str> = st.nodes("constraints").iter().filter_map(|c| c.str("word")).collect();
        // what goes with what (constraint language § 6.23); a region that breaks it does not compile
        let has = |w: &str| words.contains(&w);
        let name = st.str("name").unwrap_or("");
        let mut seen = BTreeSet::new();
        for w in &words {
            if !seen.insert(*w) {
                self.error(format!("region `{name}` says `{w}` twice"));
            }
            if !["within", "forever", "uncancellable", "atomic", "isolation", "waits", "supervise", "detach"].contains(w) {
                self.error(format!("region `{name}`: `{w}` is not a constraint a region takes (Y § 3.8)"));
            }
        }
        for (w, needs, why) in [("isolation", "atomic", "is what an atomic region observes"), ("waits", "atomic", "starts a region once its condition holds, which only an atomic region can wait for"), ("uncancellable", "within", "holds cancellation off for a bounded time"), ("detach", "within", "lets work outlive its scope for an explicit time")] {
            if has(w) && !has(needs) {
                self.error(format!("region `{name}`: `{w}` {why}, and the region is not `{needs}`"));
            }
        }
        if has("within") && has("forever") {
            self.error(format!("region `{name}` is `within` a bound or waits `forever`, not both"));
        }
        if has("atomic") {
            return self.region_atomic(st, tail);
        }
        if words.contains(&"supervise") {
            return self.region_tasks(st);
        }
        if words.contains(&"detach") {
            return self.region_detach(st);
        }
        if words.contains(&"uncancellable") {
            return self.region_uncancellable(st);
        }
        let name = st.str("name").unwrap();
        let cs: Vec<String> = st.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
        let marker = format!("// y: region {name} is {}", cs.join(", "));
        let body = st.nodes("body");
        self.regions.push((self.ctx.name.clone(), name.to_string(), st.line, cs.clone()));
        if let Some(bound) = st.nodes("constraints").iter().find(|c| c.str("word") == Some("within")).and_then(|c| c.node("bound")) {
            // `within d or fail`: a deadline the region's waits are given
            let b = self.expr(bound).code;
            self.use_("y_runtime::Deadline");
            self.line("{");
            self.ind += 1;
            self.line(marker);
            self.line(format!("let deadline = Deadline::after({b});"));
            self.push_scope();
            self.bind("deadline", Ty::Named("deadline".into()), Form::Place);
            self.stmts(&body, false);
            self.pop_scope();
            if !self.rest.is_empty() {
                self.line(format!("// y: end region {name}"));
            }
            self.ind -= 1;
            self.line("}");
            return;
        }
        if body.len() == 1 && matches!(body[0].kind, "let" | "expr" | "assign") {
            let start = self.out.len();
            self.stmts(&body, tail);
            if self.out.len() == start + 1 {
                self.out[start].push_str(&format!(" {marker}"));
                return;
            }
            self.out.truncate(start);
        }
        self.line(marker);
        self.stmts(&body, tail);
        if !self.rest.is_empty() {
            self.line(format!("// y: end region {name}"));
        }
    }

    /// The function's `ensures` clauses, checked on its result as its Y name binds it.
    pub fn ensure_lines(&mut self) {
        for w in self.ctx.ensures.clone() {
            self.use_("y_runtime::ensure");
            let c = self.expr(w.node("condition").unwrap());
            self.line(format!("ensure({}, \"{}::{}#{}\");", c.code, self.ctx.module, self.ctx.name, w.str("name").unwrap()));
        }
    }

    /// A `return` of a function with `ensures`: the value is bound to the result's name, checked, and given (Y § 3.11:
    /// what a function ensures holds on every way it returns).
    fn return_checked(&mut self, st: &Node, tail: bool) {
        let r = self.ctx.result_name.clone();
        let Some(v) = st.node("value") else {
            self.ensure_lines();
            if self.ctx.fails.is_some() {
                self.ret("Ok(())".into(), tail);
            } else if !tail {
                self.line("return;");
            }
            return;
        };
        let want = self.ctx.result.clone().unwrap_or(Ty::Unknown);
        let x = self.expr_want(v, &want);
        let val = self.owned(x, &want);
        self.line(format!("let {r} = {val};"));
        self.push_scope();
        self.bind(&r, want, Form::Place);
        self.ensure_lines();
        self.pop_scope();
        let out = if self.ctx.fails.is_some() { format!("Ok({r})") } else { r };
        self.ret(out, tail);
    }

    /// The value a `return` gives, wrapped as the function's result.
    fn result_value(&mut self, v: &Node) -> String {
        let want = self.ctx.result.clone().unwrap_or(Ty::Unknown);
        if self.ctx.fails.is_some() {
            // `return r = f(…) or fail` with the same failure: the call's result is the function's
            if v.kind == "or_fail" {
                let saved = self.want_decimal;
                if let Ty::Decimal(p, s) = self.module().resolve(&want) {
                    self.want_decimal = Some((p, s));
                }
                let x = self.expr(v.node("value").unwrap());
                self.want_decimal = saved;
                if let Ty::Res(_, e) = &x.ty {
                    if Some(&**e) == self.ctx.fails.as_ref() {
                        return x.code;
                    }
                }
            }
            let x = self.expr_want(v, &want);
            return format!("Ok({})", self.owned(x, &want));
        }
        let x = self.expr_want(v, &want);
        self.owned(x, &want)
    }

    fn ret(&mut self, code: String, tail: bool) {
        if tail {
            self.line(code);
        } else {
            self.line(format!("return {code};"));
        }
    }

    fn branch_block(&mut self, name: &str, body: &[&Node], tail: bool) {
        self.ctx.branches.push(name.to_string());
        self.ind += 1;
        self.block(body, tail);
        self.ind -= 1;
        self.ctx.branches.pop();
    }

    fn if_(&mut self, st: &Node, tail: bool) {
        let name = st.str("name").unwrap();
        let cond = st.node("condition").unwrap();
        let body = st.nodes("body");
        let els = st.node("else");
        let ename = els.and_then(|e| e.str("name")).unwrap_or("");
        let ebody = els.map(|e| e.nodes("body")).unwrap_or_default();
        // a condition that narrows: `if a = (x == optional.none): fail …` with `else b:` is a let-else
        if cond.kind == "binary" && cond.str("op") == Some("==") && is_member(cond.node("right").unwrap(), "optional", "none") && diverges(&body) {
            if let Some(x) = name_of(cond.node("left").unwrap()) {
                let inner = self.var(x).map(|v| v.ty.elem()).unwrap_or(Ty::Unknown);
                self.line(format!("let Some({x}) = {x} else {{ // y: #{name}, else #{ename}"));
                self.branch_block(name, &body, false);
                self.line("};");
                if inner == Ty::HostStr {
                    // the console's text, made a string once it is known to be there
                    self.line(format!("let {x} = YString::from({x});"));
                    self.bind(x, Ty::Str, Form::Place);
                } else {
                    self.bind(x, inner, Form::Place);
                }
                self.ctx.branches.push(ename.to_string());
                for (k, s) in ebody.iter().enumerate() {
                    self.stmt(s, tail && k + 1 == ebody.len(), &ebody[k + 1..]);
                }
                self.ctx.branches.pop();
                return;
            }
        }
        // `if a = (x != optional.none):`: a `match`, with the name narrowed in `Some`
        if cond.kind == "binary" && cond.str("op") == Some("!=") && is_member(cond.node("right").unwrap(), "optional", "none") && els.is_some() {
            if let Some(x) = name_of(cond.node("left").unwrap()) {
                let inner = self.var(x).map(|v| v.ty.elem()).unwrap_or(Ty::Unknown);
                self.line(format!("match {x} {{"));
                self.ind += 1;
                self.push_scope();
                self.bind(x, inner, Form::Place);
                self.arm(&format!("Some({x})"), name, name, &body, tail);
                self.pop_scope();
                self.arm("None", ename, ename, &ebody, tail);
                self.ind -= 1;
                self.line("}");
                return;
            }
        }
        // `if a = (x == optional.none):` whose branch goes on: a `match`, with the name narrowed in `Some`
        if cond.kind == "binary" && cond.str("op") == Some("==") && is_member(cond.node("right").unwrap(), "optional", "none") && els.is_some() {
            if let Some(x) = name_of(cond.node("left").unwrap()) {
                let inner = self.var(x).map(|v| v.ty.elem()).unwrap_or(Ty::Unknown);
                self.line(format!("match {x} {{"));
                self.ind += 1;
                self.arm("None", name, name, &body, tail);
                self.push_scope();
                self.bind(x, inner, Form::Place);
                self.arm(&format!("Some({x})"), ename, ename, &ebody, tail);
                self.pop_scope();
                self.ind -= 1;
                self.line("}");
                return;
            }
        }
        // `if a = ((x == optional.none) or (y == optional.none)):` that fails: a `match` on the names, an arm for each
        // operand of the `or`, and one in which each name is narrowed
        let ops = or_operands(cond);
        if ops.len() > 1 && diverges(&body) && ops.iter().all(|o| o.kind == "binary" && o.str("op") == Some("==") && is_member(o.node("right").unwrap(), "optional", "none") && name_of(o.node("left").unwrap()).is_some()) {
            let names: Vec<String> = ops.iter().map(|o| name_of(o.node("left").unwrap()).unwrap().to_string()).collect();
            self.line(format!("match ({}) {{", names.join(", ")));
            self.ind += 1;
            for k in 0..names.len() {
                let pat: Vec<&str> = (0..names.len()).map(|j| if j == k { "None" } else { "_" }).collect();
                self.arm(&format!("({})", pat.join(", ")), &format!("{name}'{}", k + 1), name, &body, tail);
            }
            let pat: Vec<String> = names.iter().map(|n| format!("Some({n})")).collect();
            self.push_scope();
            for n in &names {
                let t = self.var(n).map(|v| v.ty.elem()).unwrap_or(Ty::Unknown);
                self.bind(n, t, Form::Place);
            }
            self.arm(&format!("({})", pat.join(", ")), ename, ename, &ebody, tail);
            self.pop_scope();
            self.ind -= 1;
            self.line("}");
            return;
        }
        // `if a = (r == result.ok):`: a `match`, with the result narrowed in each arm
        if els.is_some() && cond.kind == "binary" && matches!(cond.str("op"), Some("==" | "!=")) && (is_member(cond.node("right").unwrap(), "result", "ok") || is_member(cond.node("right").unwrap(), "result", "err")) {
            if let Some(x) = name_of(cond.node("left").unwrap()) {
                if let Some(Ty::Res(ok, err)) = self.var(x).map(|v| v.ty.clone()) {
                    let holds_ok = (cond.str("op") == Some("==")) == is_member(cond.node("right").unwrap(), "result", "ok");
                    let subject = match self.var(x).map(|v| v.form) { Some(Form::Ref | Form::Mut) => x.to_string(), _ if self.is_copy(&ok) && self.is_copy(&err) => x.to_string(), _ => format!("&{x}") };
                    let (ok_form, err_form) = if subject != x || matches!(self.var(x).map(|v| v.form), Some(Form::Ref | Form::Mut)) { (Form::Ref, Form::Ref) } else { (Form::Place, Form::Place) };
                    self.line(format!("match {subject} {{"));
                    self.ind += 1;
                    let arms = if holds_ok { [(true, name, &body), (false, ename, &ebody)] } else { [(false, name, &body), (true, ename, &ebody)] };
                    for (is_ok, n, b) in arms {
                        self.push_scope();
                        let t = if is_ok { (*ok).clone() } else { (*err).clone() };
                        self.scopes.last_mut().unwrap().insert(x.to_string(), Var { ty: t, form: if is_ok { ok_form } else { err_form }, arm: Some(is_ok), alias: None });
                        let used = b.iter().any(|st| { let mut u = false; st.walk(&mut |m: &Node| if m.kind == "name" && m.str("name") == Some(x) { u = true; }); u });
                        let bound = if used { x } else { "_" };
                        self.arm(&format!("{}({bound})", if is_ok { "Ok" } else { "Err" }), n, n, b, tail);
                        self.pop_scope();
                    }
                    self.ind -= 1;
                    self.line("}");
                    return;
                }
            }
        }
        let c = self.expr(cond);
        self.line(format!("if {} {{ // y: #{name}", c.code));
        self.branch_block_narrowed(name, &body, tail, &narrowings(cond, true));
        if els.is_some() {
            self.line(format!("}} else {{ // y: #{ename}"));
            self.branch_block_narrowed(ename, &ebody, tail, &narrowings(cond, false));
        }
        self.line("}");
    }

    /// An arm of a `match`: on one line when it is one value, a block otherwise.
    pub(crate) fn arm(&mut self, pat: &str, marker: &str, branch: &str, body: &[&Node], tail: bool) {
        if tail && body.len() == 1 && matches!(body[0].kind, "return" | "fail") && !is_not_reachable(body[0]) {
            let start = self.out.len();
            self.ctx.branches.push(branch.to_string());
            self.stmt(body[0], true, &[]);
            self.ctx.branches.pop();
            if self.out.len() == start + 1 {
                let v = self.out.pop().unwrap();
                // a marker the statement had, such as a copy's, follows the arm's
                let (code, extra) = match v.split_once(" // y:") {
                    Some((c, e)) => (c.trim().to_string(), format!(" // y:{e}")),
                    None => (v.trim().to_string(), String::new()),
                };
                self.line(format!("{pat} => {code}, // y: #{marker}{extra}"));
                return;
            }
            self.out.truncate(start);
        }
        self.line(format!("{pat} => {{ // y: #{marker}"));
        self.branch_block(branch, body, tail);
        self.line("}");
    }

    fn conditional(&mut self, st: &Node, tail: bool) {
        let label = st.str("label").unwrap();
        let conds = st.nodes("conditions");
        let els = st.node("else");
        match st.node("subject") {
            None => {
                // a condition that narrows, `condition l.c = (x == optional.none): fail …`: the conditions before it are
                // guards, it is a let-else, and the ones after it go on as a chain
                let narrowing = |c: &Node| {
                    let t = c.node("test").unwrap();
                    t.kind == "binary" && t.str("op") == Some("==") && is_member(t.node("right").unwrap(), "optional", "none") && name_of(t.node("left").unwrap()).is_some() && diverges(&c.nodes("body"))
                };
                let mut conds = conds.clone();
                while let Some(k) = conds.iter().position(|c| narrowing(c)) {
                    for c in &conds[..k] {
                        let x = self.expr(c.node("test").unwrap());
                        let name = c.str("name").unwrap();
                        self.line(format!("if {} {{ // y: #{name}", x.code));
                        self.branch_block(name, &c.nodes("body"), false);
                        self.line("}");
                    }
                    let c = conds[k];
                    let x = name_of(c.node("test").unwrap().node("left").unwrap()).unwrap().to_string();
                    let name = c.str("name").unwrap();
                    let inner = self.var(&x).map(|v| v.ty.elem()).unwrap_or(Ty::Unknown);
                    self.line(format!("let Some({x}) = {x} else {{ // y: #{name}"));
                    self.branch_block(name, &c.nodes("body"), false);
                    self.line("};");
                    self.bind(&x, inner, Form::Place);
                    conds = conds[k + 1..].to_vec();
                }
                // what the conditions before one show by not holding holds where it is tested, and in its block
                let mut before: Vec<(String, Narrow)> = vec![];
                self.push_scope();
                for (k, c) in conds.iter().enumerate() {
                    let t = c.node("test").unwrap();
                    let name = c.str("name").unwrap();
                    let lead = if k == 0 { "" } else { "} else " };
                    // `condition l.c = (x != optional.none):` narrows `x` in its statements
                    if t.kind == "binary" && t.str("op") == Some("!=") && is_member(t.node("right").unwrap(), "optional", "none") {
                        if let Some(x) = name_of(t.node("left").unwrap()) {
                            let inner = self.var(x).map(|v| v.ty.elem()).unwrap_or(Ty::Unknown);
                            self.line(format!("{lead}if let Some({x}) = {x} {{ // y: #{name}"));
                            self.push_scope();
                            self.bind(x, inner, Form::Place);
                            self.branch_block(name, &c.nodes("body"), tail);
                            self.pop_scope();
                            continue;
                        }
                    }
                    let x = self.expr(t);
                    self.line(format!("{lead}if {} {{ // y: #{name}", x.code));
                    let mut ns = before.clone();
                    ns.extend(narrowings(t, true));
                    self.branch_block_narrowed(name, &c.nodes("body"), tail, &ns);
                    let shown = narrowings(t, false);
                    self.bind_narrowed_aliases(&shown);
                    before.extend(shown);
                }
                if let Some(e) = els {
                    let name = e.str("name").unwrap();
                    self.line(format!("}} else {{ // y: #{name}"));
                    self.branch_block_narrowed(name, &e.nodes("body"), tail, &before);
                }
                self.pop_scope();
                self.line("}");
            }
            Some(subject) => {
                let s = self.expr(subject);
                // a condition that is one test of the label against a case, with no other test joined to it
                let plain_case = |t: &Node| t.kind == "binary" && matches!(t.str("op"), Some("==" | "!=")) && name_of(t.node("left").unwrap()) == Some(label);
                let all_plain = conds.iter().all(|c| plain_case(c.node("test").unwrap()));
                let by_pattern = conds.iter().all(|c| {
                    let t = c.node("test").unwrap();
                    t.kind == "binary" && t.node("right").is_some_and(|r| r.kind == "call")
                });
                if matches!(s.ty, Ty::Res(..)) && by_pattern && all_plain {
                    // `condition said.hit = (said == result.ok(verdict.hit))`: an arm per value
                    self.line(format!("match {} {{", s.code));
                    self.ind += 1;
                    for c in &conds {
                        let right = c.node("test").unwrap().node("right").unwrap();
                        self.push_scope();
                        let pat = match self.case_pattern(right, label, &c.nodes("body"), if matches!(s.form, Form::Ref | Form::Mut) { Form::Ref } else { Form::Place }) {
                            Some(p) => p,
                            None => self.expr(right).code,
                        };
                        let name = c.str("name").unwrap();
                        self.arm(&pat, name, name, &c.nodes("body"), tail);
                        self.pop_scope();
                    }
                    if let Some(e) = els {
                        let name = e.str("name").unwrap();
                        self.arm("_", name, name, &e.nodes("body"), tail);
                    }
                    self.ind -= 1;
                    self.line("}");
                    return;
                }
                if let (Ty::Res(ok, err), true) = (s.ty.clone(), all_plain) {
                    // a borrowed result is matched as it is, and its arms bind references
                    let arm_form = if matches!(s.form, Form::Ref | Form::Mut) { Form::Ref } else { Form::Place };
                    self.line(format!("match {} {{", s.code));
                    self.ind += 1;
                    // each arm: success or failure (`None` for the rest), its name, its block, and the case it names
                    let mut arms: Vec<(Option<bool>, String, Vec<&Node>, Option<&Node>)> = vec![];
                    for c in &conds {
                        let r = c.node("test").unwrap().node("right").unwrap();
                        let with_case = |n: &str| r.kind == "call" && r.node("callee").is_some_and(|c| is_member(c, "result", n));
                        let (is_ok, case) = if with_case("ok") { (true, Some(r)) } else if with_case("err") { (false, Some(r)) } else { (is_member(r, "result", "ok"), None) };
                        arms.push((Some(is_ok), c.str("name").unwrap().to_string(), c.nodes("body"), case));
                    }
                    if let Some(e) = els {
                        let cased = arms.iter().any(|a| a.3.is_some());
                        let has_ok = arms.iter().any(|a| a.0 == Some(true) && a.3.is_none());
                        arms.push((if cased { None } else { Some(!has_ok) }, e.str("name").unwrap().to_string(), e.nodes("body"), None));
                    }
                    for (is_ok, name, body, case) in arms {
                        self.push_scope();
                        let pat = match (is_ok, case) {
                            (None, _) => "_".to_string(),
                            (Some(_), Some(r)) => match self.case_pattern(r, label, &body, arm_form) {
                                Some(p) => p,
                                None => self.expr(r).code,
                            },
                            (Some(is_ok), None) => {
                                let t = if is_ok { (*ok).clone() } else { (*err).clone() };
                                self.scopes.last_mut().unwrap().insert(label.to_string(), Var { ty: t, form: arm_form, arm: Some(is_ok), alias: None });
                                let used = body.iter().any(|b| { let mut u = false; b.walk(&mut |n: &Node| if n.kind == "name" && n.str("name") == Some(label) { u = true; }); u });
                                let bound = if used { label.to_string() } else { "_".to_string() };
                                if is_ok { format!("Ok({bound})") } else { format!("Err({bound})") }
                            }
                        };
                        self.arm(&pat, &name, &name, &body, tail);
                        self.pop_scope();
                    }
                    self.ind -= 1;
                    self.line("}");
                    return;
                }
                if let (Ty::Opt(inner), true) = (s.ty.clone(), all_plain) {
                    // `conditional held = …:` over an optional: `None`, and `Some(held)`, which narrows it; a place
                    // whose value is not copied is borrowed
                    let borrowed = s.form != Form::Temp && !self.is_copy(&inner);
                    let subject = if borrowed && matches!(s.form, Form::Place | Form::Borrowed) { format!("&{}", s.code) } else { s.code.clone() };
                    let inner_form = if borrowed { Form::Ref } else { Form::Place };
                    self.line(format!("match {subject} {{"));
                    self.ind += 1;
                    for c in &conds {
                        let t = c.node("test").unwrap();
                        let none = t.kind == "binary" && is_member(t.node("right").unwrap(), "optional", "none");
                        self.push_scope();
                        if !none {
                            self.bind(label, (*inner).clone(), inner_form);
                        }
                        let name = c.str("name").unwrap();
                        let pat = if none { "None".to_string() } else { format!("Some({label})") };
                        self.arm(&pat, name, name, &c.nodes("body"), tail);
                        self.pop_scope();
                    }
                    if let Some(e) = els {
                        self.push_scope();
                        self.bind(label, (*inner).clone(), inner_form);
                        let name = e.str("name").unwrap();
                        self.arm(&format!("Some({label})"), name, name, &e.nodes("body"), tail);
                        self.pop_scope();
                    }
                    self.ind -= 1;
                    self.line("}");
                    return;
                }
                if let Ty::Named(st) = s.ty.clone() {
                    if let Some(TypeDef::State { .. }) = self.type_def(&st) {
                        return self.conditional_state(st.as_str(), label, &s, &conds, els, tail);
                    }
                }
                let is_case = |o: &Node| o.kind == "binary" && o.str("op") == Some("==") && name_of(o.node("left").unwrap()) == Some(label) && o.node("right").is_some_and(|r| r.kind == "member");
                let guarded = |t: &Node| t.kind == "binary" && t.str("op") == Some("and") && is_case(t.node("left").unwrap());
                let by_case = !matches!(s.ty, Ty::Opt(_) | Ty::Res(..)) && conds.iter().all(|c| {
                    let t = c.node("test").unwrap();
                    is_case(t) || guarded(t) || (or_operands(t).len() > 1 && or_operands(t).iter().all(|o| is_case(o)))
                });
                if !by_case {
                    // the conditions test the value's members: it is named, and they are a chain
                    let code = match s.form {
                        Form::Temp => s.code.clone(),
                        _ if self.is_copy(&s.ty) => s.code.clone(),
                        _ => format!("&{}", s.code),
                    };
                    let form = if matches!(s.form, Form::Temp) || self.is_copy(&s.ty) { Form::Place } else { Form::Ref };
                    self.line(format!("let {label} = {code};"));
                    self.bind(label, s.ty.clone(), form);
                    // what the conditions before one show by not holding holds where it is tested, and in its block
                    let mut before: Vec<(String, Narrow)> = vec![];
                    self.push_scope();
                    for (k, c) in conds.iter().enumerate() {
                        let t = c.node("test").unwrap();
                        let x = self.expr(t);
                        let name = c.str("name").unwrap();
                        self.line(format!("{}if {} {{ // y: #{name}", if k == 0 { "" } else { "} else " }, x.code));
                        let mut ns = before.clone();
                        ns.extend(narrowings(t, true));
                        self.branch_block_narrowed(name, &c.nodes("body"), tail, &ns);
                        let shown = narrowings(t, false);
                        self.bind_narrowed_aliases(&shown);
                        before.extend(shown);
                    }
                    if let Some(e) = els {
                        let name = e.str("name").unwrap();
                        self.line(format!("}} else {{ // y: #{name}"));
                        self.branch_block_narrowed(name, &e.nodes("body"), tail, &before);
                    }
                    self.pop_scope();
                    self.line("}");
                    return;
                }
                self.line(format!("match {} {{", s.code));
                self.ind += 1;
                self.push_scope();
                self.bind(label, s.ty.clone(), Form::Place);
                for c in &conds {
                    let t = c.node("test").unwrap();
                    let name = c.str("name").unwrap();
                    let is_case = |o: &Node| o.kind == "binary" && o.str("op") == Some("==") && name_of(o.node("left").unwrap()) == Some(label);
                    let ops = or_operands(t);
                    if ops.len() > 1 && ops.iter().all(|o| is_case(o)) {
                        // `condition arrow.up = ((arrow == key.up) or (arrow == key.w)):`: an arm for each operand, named
                        // by its place
                        for (k, o) in ops.iter().enumerate() {
                            let pat = self.expr(o.node("right").unwrap()).code;
                            self.arm(&pat, &format!("{name}'{}", k + 1), name, &c.nodes("body"), tail);
                        }
                        continue;
                    }
                    self.push_scope();
                    let fielded = if is_case(t) { self.case_pattern(t.node("right").unwrap(), label, &c.nodes("body"), if matches!(s.form, Form::Ref | Form::Mut) { Form::Ref } else { Form::Place }) } else { None };
                    let pat = if let Some(p) = fielded {
                        p
                    } else if is_case(t) {
                        self.expr(t.node("right").unwrap()).code
                    } else if guarded(t) {
                        // `(shown == outcome.playing) and p`: the case, with `p` as the arm's guard
                        let case = self.expr(t.node("left").unwrap().node("right").unwrap()).code;
                        let g = self.expr(t.node("right").unwrap()).code;
                        format!("{case} if {g}")
                    } else {
                        let x = self.expr(t);
                        format!("_ if {}", x.code)
                    };
                    self.arm(&pat, name, name, &c.nodes("body"), tail);
                    self.pop_scope();
                }
                if let Some(e) = els {
                    let name = e.str("name").unwrap();
                    self.arm("_", name, name, &e.nodes("body"), tail);
                }
                self.pop_scope();
                self.ind -= 1;
                self.line("}");
            }
        }
    }

    /// `compare label = a with b:` with `condition label.x = equal:` (or `less:`, `greater:`): a `match` on the order
    /// of the two, an arm per `Ordering` (Y § 3.5).
    fn compare_stmt(&mut self, st: &Node, tail: bool) {
        let a = self.expr(st.node("left").unwrap());
        let b = self.expr_want(st.node("right").unwrap(), &a.ty);
        let b = self.by_ref(b.clone(), &b.ty.clone());
        self.use_("std::cmp::Ordering");
        self.line(format!("match {}.cmp({b}) {{", wrap(&a, ATOM)));
        self.ind += 1;
        let mut seen = vec![];
        for c in st.nodes("conditions") {
            let test = c.str("test").or_else(|| c.node("test").and_then(|t| t.str("name"))).unwrap_or("");
            let order = match test {
                "less" => "Less",
                "greater" => "Greater",
                _ => "Equal",
            };
            seen.push(order);
            let name = c.str("name").unwrap();
            self.arm(&format!("Ordering::{order}"), name, name, &c.nodes("body"), tail);
        }
        if let Some(e) = st.node("else") {
            let name = e.str("name").unwrap();
            let rest: Vec<&str> = ["Less", "Equal", "Greater"].into_iter().filter(|o| !seen.contains(o)).collect();
            let pat = rest.iter().map(|o| format!("Ordering::{o}")).collect::<Vec<_>>().join(" | ");
            self.arm(&pat, name, name, &e.nodes("body"), tail);
        }
        self.ind -= 1;
        self.line("}");
    }

    fn loop_(&mut self, st: &Node) {
        let name = st.str("name").unwrap();
        let els = st.node("else");
        let ename = els.and_then(|e| e.str("name"));
        let ebody = els.map(|e| e.nodes("body")).unwrap_or_default();
        let ran = !ebody.is_empty();
        if ran {
            self.line("let ran = false;");
        }
        let bare_else = match (ename, ran) {
            (Some(e), false) => format!(", else #{e}"),
            _ => String::new(),
        };
        self.push_scope();
        let head = if st.kind == "while" {
            let c = self.expr(st.node("condition").unwrap());
            format!("while {}", c.code)
        } else {
            let it = st.node("iterable").unwrap();
            let var = name.to_string();
            if it.kind == "call" && name_of(it.node("callee").unwrap()) == Some("range") {
                let args = self.args(it);
                let get = |k: &str| args.iter().find(|(n, _)| *n == Some(k)).map(|(_, a)| *a);
                let from = get("from").map(|a| self.expr(a)).unwrap();
                let (to, op) = match get("through") {
                    Some(t) => (self.expr_want(t, &Ty::U32), "..="),
                    None => (self.expr_want(get("below").unwrap(), &Ty::U32), ".."),
                };
                let ty = if to.ty == Ty::Int { from.ty.clone() } else { to.ty.clone() };
                self.bind(&var, ty, Form::Place);
                format!("for {var} in {}{op}{}", from.code, to.code)
            } else if let Some(TypeDef::Enumeration { .. }) = name_of(it).filter(|n| self.var(n).is_none()).and_then(|n| self.type_def(n)) {
                // `for level in severity`: each member, in order (Y § 3.2)
                let en = name_of(it).unwrap().to_string();
                self.bind(&var, Ty::Named(en.clone()), Form::Place);
                format!("for {var} in {}::ALL", pascal(&en))
            } else if it.kind == "list" {
                let items: Vec<String> = it.nodes("items").iter().map(|i| self.expr(i).code).collect();
                let elem = it.nodes("items").first().map(|i| self.expr(i).ty).unwrap_or(Ty::Unknown);
                self.bind(&var, elem, Form::Place);
                format!("for {var} in [{}]", items.join(", "))
            } else if let Ty::Iter(elem) = self.expr(it).ty {
                let x = self.expr(it);
                self.bind(&var, (*elem).clone(), Form::Place);
                format!("for {var} in {}", x.code)
            } else {
                let x = self.expr(it);
                let elem = x.ty.elem();
                let r = self.by_ref(x.clone(), &x.ty.clone());
                if self.is_copy(&elem) {
                    self.bind(&var, elem, Form::Place);
                    format!("for &{var} in {r}")
                } else {
                    self.bind(&var, elem, Form::Ref);
                    format!("for {var} in {r}")
                }
            }
        };
        self.line(format!("{head} {{ // y: #{name}{bare_else}"));
        self.loops.push(st.clone());
        self.ctx.branches.push(name.to_string());
        self.ind += 1;
        if ran {
            self.line("ran = true;");
        }
        // what a `while`'s condition shows holds in its body (Y § 3.3)
        let shown = if st.kind == "while" { narrowings(st.node("condition").unwrap(), true) } else { vec![] };
        self.push_scope();
        self.bind_narrowed_lets(&shown, &st.nodes("body"));
        self.block(&st.nodes("body"), false);
        self.pop_scope();
        self.loops.pop();
        self.ind -= 1;
        self.ctx.branches.pop();
        self.pop_scope();
        self.line("}");
        if ran {
            let e = ename.unwrap();
            self.line(format!("if !ran {{ // y: #{e}"));
            self.branch_block(e, &ebody, false);
            self.line("}");
        }
    }
}

fn is_not_reachable(st: &Node) -> bool {
    st.kind == "fail" && st.node("value").and_then(name_of).is_some_and(|n| n == "not_reachable" || n == "not_implemented")
}

/// An expression of a structure's members, with each field's name read from the structure: `misses.count()` is
/// `self.misses.count()`.
pub fn with_fields(e: &Node, fields: &[String], obj: &str) -> Node {
    fn go(n: &Node, fields: &[String], obj: &Node) -> Node {
        if n.kind == "name" && fields.iter().any(|f| Some(f.as_str()) == n.str("name")) {
            return Node::new("member", n.line).with("object", obj.clone()).with("name", n.str("name").unwrap());
        }
        let mut out = n.clone();
        for (k, v) in n.fields.iter().enumerate() {
            let nv = match v.1.clone() {
                y_parse::Value::Node(c) if !(n.kind == "member" && v.0 == "name") => y_parse::Value::Node(go(&c, fields, obj)),
                y_parse::Value::List(vs) => y_parse::Value::List(vs.into_iter().map(|x| if let y_parse::Value::Node(c) = x { y_parse::Value::Node(go(&c, fields, obj)) } else { x }).collect()),
                other => other,
            };
            out.fields[k].1 = nv;
        }
        out
    }
    let objn = obj.split('.').skip(1).fold(Node::new("name", e.line).with("name", obj.split('.').next().unwrap()), |o, p| Node::new("member", e.line).with("object", o).with("name", p));
    go(e, fields, &objn)
}

/// An expression of Y text, from the parser: the value of `return r = <text>` in a function of its own.
pub fn parse_expression(text: &str) -> Node {
    let src = format!("module x\n\n\nfunction f() returns r:\n    return r = {text}\n");
    y_parse::parse(&src)
        .ok()
        .and_then(|t| t.nodes("decls").into_iter().find(|d| d.kind == "function").and_then(|f| f.nodes("body").first().and_then(|r| r.node("value").cloned())))
        .unwrap_or_else(|| Node::new("name", 0).with("name", text))
}

/// The name a place starts from: `words` of `words[chosen]`.
fn root_name(e: &Node) -> Option<String> {
    match e.kind {
        "name" => e.str("name").map(str::to_string),
        "member" | "index" => root_name(e.node("object")?),
        _ => None,
    }
}

fn paren(code: &str, prec: u8, min: u8) -> String {
    if prec < min {
        format!("({code})")
    } else {
        code.to_string()
    }
}

fn name_like(code: &str) -> bool {
    code.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn or_operands(e: &Node) -> Vec<&Node> {
    if e.kind == "binary" && e.str("op") == Some("or") {
        let mut v = or_operands(e.node("left").unwrap());
        v.extend(or_operands(e.node("right").unwrap()));
        v
    } else {
        vec![e]
    }
}

/// Two expressions that read the same.
fn same(a: &Node, b: &Node) -> bool {
    y_text(a) == y_text(b)
}

/// A receiver's Rust code as the Y it was lowered from, near enough for a diagnostic: `game.misses` of `game.misses`.
fn y_text_of(code: &str) -> String {
    code.trim_start_matches('&').trim_start_matches("mut ").replace("()", "").to_string()
}

/// An expression as Y text, for comments.
pub fn y_text(e: &Node) -> String {
    match e.kind {
        "name" => e.str("name").unwrap_or("").to_string(),
        "member" => format!("{}.{}", y_text(e.node("object").unwrap()), e.str("name").unwrap_or("")),
        "call" => {
            let args: Vec<String> = e.nodes("args").iter().map(|a| match a.str("name") {
                Some(n) => format!("{n}: {}", y_text(a.node("value").unwrap())),
                None => y_text(a.node("value").unwrap()),
            }).collect();
            format!("{}({})", y_text(e.node("callee").unwrap()), args.join(", "))
        }
        "literal" => e.str("text").unwrap_or("").to_string(),
        "binary" => format!("{} {} {}", y_text(e.node("left").unwrap()), e.str("op").unwrap_or(""), y_text(e.node("right").unwrap())),
        "index" => format!("{}[{}]", y_text(e.node("object").unwrap()), y_text(e.node("index").unwrap())),
        _ => String::new(),
    }
}

/// The type of a constant from its declared type and its value: a list is an array of its length.
pub fn const_type(ty: Option<&Ty>, value: &Node) -> Ty {
    match (ty, value.kind) {
        (Some(Ty::List(e)), "list") => {
            let inner = value.nodes("items").first().map(|i| const_type(Some(e), i)).unwrap_or((**e).clone());
            Ty::Array(Box::new(inner), value.nodes("items").len())
        }
        (Some(t), _) if *t != Ty::Str => t.clone(),
        (_, "literal") if matches!(value.str("type"), Some("string" | "url" | "path")) => Ty::StrLit,
        (Some(t), _) => t.clone(),
        (None, "literal") if value.str("type") == Some("integer") => Ty::Int,
        _ => Ty::Unknown,
    }
}


//! The lowering of what Warehouse adds: schemas and their migrations, decimals and bounded integers, shared state
//! changed in atomic regions (with `orelse` and `waits until`), tasks under supervision, regions that hold off
//! cancellation or run detached, arenas, and resources held until the function ends.
use crate::lower::{y_text, Ex, Form, Lowerer};
use crate::ty::{digits, pascal, Ty};
use y_parse::Node;

const ATOM: u8 = 100;

fn ex(code: impl Into<String>, ty: Ty, form: Form, prec: u8) -> Ex {
    Ex { code: code.into(), ty, form, prec }
}

/// A decimal's Rust type arguments: `i64, 12, 2`.
pub fn decimal_args(p: u32, s: u32) -> String {
    format!("{}, {p}, {s}", digits(p))
}

impl<'a> Lowerer<'a> {
    /// `impl Schema` for a schema's latest version: its key, how a record is written, and how a record of each version
    /// is read, an earlier one migrated as its `migrate` says.
    pub fn schema_impl(&mut self, name: &str) {
        let m = self.m;
        let schema = m.schemas[name].clone();
        let en = pascal(name);
        let (latest, fields) = schema.versions.iter().max_by_key(|v| v.0).cloned().unwrap();
        let key = schema.key.clone();
        let others: Vec<(String, Ty)> = fields.iter().filter(|f| f.0 != key).cloned().collect();
        self.line("");
        self.line(format!("/// How a `{name}` is stored: by its `{key}`, its other fields separated by `;`, and each earlier version read and migrated."));
        self.line(format!("impl Schema for {en} {{"));
        self.ind += 1;
        self.line(format!("const NAME: &'static str = \"{name}\";"));
        self.line(format!("const VERSION: u32 = {latest};"));
        self.line("");
        self.line("fn key(&self) -> String {");
        self.line(format!("    self.{key}.to_string()"));
        self.line("}");
        self.line("");
        self.line("fn encode(&self) -> String {");
        let parts: Vec<String> = others
            .iter()
            .map(|(f, t)| match m.resolve(t) {
                Ty::Str => format!("self.{f}.as_str().replace('\\n', \"\\\\n\")"),
                _ => format!("self.{f}.to_string()"),
            })
            .collect();
        if parts.len() == 1 {
            self.line(format!("    {}", parts[0]));
        } else {
            self.line(format!("    [{}].join(\";\")", parts.join(", ")));
        }
        self.line("}");
        self.line("");
        self.line("fn decode(key: &str, version: u32, text: &str) -> Result<Self, String> {");
        self.ind += 1;
        let key_ty = fields.iter().find(|f| f.0 == key).map(|f| f.1.clone()).unwrap_or(Ty::Unknown);
        let parse_key = self.parse_field("key", &key_ty, &key);
        self.line(format!("let {key} = {parse_key};"));
        self.line("let fields = text.split(';').collect::<Vec<&str>>();");
        self.line("match (version, fields.as_slice()) {");
        self.ind += 1;
        let mut versions = schema.versions.clone();
        versions.sort_by_key(|v| v.0);
        for (v, vfields) in &versions {
            let vothers: Vec<&(String, Ty)> = vfields.iter().filter(|f| f.0 != key).collect();
            let names: Vec<String> = vothers.iter().map(|f| format!("{}_text", f.0)).collect();
            self.line(format!("({v}, [{}]) => {{", names.join(", ")));
            self.ind += 1;
            for (f, t) in &vothers {
                let p = self.parse_field(&format!("{f}_text"), t, f);
                self.line(format!("let {f} = {p};"));
            }
            // the fields the version does not have, from its migration
            for (f, _) in fields.iter().filter(|f| f.0 != key && !vfields.iter().any(|g| g.0 == f.0)) {
                let value = schema.migrations.iter().filter(|mg| mg.0 == *v).flat_map(|mg| mg.2.iter()).find(|st| st.kind == "assign" && st.node("target").and_then(|t| t.str("name")) == Some(f.as_str())).and_then(|st| st.node("value").cloned());
                let code = match value {
                    Some(val) => {
                        self.line(format!("// migrate {name} v{v} to v{latest}: {f} := {}", y_text(&val)));
                        self.expr(&val).code
                    }
                    None => format!("y_runtime::fault(\"{}::{en}::decode\", \"no migration gives {f}\")", m.name),
                };
                self.line(format!("let {f} = {code};"));
            }
            let all: Vec<String> = fields.iter().map(|f| f.0.clone()).collect();
            self.line(format!("Ok({en} {{ {} }})", all.join(", ")));
            self.ind -= 1;
            self.line("}");
        }
        self.line(format!("_ => Err(format!(\"not a {name} of version {{version}}\")),"));
        self.ind -= 1;
        self.line("}");
        self.ind -= 1;
        self.line("}");
        self.ind -= 1;
        self.line("}");
    }

    /// How a stored field's text is read as its type.
    fn parse_field(&mut self, text: &str, t: &Ty, field: &str) -> String {
        match self.m.resolve(t) {
            Ty::Str => format!("YString::new(&{text}.replace(\"\\\\n\", \"\\n\"))"),
            Ty::Ascii => format!("AsciiString::new({text}).map_err(|_| format!(\"`{{{text}}}` is not a {field}\"))?"),
            Ty::Decimal(p, s) => format!("Decimal::<{}>::parse({text}).ok_or_else(|| format!(\"`{{{text}}}` is not a {field}\"))?", decimal_args(p, s)),
            _ => format!("{text}.parse().map_err(|_| format!(\"`{{{text}}}` is not a {field}\"))?"),
        }
    }

    /// A decimal expression as a decimal of another type: unchanged when it is one already.
    pub fn as_decimal(&mut self, x: &Ex, p: u32, s: u32) -> String {
        match x.ty {
            Ty::Decimal(xp, xs) if xp == p && xs == s => wrap(x),
            _ => format!("{}.to_decimal::<{}>()", wrap(x), decimal_args(p, s)),
        }
    }

    /// Arithmetic of decimals (Y § 3.3): a sum or a difference at the wider of the two types, a product of a decimal and
    /// an integer at the full precision and the decimal's scale, and a product of two decimals at the full precision
    /// and the sum of their scales.
    pub fn decimal_binary(&mut self, op: &str, a: Ex, b: Ex) -> Option<Ex> {
        match (op, &a.ty, &b.ty) {
            ("+" | "-", Ty::Decimal(p1, s1), Ty::Decimal(p2, s2)) => {
                let (p, s) = if (p1, s1) == (p2, s2) { (*p1, *s1) } else { (38, (*s1).max(*s2)) };
                let (l, r) = (self.as_decimal(&a, p, s), self.as_decimal(&b, p, s));
                Some(ex(format!("{l} {op} {r}"), Ty::Decimal(p, s), Form::Temp, 70))
            }
            ("+" | "-", Ty::Decimal(p, s), _) if is_literal_number(&b) => {
                let r = decimal_literal(&b.code, *p, *s);
                Some(ex(format!("{} {op} {r}", a.code), a.ty.clone(), Form::Temp, 70))
            }
            ("*", Ty::Decimal(_, s), Ty::Integer(..)) => {
                let l = self.as_decimal(&a, 38, *s);
                Some(ex(format!("{l}.times({}.to_i128())", wrap(&b)), Ty::Decimal(38, *s), Form::Temp, ATOM))
            }
            ("*", Ty::Integer(..), Ty::Decimal(_, s)) => {
                let r = self.as_decimal(&b, 38, *s);
                Some(ex(format!("{r}.times({}.to_i128())", wrap(&a)), Ty::Decimal(38, *s), Form::Temp, ATOM))
            }
            ("*", Ty::Decimal(_, s1), Ty::Decimal(_, s2)) => {
                let (l, r) = (self.as_decimal(&a, 38, *s1), self.as_decimal(&b, 38, *s2));
                Some(ex(format!("{l}.multiply({r})"), Ty::Decimal(38, s1 + s2), Form::Temp, ATOM))
            }
            ("/", Ty::Rational(p1), Ty::Rational(p2)) => Some(ex(format!("{} / {}", wrap(&a), wrap(&b)), Ty::Rational((*p1).max(*p2)), Form::Temp, 80)),
            _ => None,
        }
    }

    /// The methods of Warehouse's types: decimals, integers, bags, queues, sorted maps, stores, shared state, arenas
    /// and lists' iteration.
    pub fn ware_method(&mut self, recv: &Ex, name: &str, args: &[(Option<&str>, &Node)]) -> Option<Ex> {
        let r = wrap(recv);
        let arg = |k: &str| args.iter().find(|(n, _)| *n == Some(k)).map(|(_, a)| *a);
        let ty = self.module().resolve(&recv.ty);
        Some(match (&ty, name) {
            (Ty::Decimal(p, _), "to_rational") => ex(format!("{r}.to_rational::<Big, UNBOUNDED>()"), Ty::Rational(*p), Form::Temp, ATOM),
            (Ty::Integer(..), "to_rational") => ex(format!("Rational::new({r}.to_i128(), 1)"), Ty::Rational(0), Form::Temp, ATOM),
            (Ty::Decimal(p, _) | Ty::Rational(p), "to_decimal_rounded") => {
                let s: u32 = arg("scale").and_then(|x| x.str("text")).and_then(|t| t.parse().ok()).unwrap_or(0);
                let (p, s) = match &self.want_decimal {
                    Some((wp, ws)) if *ws == s => (*wp, *ws),
                    _ => (*p, s),
                };
                let mode = arg("rounding").and_then(|x| x.str("name")).map(pascal).unwrap_or_else(|| "HalfEven".into());
                let from = if let Ty::Decimal(..) = ty { self.as_decimal(recv, 38, max_scale(&ty)) } else { r.clone() };
                ex(format!("{from}.to_decimal_rounded::<{}>(Rounding::{mode})", decimal_args(p, s)), Ty::Decimal(p, s), Form::Temp, ATOM)
            }
            (Ty::Count | Ty::U32 | Ty::I32, "divide_floor") => {
                let n = self.expr_want(args[0].1, &ty).code;
                ex(format!("{r}.div_euclid({n})"), ty.clone(), Form::Temp, ATOM)
            }
            (Ty::Integer(..), "to_usize") => ex(format!("{r}.to_usize()"), Ty::Count, Form::Temp, ATOM),
            (Ty::Bag(e), "count_of") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.by_ref(v, &e);
                ex(format!("{r}.count_of({v})"), Ty::Count, Form::Temp, ATOM)
            }
            (Ty::Bag(e), "remove") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.by_ref(v, &e);
                let n = self.count_arg(arg("times"));
                ex(format!("{r}.remove_times({v}, {n})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::Bag(e), "add") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.owned(v, &e);
                match arg("times") {
                    Some(t) => {
                        let n = self.count_arg(Some(t));
                        ex(format!("{r}.add_times({v}, {n})"), Ty::Unit, Form::Temp, ATOM)
                    }
                    None => ex(format!("{r}.add({v})"), Ty::Unit, Form::Temp, ATOM),
                }
            }
            (Ty::Queue(e), "add") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.owned(v, &e);
                ex(format!("{r}.add({v})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::Bag(_) | Ty::Queue(_) | Ty::SortedMap(..), "count") => ex(format!("{r}.count()"), Ty::Count, Form::Temp, ATOM),
            (Ty::SortedMap(k, v), "set") => {
                let (k, v) = ((**k).clone(), (**v).clone());
                let key = self.expr_want(args[0].1, &k);
                let val = self.expr_want(args[1].1, &v);
                // the key read from the value is copied before the value is given
                let key = if key.code.starts_with(&format!("{}.", val.code)) { format!("{}.clone()", key.code) } else { self.owned(key, &k) };
                let val = self.owned(val, &v);
                ex(format!("{r}.set({key}, {val})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::SortedMap(k, v), "get") => {
                let k = (**k).clone();
                let key = self.expr_want(args[0].1, &k);
                let key = self.by_ref(key, &k);
                ex(format!("{r}.get({key}).cloned().unwrap_or_else(|| y_runtime::fault(\"{}::{}\", \"no such key\"))", self.ctx.module, self.ctx.name), (**v).clone(), Form::Temp, ATOM)
            }
            (Ty::Store(e), "read_all") => ex(format!("{r}.read_all()"), Ty::Iter(Box::new(Ty::Res(e.clone(), Box::new(Ty::Named("record_failure".into()))))), Form::Temp, ATOM),
            (Ty::Store(_), "lines") => ex(format!("{r}.lines()"), Ty::List(Box::new(Ty::HostStr)), Form::Temp, ATOM),
            (Ty::Store(e), "batch") => ex(format!("Held::new({r}.batch())"), Ty::Named(format!("batch:{}", match &**e { Ty::Named(n) => n.clone(), _ => String::new() })), Form::Temp, ATOM),
            (Ty::Named(b), "put") if b.starts_with("batch:") => {
                let t = Ty::Named(b.trim_start_matches("batch:").to_string());
                let v = self.expr_want(args[0].1, &t);
                let v = self.owned(v, &t);
                ex(format!("{r}.get().put({v})"), Ty::Unit, Form::Temp, ATOM)
            }
            (Ty::Shared(e), "snapshot") => ex(format!("{r}.snapshot()"), (**e).clone(), Form::Temp, ATOM),
            (Ty::Arena(e), "allocate") => {
                let e = (**e).clone();
                let v = self.expr_want(args[0].1, &e);
                let v = self.owned(v, &e);
                ex(format!("{r}.allocate({v})"), Ty::Index(Box::new(e)), Form::Temp, ATOM)
            }
            (Ty::Arena(e), "rendered") => {
                self.rendered.insert(match &**e { Ty::Named(n) => n.clone(), _ => String::new() });
                ex(format!("rendered(&{r})"), Ty::Str, Form::Temp, ATOM)
            }
            // a list's iteration: `map`, `filter`, `flatten`, `sum`, `take`, `skip`
            (Ty::List(e), "map") | (Ty::Iter(e), "map") => {
                let f = args[0].1.str("name").unwrap_or("").to_string();
                let out = self.function(&f).and_then(|g| g.result.as_ref().map(|r| r.1.clone())).unwrap_or(Ty::Unknown);
                let _ = e;
                let it = if matches!(ty, Ty::List(_)) { format!("{r}.iter()") } else { r.clone() };
                ex(format!("{it}.map({f})"), Ty::Iter(Box::new(out)), Form::Temp, ATOM)
            }
            (Ty::List(e), "filter") | (Ty::Iter(e), "filter") => {
                let f = args[0].1.str("name").unwrap_or("").to_string();
                let it = if matches!(ty, Ty::List(_)) { format!("{r}.iter()") } else { r.clone() };
                ex(format!("{it}.filter(|item| {f}(item))"), Ty::Iter(e.clone()), Form::Temp, ATOM)
            }
            (Ty::Iter(e), "flatten") => {
                let inner = e.elem();
                let code = r.replacen(".map(", ".flat_map(", 1);
                let t = Ty::List(Box::new(inner));
                let rt = self.rust_type(&t);
                ex(format!("{code}.collect::<{rt}>()"), t, Form::Temp, ATOM)
            }
            (Ty::Iter(e), "sum") => {
                let zero = match &**e {
                    Ty::Decimal(p, s) => format!("Decimal::<{}>::zero()", decimal_args(*p, *s)),
                    _ => "0".into(),
                };
                ex(format!("{r}.fold({zero}, |a, b| a + b)"), (**e).clone(), Form::Temp, ATOM)
            }
            (Ty::List(e), "take" | "skip") => {
                let n = self.expr(args[0].1).code;
                let t = Ty::List(e.clone());
                let rt = self.rust_type(&t);
                ex(format!("{r}.iter().{name}({n}).cloned().collect::<{rt}>()"), t, Form::Temp, ATOM)
            }
            _ => return None,
        })
    }

    /// A count from an integer the bounds of which Y knows: `wanted.quantity` as a `usize`.
    fn count_arg(&mut self, a: Option<&Node>) -> String {
        let Some(a) = a else { return "1".into() };
        let x = self.expr(a);
        match x.ty {
            Ty::Integer(..) => format!("{}.to_usize()", wrap(&x)),
            _ => x.code,
        }
    }

    /// A conversion to a type the runtime has a constructor for: a decimal from its literal or another decimal, an
    /// integer from a whole number, text with a character set from a string.
    pub fn ware_owned(&mut self, x: &Ex, want: &Ty) -> Option<String> {
        let want = self.module().resolve(want);
        Some(match (&want, &x.ty) {
            (Ty::Decimal(p, s), _) if is_literal_number(x) => {
                if self.in_group.is_some() {
                    // a constant: its units, as `of_units` is a constant function
                    let digits_only: String = x.code.chars().filter(|c| c.is_ascii_digit()).collect();
                    format!("Decimal::<{}>::of_units({})", decimal_args(*p, *s), digits_only.trim_start_matches('0').parse::<i128>().unwrap_or(0))
                } else {
                    decimal_literal(&x.code, *p, *s)
                }
            }
            (Ty::Decimal(p, s), Ty::Decimal(..)) => self.as_decimal(x, *p, *s),
            (Ty::Integer(..), Ty::Int) => format!("Integer::from_i128({})", x.code),
            (Ty::Ascii, Ty::StrLit | Ty::HostStr | Ty::Unknown) => {
                let text = if x.ty == Ty::StrLit { x.code.clone() } else { format!("&{}", x.code) };
                format!("AsciiString::new({text}).unwrap_or_else(|_| y_runtime::fault(\"{}::{}\", \"text of the character set\"))", self.ctx.module, self.ctx.name)
            }
            (Ty::U32, Ty::Integer(..)) => format!("{}.to_u32()", wrap(x)),
            (Ty::Count, Ty::Integer(..)) => format!("{}.to_usize()", wrap(x)),
            _ => return None,
        })
    }

    /// An atomic region over shared state (Y § 3.8), its statements in a closure over a transaction: each shared name
    /// it reads is read once at its start, and written back at its end when it changes.
    pub fn atomic_closure(&mut self, st: &Node, result: &Ty) -> Vec<String> {
        let body = st.nodes("body");
        let shared = self.shared_names(&body, st);
        let mut out = vec![];
        let changed = |n: &str| {
            let mut c = false;
            for b in &body {
                b.walk(&mut |x: &Node| {
                    if x.kind == "call" {
                        if let Some(callee) = x.node("callee").filter(|c| c.kind == "member") {
                            let t = y_text(callee.node("object").unwrap());
                            if (t == n || t.starts_with(&format!("{n}."))) && ["remove", "add", "set", "push", "insert"].contains(&callee.str("name").unwrap_or("")) {
                                c = true;
                            }
                        }
                    }
                    if x.kind == "assign" && y_text(x.node("target").unwrap()).starts_with(n) {
                        c = true;
                    }
                });
            }
            c
        };
        let saved = self.out.len();
        self.ind += 1;
        self.push_scope();
        for (n, t) in &shared {
            let local = format!("held_{n}");
            self.line(format!("let {local} = tx.get({n});"));
            self.bind_alias(n, t.clone(), &local);
        }
        for c in st.nodes("constraints").iter().filter(|c| c.str("word") == Some("waits")) {
            let name = c.str("name").unwrap_or("");
            self.line(format!("// y: waits until {name}"));
            let cond = self.expr(c.node("condition").unwrap());
            self.line(format!("if !({}) {{", cond.code));
            self.line("    return Ok(Step::Retry);");
            self.line("}");
        }
        // the statements; a `return` gives the region's value
        self.atomic_result = Some(shared.iter().filter(|(n, _)| changed(n)).map(|(n, _)| n.clone()).collect());
        let n = body.len();
        for (k, s) in body.iter().enumerate() {
            self.stmt_tail(s, k + 1 == n);
        }
        if !body.last().is_some_and(|s| s.kind == "return") {
            for n in self.atomic_result.clone().unwrap_or_default() {
                self.line(format!("tx.set({n}, held_{n});"));
            }
            let _ = n;
            self.line("Ok(Step::Done(()))");
        }
        self.atomic_result = None;
        self.pop_scope();
        self.ind -= 1;
        out.extend(self.out.drain(saved..));
        let _ = result;
        out
    }

    /// The shared names a region's statements read: the injected parameters of type `shared<…>`.
    fn shared_names(&self, body: &[&Node], st: &Node) -> Vec<(String, Ty)> {
        let mut names = vec![];
        let mut see = |n: &Node| {
            n.walk(&mut |x: &Node| {
                if x.kind == "name" {
                    let nm = x.str("name").unwrap();
                    if let Some(v) = self.var(nm) {
                        if let Ty::Shared(t) = &v.ty {
                            if !names.iter().any(|(m, _): &(String, Ty)| m == nm) {
                                names.push((nm.to_string(), (**t).clone()));
                            }
                        }
                    }
                }
            });
        };
        for c in st.nodes("constraints") {
            see(c);
        }
        for b in body {
            see(b);
        }
        names
    }
}

fn wrap(e: &Ex) -> String {
    if e.prec < ATOM {
        format!("({})", e.code)
    } else {
        e.code.clone()
    }
}

fn max_scale(t: &Ty) -> u32 {
    match t {
        Ty::Decimal(_, s) => *s,
        _ => 0,
    }
}

/// Whether an expression is a number as written: `4.50`, `3`.
pub fn is_literal_number(x: &Ex) -> bool {
    !x.code.is_empty() && x.code.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && matches!(x.ty, Ty::Int | Ty::Unknown | Ty::Decimal(..))
}

/// A decimal literal of a type: `Decimal::<i64, 12, 2>::literal("4.50")`.
pub fn decimal_literal(text: &str, p: u32, s: u32) -> String {
    format!("Decimal::<{}>::literal(\"{text}\")", decimal_args(p, s))
}

impl<'a> Lowerer<'a> {
    /// The Rust type of what an atomic region gives: the function's result when the region returns it.
    fn region_result(&mut self, alternatives: &[&Node]) -> (Ty, String) {
        let returns = alternatives.iter().any(|a| a.nodes("body").iter().any(|s| s.kind == "return"));
        if returns {
            let t = self.ctx.result.clone().unwrap_or(Ty::Unit);
            let r = self.rust_type(&t);
            (t, r)
        } else {
            (Ty::Unit, "()".into())
        }
    }

    /// `region name is atomic …:` and its `orelse region`s: a closure over a transaction for each alternative, run by
    /// `atomic`, `atomic_with` (an isolation level) or `orelse`; the regions' value is the function's when they return it.
    pub fn region_atomic(&mut self, st: &Node, tail: bool) {
        let mut alternatives = vec![st];
        alternatives.extend(st.nodes("orelse"));
        let (rt, rust) = self.region_result(&alternatives);
        let label = format!("{}::{}", self.ctx.module, self.ctx.name);
        let returns = rt != Ty::Unit;
        for a in &alternatives {
            let cs: Vec<String> = a.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
            self.regions.push((self.ctx.name.clone(), a.str("name").unwrap().to_string(), a.line, cs));
        }
        let result_name = self.function(&self.ctx.name.clone()).and_then(|f| f.result.as_ref().map(|r| r.0.clone())).unwrap_or_else(|| "value".into());
        if alternatives.len() > 1 {
            // `orelse`: the alternatives in order, the first that does not wait commits
            let mut names = vec![];
            for a in &alternatives {
                let name = a.str("name").unwrap();
                self.line(format!("let {name} = |tx: &mut Tx| -> Result<Step<{rust}>, Infallible> {{ // y: #{name}"));
                let body = self.atomic_closure(a, &rt);
                self.out.extend(body);
                self.line("};");
                names.push(format!("&mut {name}"));
            }
            let last = alternatives.last().unwrap().str("name").unwrap();
            let call = format!("orelse(\"{label}\", None, vec![{}])", names.join(", "));
            self.finish_atomic(&call, returns, tail, &result_name, &format!("{label}#{last}"));
            return;
        }
        let name = st.str("name").unwrap();
        let cs: Vec<String> = st.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
        self.line(format!("// y: region {name} is {}", cs.join(", ")));
        let isolation = st.nodes("constraints").iter().find(|c| c.str("word") == Some("isolation")).and_then(|c| c.str("level")).map(pascal);
        let head = match &isolation {
            Some(level) => format!("let {name} = atomic_with(\"{label}#{name}\", None, Isolation::{level}, |tx| -> Result<Step<{rust}>, Infallible> {{"),
            None => format!("let {name} = atomic(\"{label}#{name}\", None, |tx| -> Result<Step<{rust}>, Infallible> {{"),
        };
        self.line(head);
        let body = self.atomic_closure(st, &rt);
        self.out.extend(body);
        self.line("});");
        self.finish_atomic(name, returns, tail, &result_name, &format!("{label}#{name}"));
    }

    /// What an atomic region's runner gives: its value, or, when it was aborted, a fault, as nothing aborts it.
    fn finish_atomic(&mut self, call: &str, returns: bool, tail: bool, result_name: &str, label: &str) {
        if returns {
            let lead = if tail { "" } else { "return " };
            self.line(format!("{lead}match {call} {{"));
            self.line(format!("    Ok({result_name}) => {result_name},"));
            self.line(format!("    Err(_) => not_reachable(\"{label}\"),"));
            self.line(if tail { "}" } else { "};" });
        } else {
            self.line(format!("if {call}.is_err() {{"));
            self.line(format!("    not_reachable(\"{label}\");"));
            self.line("}");
        }
    }

    /// `region name is within d or fail, supervise(…):` holding `task` statements: each task a closure, run under
    /// supervision, and the deadline checked when they have all ended.
    pub fn region_tasks(&mut self, st: &Node) {
        let name = st.str("name").unwrap();
        let cs: Vec<String> = st.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
        self.regions.push((self.ctx.name.clone(), name.to_string(), st.line, cs.clone()));
        self.line(format!("// y: region {name} is {}", cs.join(", ")));
        let within = st.nodes("constraints").iter().find(|c| c.str("word") == Some("within")).and_then(|c| c.node("bound")).cloned();
        if let Some(b) = &within {
            let b = self.expr(b).code;
            self.line(format!("let deadline = Deadline::after({b});"));
        }
        let mut tasks = vec![];
        for t in st.nodes("body").into_iter().filter(|t| t.kind == "task") {
            let tn = t.str("name").unwrap();
            let call = self.expr(t.node("value").unwrap()).code;
            self.line(format!("let {tn}: TaskBody<'_, Infallible> = Box::new(|_| {{"));
            self.line(format!("    {call};"));
            self.line("    Ok(())");
            self.line("});");
            tasks.push(tn.to_string());
        }
        let sup = st.nodes("constraints").into_iter().find(|c| c.str("word") == Some("supervise"));
        let (mut restart, mut at_most) = ("EachFailed".to_string(), "0".to_string());
        if let Some(s) = sup {
            for a in s.nodes("args") {
                match a.str("name") {
                    Some("restart") => restart = a.node("value").and_then(|v| v.str("name")).map(pascal).unwrap_or(restart),
                    Some("at_most") => at_most = a.node("value").and_then(|v| v.str("text")).unwrap_or("0").to_string(),
                    _ => {}
                }
            }
        }
        self.line(format!("let policy = Supervision {{ restart: Restart::{restart}, at_most: {at_most} }};"));
        self.line(format!("if let Err(failed) = supervised(policy, vec![{}]) {{", tasks.join(", ")));
        self.line("    match failed.failure {}");
        self.line("}");
        if within.is_some() {
            // the function's failure for a deadline that passed: the case that carries an `expired`
            let case = self.ctx.fails.clone().and_then(|f| match f {
                Ty::Named(n) => match self.module().types.get(&n).map(|t| &t.def) {
                    Some(crate::model::TypeDef::Enum { cases }) => cases.iter().find(|(_, fs)| matches!(&fs[..], [(_, Ty::Named(t))] if t == "expired")).map(|(c, fs)| (pascal(&n), pascal(c), fs[0].0.clone())),
                    _ => None,
                },
                _ => None,
            });
            if let Some((en, c, f)) = case {
                self.line(format!("deadline.remaining().map_err(|{f}| {en}::{c} {{ {f} }})?;"));
            }
        }
    }

    /// `region name is uncancellable, within d:`: the statements in a closure that cancellation is held off from.
    pub fn region_uncancellable(&mut self, st: &Node) {
        let name = st.str("name").unwrap();
        let cs: Vec<String> = st.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
        self.regions.push((self.ctx.name.clone(), name.to_string(), st.line, cs.clone()));
        let bound = st.nodes("constraints").iter().find(|c| c.str("word") == Some("within")).and_then(|c| c.node("bound")).map(|b| self.expr(b).code).unwrap_or_else(|| "Duration::MAX".into());
        self.line(format!("// y: region {name} is {}", cs.join(", ")));
        self.line(format!("Cancellation::new().held(\"{}::{}#{name}\", {bound}, || {{", self.ctx.module, self.ctx.name));
        self.ind += 1;
        self.block(&st.nodes("body"), false);
        self.ind -= 1;
        self.line("});");
        self.line(format!("// y: end region {name}"));
    }

    /// `region name is detach because "…", within d:`: the statements on a task of their own, which may outlive the
    /// function; it is waited for, no longer than its bound, before the function ends.
    pub fn region_detach(&mut self, st: &Node) {
        let name = st.str("name").unwrap();
        let cs: Vec<String> = st.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
        self.regions.push((self.ctx.name.clone(), name.to_string(), st.line, cs.clone()));
        let bound = st.nodes("constraints").iter().find(|c| c.str("word") == Some("within")).and_then(|c| c.node("bound")).map(|b| self.expr(b).code).unwrap_or_else(|| "Duration::MAX".into());
        // what its `or fail`s fail with: the console's failure, the one the examples' detached regions have
        let fails = Ty::Named("console_error".into());
        let ft = self.rust_type(&fails);
        self.line(format!("// y: region {name} is {}", cs.join(", ")));
        self.line(format!("let {name} = detach({bound}, move || -> Result<(), {ft}> {{"));
        self.ind += 1;
        let saved = self.ctx.fails.replace(fails);
        self.block(&st.nodes("body"), false);
        self.ctx.fails = saved;
        self.line("Ok(())");
        self.ind -= 1;
        self.line("});");
        self.line(format!("// y: end region {name}"));
        self.line(format!("if let Ok(done) = {name}.wait() {{"));
        self.line("    done?;");
        self.line("}");
    }

    /// `arena name:`: an arena made where the block starts, released when the function ends.
    pub fn arena(&mut self, st: &Node, tail: bool) {
        let name = st.str("name").unwrap();
        let mut elem = Ty::Unknown;
        for b in st.nodes("body") {
            b.walk(&mut |n: &Node| {
                if elem == Ty::Unknown && n.kind == "call" {
                    let c = n.node("callee").unwrap();
                    if c.kind == "member" && c.str("name") == Some("allocate") && c.node("object").and_then(|o| o.str("name")) == Some(name) {
                        if let Some(t) = n.nodes("args").first().and_then(|a| a.node("value")).and_then(|v| v.node("callee")).and_then(|c| c.str("name")) {
                            elem = Ty::Named(t.to_string());
                        }
                    }
                }
            });
        }
        self.line(format!("// y: arena {name}"));
        self.line(format!("let {name} = Arena::new(\"{name}\");"));
        self.bind(name, Ty::Arena(Box::new(elem)), Form::Place);
        let body = st.nodes("body");
        self.block_open_tail_or(&body, tail);
    }

    /// The function that reads an arena of entries as text, which `rendered()` names: each entry on a line of its own,
    /// an entry under another indented by two spaces.
    pub fn rendered_helper(&mut self, elem: &str) {
        let en = pascal(elem);
        self.line("");
        self.line(format!("/// What an arena of `{elem}` entries reads as: each on a line of its own, one under another indented by two spaces."));
        self.line(format!("fn rendered(scratch: &Arena<{en}>) -> YString {{"));
        self.line("    let lines = scratch.iter().map(|e| if e.under.is_some() { format!(\"  {}\", e.text) } else { e.text.to_string() }).collect::<Vec<String>>();");
        self.line("    YString::from(lines.join(\"\\n\"))");
        self.line("}");
    }
}

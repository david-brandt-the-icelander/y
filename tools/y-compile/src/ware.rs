//! The lowering of what Warehouse adds: schemas and their migrations, decimals and bounded integers, shared state
//! changed in atomic regions (with `orelse` and `waits until`), tasks under supervision, regions that hold off
//! cancellation or run detached, arenas, and resources held until the function ends.
use crate::lower::{y_text, Ex, Form, Lowerer, Var};
use crate::ty::{digits, pascal, Ty};
use y_parse::Node;

const ATOM: u8 = 100;

fn ex(code: impl Into<String>, ty: Ty, form: Form, prec: u8) -> Ex {
    Ex { code: code.into(), ty, form, prec }
}

/// A decimal's Rust type arguments: `i64, 12, 2`.
pub fn decimal_args(p: u32, s: u32) -> String {
    if p == crate::ty::UNBOUNDED {
        return format!("Big, UNBOUNDED, {s}");
    }
    format!("{}, {p}, {s}", digits(p))
}

/// The precision a value of a type has, as a decimal operand: a decimal's own, an integer's range's digits.
fn operand_precision(t: &Ty) -> Option<(u32, u32)> {
    match t {
        Ty::Decimal(p, s) => Some((*p, *s)),
        Ty::Integer(lo, hi) => Some((crate::ty::range_precision(*lo, *hi), 0)),
        _ => None,
    }
}

/// The range of `a op b` for integers of two ranges: the extremes of the operands' extremes.
fn integer_range(op: &str, (a0, a1): (i128, i128), (b0, b1): (i128, i128)) -> (i128, i128) {
    let at = |x: i128, y: i128| match op {
        "+" => x.saturating_add(y),
        "-" => x.saturating_sub(y),
        _ => x.saturating_mul(y),
    };
    let all = [at(a0, b0), at(a0, b1), at(a1, b0), at(a1, b1)];
    (*all.iter().min().unwrap(), *all.iter().max().unwrap())
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
            Ty::Decimal(_, xs) if xs > s => {
                // a scale never falls silently (Y § 3.3)
                self.error(format!("a decimal of scale {xs} where one of scale {s} is wanted: `to_decimal_rounded(scale: {s}, rounding: …)` lowers it, as named"));
                wrap(x)
            }
            _ => format!("{}.to_decimal::<{}>()", wrap(x), decimal_args(p, s)),
        }
    }

    /// Arithmetic of decimals, rationals and integers (Y § 3.3), with the result's constraints derived from the
    /// operands', so that the arithmetic itself never overflows: a sum or a difference has the larger scale, and the
    /// precision the larger whole part needs plus that scale and one; a product of two decimals has the sum of their
    /// precisions and the sum of their scales; a decimal times an integer keeps the decimal's scale, and adds the
    /// integer's digits to its precision. Each operand is converted to the result's size first. A rational's `+`, `-`,
    /// `*` and `/` are exact; an integer's range is the extremes of its operands'.
    pub fn decimal_binary(&mut self, op: &str, a: Ex, b: Ex) -> Option<Ex> {
        let lit_int = |x: &Ex| x.ty == Ty::Int && is_literal_number(x) && !x.code.contains('.');
        match (op, &a.ty, &b.ty) {
            ("+" | "-", Ty::Decimal(p1, s1), Ty::Decimal(p2, s2)) => {
                let s = (*s1).max(*s2);
                let p = crate::ty::capped((p1 - s1).max(p2 - s2) + s + 1);
                let (l, r) = (self.as_decimal(&a, p, s), self.as_decimal(&b, p, s));
                Some(ex(format!("{l} {op} {r}"), Ty::Decimal(p, s), Form::Temp, 70))
            }
            ("+" | "-", Ty::Decimal(p, s), _) if is_literal_number(&b) => {
                let rt = crate::ty::capped(p + 1);
                let l = self.as_decimal(&a, rt, *s);
                let r = decimal_literal(&b.code, rt, *s);
                Some(ex(format!("{l} {op} {r}"), Ty::Decimal(rt, *s), Form::Temp, 70))
            }
            ("+" | "-", _, Ty::Decimal(p, s)) if is_literal_number(&a) => {
                let rt = crate::ty::capped(p + 1);
                let l = decimal_literal(&a.code, rt, *s);
                let r = self.as_decimal(&b, rt, *s);
                Some(ex(format!("{l} {op} {r}"), Ty::Decimal(rt, *s), Form::Temp, 70))
            }
            ("*", Ty::Decimal(p, s), Ty::Integer(..)) | ("*", Ty::Integer(..), Ty::Decimal(p, s)) => {
                let (d, i) = if matches!(a.ty, Ty::Decimal(..)) { (&a, &b) } else { (&b, &a) };
                let (q, _) = operand_precision(&i.ty).unwrap();
                let rt = crate::ty::capped(p + q);
                let l = self.as_decimal(d, rt, *s);
                Some(ex(format!("{l}.times({}.to_i128())", wrap(i)), Ty::Decimal(rt, *s), Form::Temp, ATOM))
            }
            ("*", Ty::Decimal(p, s), _) | ("*", _, Ty::Decimal(p, s)) if lit_int(&a) || lit_int(&b) => {
                let (d, n) = if matches!(a.ty, Ty::Decimal(..)) { (&a, &b) } else { (&b, &a) };
                let v: i128 = n.code.parse().unwrap_or(0);
                let rt = crate::ty::capped(p + crate::ty::range_precision(v, v));
                let l = self.as_decimal(d, rt, *s);
                Some(ex(format!("{l}.times({v})"), Ty::Decimal(rt, *s), Form::Temp, ATOM))
            }
            ("*", Ty::Decimal(p1, s1), Ty::Decimal(p2, s2)) => {
                let (p, s) = (crate::ty::capped(p1 + p2), s1 + s2);
                let (l, r) = (self.as_decimal(&a, p, *s1), self.as_decimal(&b, p, *s2));
                let (pt, st) = (if p == crate::ty::UNBOUNDED { "UNBOUNDED".to_string() } else { p.to_string() }, s2);
                Some(ex(format!("{l}.multiply::<{pt}, {st}, {pt}, {s}>({r})"), Ty::Decimal(p, s), Form::Temp, ATOM))
            }
            ("+" | "-" | "*" | "/", Ty::Rational(p1), Ty::Rational(p2)) => {
                let p = if op == "+" || op == "-" { 70 } else { 80 };
                Some(ex(format!("{} {op} {}", wrap_at(&a, p), wrap_at(&b, p + 1)), Ty::Rational((*p1).max(*p2)), Form::Temp, p))
            }
            ("+" | "-" | "*", Ty::Integer(a0, a1), Ty::Integer(b0, b1)) => {
                let (lo, hi) = integer_range(op, (*a0, *a1), (*b0, *b1));
                let t = Ty::Integer(lo, hi);
                let rt = self.module().rust_type(&t);
                Some(ex(format!("{rt}::from_i128({}.to_i128() {op} {}.to_i128())", wrap(&a), wrap(&b)).replacen('<', "::<", 1), t, Form::Temp, ATOM))
            }
            ("+" | "-" | "*", Ty::Integer(a0, a1), _) if lit_int(&b) => {
                let v: i128 = b.code.parse().unwrap_or(0);
                let (lo, hi) = integer_range(op, (*a0, *a1), (v, v));
                let t = Ty::Integer(lo, hi);
                let rt = self.module().rust_type(&t);
                Some(ex(format!("{rt}::from_i128({}.to_i128() {op} {v})", wrap(&a)).replacen('<', "::<", 1), t, Form::Temp, ATOM))
            }
            ("/", Ty::Decimal(..), _) | ("/", _, Ty::Decimal(..)) | ("/", Ty::Integer(..), _) => {
                self.error("`/` is defined on rationals and `float64` only: a decimal divides with `divide_rounded` or `divide_exact`, an integer with `divide_floor` and the like (Y § 3.1, § 3.3)");
                None
            }
            _ => None,
        }
    }

    /// Two decimals, or two integers, compared exactly across their constraints: each in one type that holds both.
    pub fn compare_numbers(&mut self, a: &Ex, b: &Ex) -> Option<(String, String)> {
        // a number as written takes the other's type, below
        if (is_literal_number(a) || is_literal_number(b)) && (matches!(a.ty, Ty::Decimal(..)) || matches!(b.ty, Ty::Decimal(..))) {
            return None;
        }
        match (&a.ty, &b.ty) {
            (Ty::Decimal(p1, s1), Ty::Decimal(p2, s2)) if (p1, s1) != (p2, s2) => {
                let s = (*s1).max(*s2);
                let p = crate::ty::capped((p1 - s1).max(p2 - s2) + s);
                Some((self.as_decimal(a, p, s), self.as_decimal(b, p, s)))
            }
            (Ty::Integer(..), Ty::Integer(..)) if a.ty != b.ty => Some((format!("{}.to_i128()", wrap(a)), format!("{}.to_i128()", wrap(b)))),
            (Ty::Integer(..), Ty::Int) => Some((format!("{}.to_i128()", wrap(a)), b.code.clone())),
            (Ty::Int, Ty::Integer(..)) => Some((a.code.clone(), format!("{}.to_i128()", wrap(b)))),
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
                let mode = arg("rounding").and_then(rounding_mode).unwrap_or_else(|| "HalfEven".into());
                let from = r.clone();
                ex(format!("{from}.to_decimal_rounded::<{}>(Rounding::{mode})", decimal_args(p, s)), Ty::Decimal(p, s), Form::Temp, ATOM)
            }
            // `a.divide_rounded(b, rounding.half_even)`: the quotient at the dividend's scale, rounded as named; the two
            // in one size, the larger precision of theirs
            (Ty::Decimal(p1, s1), "divide_rounded" | "divide_exact") => {
                let b = self.expr(args[0].1);
                let Ty::Decimal(p2, s2) = b.ty.clone() else {
                    self.error(format!("`{name}` divides a decimal by a decimal"));
                    return None;
                };
                let p = (*p1).max(p2);
                let (l, rr) = (self.as_decimal(recv, p, *s1), self.as_decimal(&b, p, s2));
                if name == "divide_exact" {
                    let t = Ty::Res(Box::new(Ty::Decimal(p, *s1)), Box::new(Ty::Named("inexact".into())));
                    ex(format!("{l}.divide_exact({rr})"), t, Form::Temp, ATOM)
                } else {
                    let mode = args.get(1).map(|a| a.1).or(arg("rounding")).and_then(rounding_mode).unwrap_or_else(|| {
                        self.error("`divide_rounded` names its rounding mode: `divide_rounded(b, rounding.half_even)` (Y § 3.3)");
                        "HalfEven".into()
                    });
                    ex(format!("{l}.divide_rounded({rr}, Rounding::{mode})"), Ty::Decimal(p, *s1), Form::Temp, ATOM)
                }
            }
            // `to_integer_rounded(rounding.half_even)`: the whole number nearest, as an integer of the range the
            // decimal's whole part has
            (Ty::Decimal(p, s), "to_integer_rounded") => {
                let mode = args.first().map(|a| a.1).and_then(rounding_mode).unwrap_or_else(|| "HalfEven".into());
                let m = 10i128.saturating_pow(p - s);
                let t = Ty::Integer(-m, m);
                let rt = self.module().rust_type(&t).replacen('<', "::<", 1);
                ex(format!("{rt}::from_i128({r}.to_integer_rounded(Rounding::{mode}))"), t, Form::Temp, ATOM)
            }
            (Ty::Decimal(..), "to_float64_nearest") => ex(format!("{r}.to_float64_nearest()"), Ty::F64, Form::Temp, ATOM),
            // `to_decimal_exact(scale: s)`: the decimal, or `inexact` when it cannot hold the rational
            (Ty::Rational(_), "to_decimal_exact") => {
                let s: u32 = arg("scale").and_then(|x| x.str("text")).and_then(|t| t.parse().ok()).unwrap_or(0);
                let (p, s) = match &self.want_decimal {
                    Some((wp, ws)) if *ws == s => (*wp, *ws),
                    _ => (38, s),
                };
                let t = Ty::Res(Box::new(Ty::Decimal(p, s)), Box::new(Ty::Named("inexact".into())));
                ex(format!("{r}.to_decimal_exact::<{}>()", decimal_args(p, s)), t, Form::Temp, ATOM)
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
                // a lambda is a closure, whose arguments are not coerced: it is given the element's reference
                let item = if self.locals_fns.contains_key(&f) { "*item" } else { "item" };
                ex(format!("{it}.filter(|item| {f}({item}))"), Ty::Iter(e.clone()), Form::Temp, ATOM)
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
                    if x.kind == "enqueue" {
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
        // the outbox of each command the region enqueues
        for b in body {
            b.walk(&mut |x: &Node| {
                if x.kind != "enqueue" {
                    return;
                }
                let cmd = x.node("value").and_then(|v| v.node("callee")).and_then(|c| c.str("name")).unwrap_or("").to_string();
                for s in self.scopes.iter().rev() {
                    for (n, v) in s {
                        if let Ty::Shared(t) = &v.ty {
                            if matches!(&**t, Ty::Outbox(c) if **c == Ty::Named(cmd.clone())) && !names.iter().any(|(m, _)| m == n) {
                                names.push((n.clone(), (**t).clone()));
                            }
                        }
                    }
                }
            });
        }
        names
    }
}

/// A rounding mode as the runtime names it: `rounding.half_even` is `HalfEven`.
fn rounding_mode(n: &Node) -> Option<String> {
    (n.kind == "member" && n.node("object").and_then(|o| o.str("name")) == Some("rounding")).then(|| n.str("name").map(pascal)).flatten()
}

fn wrap_at(e: &Ex, min: u8) -> String {
    if e.prec < min {
        format!("({})", e.code)
    } else {
        e.code.clone()
    }
}

fn wrap(e: &Ex) -> String {
    if e.prec < ATOM {
        format!("({})", e.code)
    } else {
        e.code.clone()
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

/// The constraints of a region that wrap what keeps its others: its bound, whether its expiry is a failure, whether
/// it holds cancellation, and the `forever` capability its waits are given.
pub struct Wrapping {
    pub bound: Option<String>,
    pub or_fail: bool,
    pub uncancellable: bool,
    pub forever: Option<String>,
}

impl<'a> Lowerer<'a> {
    /// What wraps a region's core: its `within` bound, `uncancellable` and `forever`.
    pub fn wrapping(&mut self, st: &Node) -> Wrapping {
        let cs = st.nodes("constraints");
        let within = cs.iter().find(|c| c.str("word") == Some("within"));
        let bound = within.and_then(|c| c.node("bound")).map(|b| self.expr(b).code);
        let forever = cs.iter().any(|c| c.str("word") == Some("forever")).then(|| match self.var("forever").map(|v| v.form) {
            Some(Form::Ref | Form::Mut) => "forever".to_string(),
            _ => "&forever".to_string(),
        });
        Wrapping { bound, or_fail: within.is_some_and(|c| c.flag("or_fail")), uncancellable: cs.iter().any(|c| c.str("word") == Some("uncancellable")), forever }
    }

    /// The text that opens and closes what wraps a call that keeps a region's core: `Cancellation::new().held(…, || `
    /// for `uncancellable`, `Forever::wait_on(forever, || ` for `forever`.
    pub fn wrap_call(&mut self, w: &Wrapping, label: &str) -> (String, String) {
        let (mut open, mut close) = (String::new(), String::new());
        if let Some(f) = &w.forever {
            open.push_str(&format!("Forever::wait_on({f}, || "));
            close.push(')');
        }
        if w.uncancellable {
            let b = w.bound.clone().unwrap_or_else(|| "Duration::MAX".into());
            open.push_str(&format!("Cancellation::new().held(\"{label}\", {b}, || "));
            close.push(')');
        }
        (open, close)
    }

    /// The function's failure for a deadline that passed: its type's case that carries an `expired`, with the case's
    /// field.
    pub fn expired_case(&self) -> Option<(String, String, String)> {
        let Some(Ty::Named(n)) = self.ctx.fails.clone() else { return None };
        match self.module().types.get(&n).map(|t| &t.def) {
            Some(crate::model::TypeDef::Enum { cases }) => cases.iter().find(|(_, fs)| matches!(&fs[..], [(_, Ty::Named(t))] if t == "expired")).map(|(c, fs)| (pascal(&n), pascal(c), fs[0].0.clone())),
            _ => None,
        }
    }

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
        let w = self.wrapping(st);
        let deadline = if w.bound.is_some() { "Some(&deadline)" } else { "None" };
        let (open, close) = self.wrap_call(&w, &format!("{label}#{}", st.str("name").unwrap()));
        let expiry = if w.bound.is_some() && w.or_fail { self.expired_case() } else { None };
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
            if let Some(b) = &w.bound {
                self.line(format!("let deadline = Deadline::after({b});"));
            }
            let call = format!("{open}orelse(\"{label}\", {deadline}, vec![{}]){close}", names.join(", "));
            self.finish_atomic_with(&call, returns, tail, &result_name, &format!("{label}#{last}"), expiry.as_ref());
            return;
        }
        let name = st.str("name").unwrap();
        let cs: Vec<String> = st.nodes("constraints").iter().map(|c| c.str("text").unwrap_or("").to_string()).collect();
        self.line(format!("// y: region {name} is {}", cs.join(", ")));
        if let Some(b) = &w.bound {
            // the deadline every wait of the region shares: its `waits until`
            self.line(format!("let deadline = Deadline::after({b});"));
        }
        let isolation = st.nodes("constraints").iter().find(|c| c.str("word") == Some("isolation")).and_then(|c| c.str("level")).map(pascal);
        let head = match &isolation {
            Some(level) => format!("let {name} = {open}atomic_with(\"{label}#{name}\", {deadline}, Isolation::{level}, |tx| -> Result<Step<{rust}>, Infallible> {{"),
            None => format!("let {name} = {open}atomic(\"{label}#{name}\", {deadline}, |tx| -> Result<Step<{rust}>, Infallible> {{"),
        };
        self.line(head);
        let body = self.atomic_closure(st, &rt);
        self.out.extend(body);
        self.line(format!("}}){close};"));
        self.finish_atomic_with(name, returns, tail, &result_name, &format!("{label}#{name}"), expiry.as_ref());
        if !returns && !self.rest.is_empty() {
            self.line(format!("// y: end region {name}"));
        }
    }

    /// What an atomic region's runner gives: its value; when its deadline passed, the function's failure that carries
    /// the expiry, for a region that is `within d or fail`; and otherwise a fault, as nothing else aborts it.
    fn finish_atomic_with(&mut self, call: &str, returns: bool, tail: bool, result_name: &str, label: &str, expiry: Option<&(String, String, String)>) {
        let Some((en, case, field)) = expiry else { return self.finish_atomic(call, returns, tail, result_name, label) };
        self.use_("y_runtime::shared::Aborted");
        let ok = if self.ctx.fails.is_some() { format!("Ok({result_name})") } else { result_name.to_string() };
        if returns {
            let lead = if tail { "" } else { "return " };
            self.line(format!("{lead}match {call} {{"));
            self.line(format!("    Ok({result_name}) => {ok},"));
        } else {
            self.line(format!("match {call} {{"));
            self.line("    Ok(()) => {}");
        }
        self.line(format!("    Err(Aborted::Expired({field})) => return Err({en}::{case} {{ {field} }}),"));
        self.line(format!("    Err(_) => not_reachable(\"{label}\"),"));
        self.line(if returns && tail { "}" } else if returns { "};" } else { "}" });
    }

    /// What an atomic region's runner gives: its value, or, when it was aborted, a fault, as nothing aborts it.
    fn finish_atomic(&mut self, call: &str, returns: bool, tail: bool, result_name: &str, label: &str) {
        if returns {
            let lead = if tail { "" } else { "return " };
            self.line(format!("{lead}match {call} {{"));
            let ok = if self.ctx.fails.is_some() { format!("Ok({result_name})") } else { result_name.to_string() };
            self.line(format!("    Ok({result_name}) => {ok},"));
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
        // `detach because "…"`: the tasks run on work of their own, which may outlive the function
        let detached = st.nodes("constraints").iter().any(|c| c.str("word") == Some("detach"));
        if let Some(b) = &within {
            let b = self.expr(b).code;
            self.line(format!("let deadline = Deadline::after({b});"));
            if detached {
                self.detach_shared(st);
                self.line(format!("let {name} = detach(deadline.remaining().unwrap_or_default(), move || {{"));
                self.ind += 1;
            }
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
        let w = self.wrapping(st);
        let (open, close) = self.wrap_call(&w, &format!("{}::{}#{name}", self.ctx.module, self.ctx.name));
        self.line(format!("if let Err(failed) = {open}supervised(policy, vec![{}]){close} {{", tasks.join(", ")));
        self.line("    match failed.failure {}");
        self.line("}");
        if within.is_some() && !detached {
            // the function's failure for a deadline that passed: the case that carries an `expired`
            if let Some((en, c, f)) = self.expired_case() {
                self.line(format!("deadline.remaining().map_err(|{f}| {en}::{c} {{ {f} }})?;"));
            }
        }
        if detached {
            self.ind -= 1;
            self.line("});");
            self.line(format!("// y: end region {name}"));
            // its tasks may outlive the function; it is waited for, no longer than its bound, before the function ends
            self.line(format!("let _ = {name}.wait();"));
        } else if !self.rest.is_empty() {
            self.line(format!("// y: end region {name}"));
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
        // what its `or fail`s fail with: the console's failure, the one the examples' detached regions have, in a
        // function that may fail; in one that cannot, the work does not fail either
        let fallible = self.ctx.fails.is_some();
        let fails = Ty::Named("console_error".into());
        let ft = if fallible { self.rust_type(&fails) } else { "std::convert::Infallible".into() };
        self.line(format!("// y: region {name} is {}", cs.join(", ")));
        self.detach_shared(st);
        self.line(format!("let {name} = detach({bound}, move || -> Result<(), {ft}> {{"));
        self.ind += 1;
        let saved = self.ctx.fails.replace(fails);
        let held = st.nodes("constraints").iter().any(|c| c.str("word") == Some("uncancellable"));
        if held {
            // `uncancellable`: cancellation is held off while the detached work runs, for no longer than its bound
            self.line(format!("Cancellation::new().held(\"{}::{}#{name}\", {bound}, || -> Result<(), {ft}> {{", self.ctx.module, self.ctx.name));
            self.ind += 1;
        }
        self.block(&st.nodes("body"), false);
        self.ctx.fails = saved;
        self.line("Ok(())");
        if held {
            self.ind -= 1;
            self.line("})");
        }
        self.ind -= 1;
        self.line("});");
        self.line(format!("// y: end region {name}"));
        if fallible {
            self.line(format!("if let Ok(done) = {name}.wait() {{"));
            self.line("    done?;");
            self.line("}");
        } else {
            self.line(format!("let _ = {name}.wait();"));
        }
    }

    /// The shared state a detached region uses, each a handle of its own that the detached work takes with it: a
    /// `shared<…>` is a handle to one state, and the work may outlive the borrow the function was lent.
    pub fn detach_shared(&mut self, st: &Node) {
        // the names its statements read, and the capabilities injected into the functions they call
        let mut names = vec![];
        for b in st.nodes("body") {
            b.walk(&mut |n: &Node| {
                let mut add = |nm: String| if !names.contains(&nm) { names.push(nm) };
                if n.kind == "name" {
                    add(n.str("name").unwrap().to_string());
                }
                if n.kind == "call" {
                    if let Some(f) = n.node("callee").and_then(|c| (c.kind == "name").then(|| c.str("name")).flatten()).and_then(|f| self.function(f)) {
                        for p in f.injected() {
                            add(p.name.clone());
                        }
                    }
                }
            });
        }
        for n in names {
            if let Some(Var { ty: t @ Ty::Shared(_), .. }) = self.var(&n).cloned() {
                self.line(format!("let {n} = {n}.clone();"));
                self.bind(&n, t, Form::Place);
            }
        }
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

//! The type language of the constraint file (proposal 4.4): a small AST, its string form, and its Rust form.
use std::fmt::Write;

pub type R<T> = Result<T, String>;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Prim(String),
    Text,
    Char,
    Name(String),
    App(String, Vec<Ty>),
    Ref(Box<Ty>),
    /// A host pointer: the address of a `T`, with no guarantee that it is valid or about any other path (Rust's
    /// `*const T` and `*mut T`). The slot's access says whether it may write: `read` for `*const`, none for `*mut`.
    Ptr(Box<Ty>),
    Opt(Box<Ty>),
    Tuple(Vec<Ty>),
    Coll(String, Vec<Ty>),
    As(Box<Ty>, String),
    /// A refinement: the predicate after `where`, erased in Rust.
    Refined(Box<Ty>, String),
    /// A lifetime argument of a host type: `'a`, `'_`, `'static`. Only an argument of `App` or of a bound has one.
    Lifetime(String),
    /// `some I`: a value of one type, fixed by the subject, that implements the interfaces (Rust's `impl Trait`).
    Some(Vec<Bound>),
    /// `any I`: a value of any type that implements the interfaces, reached through a reference or a host pointer
    /// (Rust's `dyn Trait`).
    Any(Vec<Bound>),
}

/// One bound of an existential type: an interface with its arguments and its associated types, or a lifetime.
#[derive(Clone, Debug, PartialEq)]
pub enum Bound {
    /// `Into<string>`, `Iterator { Item = u8 }`, or the call sugar `Fn(u8) -> bool`.
    Iface { path: String, args: Vec<Ty>, assoc: Vec<(String, Ty)>, call: Option<(Vec<Ty>, Option<Box<Ty>>)> },
    Lifetime(String),
}

const PRIMS: [&str; 14] =
    ["bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64", "_"];
pub const RESERVED: [&str; 9] = ["string", "scalar", "list", "map", "set", "ref", "ptr", "some", "any"];

/// The names of the type arguments of the language's own generic types, in the order they are written. A host type's
/// arguments (`Node<'a>`) are the host's, and positional.
pub fn arg_names(ctor: &str) -> &'static [&'static str] {
    match ctor {
        "ref" | "ptr" => &["target type"],
        "list" => &["element type"],
        "set" => &["element type", "hasher type"],
        "map" => &["key type", "value type", "hasher type"],
        _ => &[],
    }
}

pub fn is_prim(s: &str) -> bool {
    PRIMS.contains(&s)
}

fn join(v: &[Ty]) -> String {
    v.iter().map(Ty::show).collect::<Vec<_>>().join(", ")
}

/// The result of call sugar: an `as` or a refinement after it would belong to the whole type, so a result that has
/// one is in brackets: `Fn(u8) -> (u64 as 'usize')?`.
fn show_ret(t: &Ty) -> String {
    match t {
        Ty::Opt(i) => format!("{}?", show_ret(i)),
        Ty::As(..) | Ty::Refined(..) => format!("({})", t.show()),
        _ => t.show(),
    }
}

fn show_bounds(b: &[Bound]) -> String {
    b.iter().map(Bound::show).collect::<Vec<_>>().join(" + ")
}

impl Bound {
    /// The string form: `Into<string>`, `Iterator { Item = u8 }`, `Fn(u8) -> bool`, `'_`.
    pub fn show(&self) -> String {
        match self {
            Bound::Lifetime(l) => l.clone(),
            Bound::Iface { path, call: Some((a, r)), .. } => {
                format!("{path}({}){}", join(a), r.as_ref().map(|r| format!(" -> {}", show_ret(r))).unwrap_or_default())
            }
            Bound::Iface { path, args, assoc, .. } => {
                let mut s = path.clone();
                if !args.is_empty() {
                    s += &format!("<{}>", join(args));
                }
                if !assoc.is_empty() {
                    let a: Vec<String> = assoc.iter().map(|(n, t)| format!("{n} = {}", t.show())).collect();
                    s += &format!(" {{ {} }}", a.join(", "));
                }
                s
            }
        }
    }
}

/// How `rust` prints: the access and lifetime of every `ref` outside an existential's bounds, and, when the check
/// of a library file lowers existentials, the generic parameter each `some` becomes.
struct Cx {
    m: bool,
    lt: Option<String>,
    lower: Option<Lower>,
    /// Inside the bounds of an existential: a `ref` there is shared and unnamed (`&T`).
    in_bound: bool,
}

/// A lowering of `some I` to `__S0: I`, for a function pointer type, which cannot hold `impl Trait`.
#[derive(Default)]
pub struct Lower {
    /// The lifetime of the shared references inside a bound, which a generic's bound cannot leave elided.
    pub inner: String,
    /// `__S0: Bound + ...` for each `some` met, in order.
    pub params: Vec<String>,
}

impl Ty {
    /// The string form, in the constraint language's own type syntax.
    pub fn show(&self) -> String {
        match self {
            Ty::Prim(p) => p.clone(),
            Ty::Text => "string".into(),
            Ty::Char => "scalar".into(),
            Ty::Name(n) => n.clone(),
            Ty::App(n, a) => format!("{n}<{}>", join(a)),
            Ty::Ref(t) => format!("ref<target type={}>", t.show()),
            Ty::Ptr(t) => format!("ptr<target type={}>", t.show()),
            Ty::Opt(t) => format!("{}?", t.show()),
            Ty::Tuple(v) if v.len() == 1 => format!("({},)", v[0].show()),
            Ty::Tuple(v) => format!("({})", join(v)),
            Ty::Coll(k, a) => {
                let named: Vec<String> =
                    arg_names(k).iter().zip(a).map(|(n, t)| format!("{n}={}", t.show())).collect();
                format!("{k}<{}>", named.join(", "))
            }
            Ty::As(t, w) => format!("{} as '{w}'", t.show()),
            Ty::Refined(t, p) => format!("{} where {p}", t.show()),
            Ty::Lifetime(l) => l.clone(),
            Ty::Some(b) => format!("some {}", show_bounds(b)),
            Ty::Any(b) => format!("any {}", show_bounds(b)),
        }
    }

    pub fn has_ref(&self) -> bool {
        match self {
            Ty::Ref(_) => true,
            Ty::Ptr(t) | Ty::Opt(t) | Ty::As(t, _) | Ty::Refined(t, _) => t.has_ref(),
            Ty::App(_, v) | Ty::Tuple(v) | Ty::Coll(_, v) => v.iter().any(Ty::has_ref),
            _ => false,
        }
    }

    pub fn has_ptr(&self) -> bool {
        match self {
            Ty::Ptr(_) => true,
            Ty::Ref(t) | Ty::Opt(t) | Ty::As(t, _) | Ty::Refined(t, _) => t.has_ptr(),
            Ty::App(_, v) | Ty::Tuple(v) | Ty::Coll(_, v) => v.iter().any(Ty::has_ptr),
            _ => false,
        }
    }

    /// Whether the slot's access lets its references or pointers write: a reference writes with `write`, a pointer
    /// with anything but `read` (no access on a pointer is the absence of the read-only constraint, `*mut`).
    pub fn writes(&self, rights: Option<&str>) -> bool {
        if self.has_ptr() && !self.has_ref() {
            rights != Some("read")
        } else {
            rights == Some("write")
        }
    }

    /// Whether a reference hides inside the bounds of a `some` or `any`: `some Iterator { Item = ref<target type=string> }`.
    /// Such a reference is not a slot's `ref`, yet the value still borrows.
    pub fn bound_borrows(&self) -> bool {
        let inside = |b: &[Bound]| {
            b.iter().any(|b| match b {
                Bound::Lifetime(_) => false,
                // the arguments of call sugar are higher-ranked: `Fn(&u8)` borrows from nothing
                Bound::Iface { args, assoc, .. } => {
                    let refs = |t: &Ty| t.has_ref() || t.bound_borrows();
                    args.iter().any(refs) || assoc.iter().any(|(_, t)| refs(t))
                }
            })
        };
        match self {
            Ty::Some(b) | Ty::Any(b) => inside(b),
            Ty::Ref(t) | Ty::Ptr(t) | Ty::Opt(t) | Ty::As(t, _) | Ty::Refined(t, _) => t.bound_borrows(),
            Ty::App(_, v) | Ty::Tuple(v) | Ty::Coll(_, v) => v.iter().any(Ty::bound_borrows),
            _ => false,
        }
    }

    /// Whether the outermost wrapper is shared ownership.
    pub fn shares(&self) -> bool {
        matches!(self, Ty::As(_, w) if w.rsplit("::").next().is_some_and(|l| l == "Rc" || l == "Arc"))
    }

    /// The Rust text. Every `ref` in the type is printed with `mutable` and `lifetime`; those inside the bounds of a
    /// `some` or `any` are always shared and elided.
    pub fn rust(&self, mutable: bool, lifetime: Option<&str>) -> R<String> {
        self.rust_in(&mut Cx { m: mutable, lt: lifetime.map(str::to_string), lower: None, in_bound: false }, false)
    }

    /// The Rust text with each `some I` replaced by a new generic parameter `__S<n>` (recorded in `lower`), as a
    /// function pointer type needs: `fn(__S0)` with `__S0: I`. The shared references inside bounds take `lower.inner`.
    pub fn rust_lowered(&self, mutable: bool, lifetime: Option<&str>, lower: &mut Lower) -> R<String> {
        let mut cx = Cx { m: mutable, lt: lifetime.map(str::to_string), lower: Some(std::mem::take(lower)), in_bound: false };
        let r = self.rust_in(&mut cx, false);
        *lower = cx.lower.take().unwrap_or_default();
        r
    }

    /// The bounds of a top-level `some` or `any` as Rust text, with the shared references inside them named `inner`:
    /// what a generic parameter `R: …` must satisfy. `None` for any other type.
    pub fn bounds_rust(&self, inner: &str) -> Option<R<String>> {
        let (Ty::Some(b) | Ty::Any(b)) = self else { return None };
        let mut cx = Cx { m: false, lt: None, lower: Some(Lower { inner: inner.to_string(), params: vec![] }), in_bound: false };
        Some(Self::rust_bounds(b, &mut cx))
    }

    /// Whether the type has a `some` anywhere.
    pub fn has_some(&self) -> bool {
        match self {
            Ty::Some(_) => true,
            Ty::Any(b) => b.iter().any(|b| match b {
                Bound::Lifetime(_) => false,
                Bound::Iface { args, assoc, call, .. } => {
                    args.iter().any(Ty::has_some)
                        || assoc.iter().any(|(_, t)| t.has_some())
                        || call.as_ref().is_some_and(|(a, r)| a.iter().any(Ty::has_some) || r.as_ref().is_some_and(|r| r.has_some()))
                }
            }),
            Ty::Ref(t) | Ty::Ptr(t) | Ty::Opt(t) | Ty::As(t, _) | Ty::Refined(t, _) => t.has_some(),
            Ty::App(_, v) | Ty::Tuple(v) | Ty::Coll(_, v) => v.iter().any(Ty::has_some),
            _ => false,
        }
    }

    fn rust_bounds(bs: &[Bound], cx: &mut Cx) -> R<String> {
        let was = cx.in_bound;
        cx.in_bound = true;
        let mut parts = vec![];
        for b in bs {
            parts.push(match b {
                Bound::Lifetime(l) => l.clone(),
                Bound::Iface { path, call: Some((a, r)), .. } => {
                    let a: R<Vec<String>> = a.iter().map(|t| t.rust_in(cx, false)).collect();
                    let r = match r {
                        Some(r) => format!(" -> {}", r.rust_in(cx, false)?),
                        None => String::new(),
                    };
                    format!("{path}({}){r}", a?.join(", "))
                }
                Bound::Iface { path, args, assoc, .. } => {
                    let mut inner: Vec<String> = vec![];
                    for t in args {
                        inner.push(t.rust_in(cx, false)?);
                    }
                    for (n, t) in assoc {
                        inner.push(format!("{n} = {}", t.rust_in(cx, false)?));
                    }
                    if inner.is_empty() {
                        path.clone()
                    } else {
                        format!("{path}<{}>", inner.join(", "))
                    }
                }
            });
        }
        cx.in_bound = was;
        Ok(parts.join(" + "))
    }

    fn rust_in(&self, cx: &mut Cx, under_ref: bool) -> R<String> {
        let list = |v: &[Ty], cx: &mut Cx| -> R<String> {
            let mut parts = vec![];
            for t in v {
                parts.push(t.rust_in(cx, false)?);
            }
            Ok(parts.join(", "))
        };
        Ok(match self {
            Ty::Prim(p) => p.clone(),
            Ty::Text if under_ref => "str".into(),
            Ty::Text => "String".into(),
            Ty::Char => "char".into(),
            Ty::Name(n) => n.clone(),
            Ty::Lifetime(l) => l.clone(),
            Ty::App(n, a) => format!("{n}<{}>", list(a, cx)?),
            Ty::Ref(t) => {
                let mut s = String::from("&");
                let lt = if cx.in_bound { cx.lower.as_ref().map(|l| l.inner.clone()) } else { cx.lt.clone() };
                if let Some(l) = lt {
                    let _ = write!(s, "{l} ");
                }
                if cx.m && !cx.in_bound {
                    s.push_str("mut ");
                }
                let inner = t.rust_in(cx, true)?;
                // `&dyn A + B` would read as `(&dyn A) + B`
                match &**t {
                    Ty::Any(b) | Ty::Some(b) if b.len() > 1 => s + &format!("({inner})"),
                    _ => s + &inner,
                }
            }
            // A pointer inside the bounds of an existential reads only, as a reference there is shared.
            Ty::Ptr(t) => {
                let s = if cx.m && !cx.in_bound { "*mut " } else { "*const " };
                let inner = t.rust_in(cx, true)?;
                match &**t {
                    Ty::Any(b) | Ty::Some(b) if b.len() > 1 => format!("{s}({inner})"),
                    _ => format!("{s}{inner}"),
                }
            }
            Ty::Opt(t) => format!("Option<{}>", t.rust_in(cx, false)?),
            Ty::Tuple(v) if v.len() == 1 => format!("({},)", list(v, cx)?),
            Ty::Tuple(v) => format!("({})", list(v, cx)?),
            Ty::Coll(k, _) => return Err(format!("slot missing: host type name of `{k}<..>` (`as`)")),
            Ty::Refined(t, _) => t.rust_in(cx, under_ref)?,
            Ty::Some(b) => {
                let text = Self::rust_bounds(b, cx)?;
                match cx.lower.as_mut() {
                    Some(l) => {
                        let n = format!("__S{}", l.params.len());
                        l.params.push(format!("{n}: {text}"));
                        n
                    }
                    None => format!("impl {text}"),
                }
            }
            Ty::Any(b) => format!("dyn {}", Self::rust_bounds(b, cx)?),
            Ty::As(t, w) => match (&**t, w.as_str()) {
                (_, "usize" | "isize" | "char") => w.clone(),
                (Ty::Text, "String") => "String".into(),
                // a slice is a `list` whose host form is the unsized `[T]`, which only a reference can name
                (Ty::Coll(k, a), "std::primitive::slice") if k == "list" => format!("[{}]", list(a, cx)?),
                (Ty::Refined(l, p), "std::primitive::array") if matches!(&**l, Ty::Coll(k, _) if k == "list") => {
                    let (Ty::Coll(_, a), Some(n)) = (&**l, p.strip_prefix("count(value) == ")) else {
                        return Err(format!("bad array refinement `{p}`"));
                    };
                    format!("[{}; {n}]", list(a, cx)?)
                }
                (Ty::Coll(_, a), _) => format!("{w}<{}>", list(a, cx)?),
                (t, _) => format!("{w}<{}>", t.rust_in(cx, false)?),
            },
        })
    }

    /// Parse the string form.
    pub fn parse(s: &str) -> R<Ty> {
        let mut p = P { s: s.as_bytes(), i: 0 };
        let t = p.ty()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(format!("bad type `{s}` at {}", p.i));
        }
        Ok(t)
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i) == Some(&b' ') {
            self.i += 1;
        }
    }
    fn eat(&mut self, t: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(t.as_bytes()) {
            self.i += t.len();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: &str) -> R<()> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(format!("bad type: expected `{t}` at {}", self.i))
        }
    }
    fn ident(&mut self) -> String {
        self.ws();
        let st = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_' || self.s[self.i] == b':') {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[st..self.i]).into_owned()
    }
    /// The text of a refinement: up to the ` as '` that ends the type, or to the end.
    fn pred(&mut self) -> R<String> {
        let rest = String::from_utf8_lossy(&self.s[self.i..]).into_owned();
        let end = rest.find(" as '").unwrap_or(rest.len());
        self.i += end;
        match rest[..end].trim() {
            "" => Err(format!("bad type: empty refinement at {}", self.i)),
            p => Ok(p.to_string()),
        }
    }
    /// The named arguments of one of the language's own generic types: `list<element type=u8>`. Each name is required,
    /// in the order `arg_names` gives; a positional argument is refused with the form it should take.
    fn named_args(&mut self, ctor: &str) -> R<Vec<Ty>> {
        let names = arg_names(ctor);
        let mut v = vec![];
        loop {
            self.ws();
            let st = self.i;
            while self.s.get(self.i).is_some_and(|c| c.is_ascii_lowercase() || *c == b' ') {
                self.i += 1;
            }
            let name = String::from_utf8_lossy(&self.s[st..self.i]).trim().to_string();
            let want = names.get(v.len()).copied();
            if self.s.get(self.i) != Some(&b'=') || self.s.get(self.i + 1) == Some(&b'=') || want != Some(name.as_str()) {
                let shown: Vec<&&str> = names.iter().filter(|n| **n != "hasher type").collect();
                let form: Vec<String> = shown.iter().map(|n| format!("{n}=…")).collect();
                return Err(format!("bad type: the arguments of `{ctor}` are named, `{ctor}<{}>`, at {st}", form.join(", ")));
            }
            self.i += 1;
            v.push(self.arg()?);
            if !self.eat(",") {
                break;
            }
        }
        self.expect(">")?;
        Ok(v)
    }
    fn args(&mut self) -> R<Vec<Ty>> {
        let mut v = vec![self.arg()?];
        while self.eat(",") {
            v.push(self.arg()?);
        }
        self.expect(">")?;
        Ok(v)
    }
    /// A lifetime (`'a`, `'_`) as opposed to a quoted host path (`'std::x'`, `'v1'`): after the quote comes a name
    /// that no closing quote and no `::` follows.
    fn at_lifetime(&mut self) -> Option<String> {
        self.ws();
        if self.s.get(self.i) != Some(&b'\'') {
            return None;
        }
        let mut j = self.i + 1;
        while self.s.get(j).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            j += 1;
        }
        let first = self.s.get(self.i + 1).copied().unwrap_or(b' ');
        let named = j > self.i + 1 && (first.is_ascii_alphabetic() || first == b'_');
        if !named || self.s.get(j) == Some(&b'\'') || self.s.get(j) == Some(&b':') {
            return None;
        }
        let l = String::from_utf8_lossy(&self.s[self.i..j]).into_owned();
        self.i = j;
        Some(l)
    }
    fn arg(&mut self) -> R<Ty> {
        match self.at_lifetime() {
            Some(l) => Ok(Ty::Lifetime(l)),
            None => self.ty(),
        }
    }
    /// `+`-separated bounds after `some` or `any`: a name or quoted host path, with arguments, associated types
    /// `{ A = T }` or call sugar `(T) -> U`; or a lifetime.
    fn bounds(&mut self) -> R<Vec<Bound>> {
        let mut v = vec![];
        loop {
            if let Some(l) = self.at_lifetime() {
                v.push(Bound::Lifetime(l));
            } else {
                self.ws();
                let path = if self.s.get(self.i) == Some(&b'\'') {
                    self.i += 1;
                    let st = self.i;
                    while self.s.get(self.i).is_some_and(|c| *c != b'\'') {
                        self.i += 1;
                    }
                    let p = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                    self.expect("'")?;
                    p
                } else {
                    self.ident()
                };
                if path.is_empty() {
                    return Err(format!("bad type: an interface is expected at {}", self.i));
                }
                let (mut args, mut assoc, mut call) = (vec![], vec![], None);
                if self.s.get(self.i) == Some(&b'<') {
                    self.i += 1;
                    args = self.args()?;
                }
                if self.eat("(") {
                    let mut a = vec![];
                    if !self.eat(")") {
                        a.push(self.ty()?);
                        while self.eat(",") {
                            a.push(self.ty()?);
                        }
                        self.expect(")")?;
                    }
                    let r = if self.eat("->") { Some(Box::new(self.ret()?)) } else { None };
                    call = Some((a, r));
                } else if self.eat("{") {
                    loop {
                        let n = self.ident();
                        if n.is_empty() {
                            return Err(format!("bad type: an associated type name is expected at {}", self.i));
                        }
                        self.expect("=")?;
                        assoc.push((n, self.ty()?));
                        if !self.eat(",") {
                            break;
                        }
                    }
                    self.expect("}")?;
                }
                v.push(Bound::Iface { path, args, assoc, call });
            }
            if !self.eat("+") {
                return Ok(v);
            }
        }
    }
    /// The result of call sugar: a type without `as` or a refinement, which belong to the whole type.
    fn ret(&mut self) -> R<Ty> {
        let mut t = self.base()?;
        while self.eat("?") {
            t = Ty::Opt(Box::new(t));
        }
        Ok(t)
    }
    fn ty(&mut self) -> R<Ty> {
        let mut t = self.base()?;
        loop {
            if self.eat("?") {
                t = Ty::Opt(Box::new(t));
            } else if self.eat("as '") {
                let st = self.i;
                while self.s.get(self.i).is_some_and(|c| *c != b'\'') {
                    self.i += 1;
                }
                let w = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                self.expect("'")?;
                t = Ty::As(Box::new(t), w);
            } else {
                if !self.eat("where ") {
                    return Ok(t);
                }
                let p = self.pred()?;
                t = Ty::Refined(Box::new(t), p);
            }
        }
    }
    fn base(&mut self) -> R<Ty> {
        if self.eat("(") {
            let mut v = vec![];
            if !self.eat(")") {
                v.push(self.ty()?);
                let mut comma = false;
                while self.eat(",") {
                    comma = true;
                    if self.eat(")") {
                        return Ok(Ty::Tuple(v));
                    }
                    v.push(self.ty()?);
                }
                self.expect(")")?;
                // `(T)` groups, as a one-tuple is written `(T,)`.
                if !comma {
                    return Ok(v.remove(0));
                }
            }
            return Ok(Ty::Tuple(v));
        }
        let id = self.ident();
        if id.is_empty() {
            return Err(format!("bad type at {}", self.i));
        }
        if (id == "some" || id == "any") && self.s.get(self.i) == Some(&b' ') {
            let b = self.bounds()?;
            return Ok(if id == "some" { Ty::Some(b) } else { Ty::Any(b) });
        }
        let generic = self.s.get(self.i) == Some(&b'<');
        if generic {
            self.i += 1;
            let a = if arg_names(&id).is_empty() { self.args()? } else { self.named_args(&id)? };
            return Ok(match id.as_str() {
                "ref" if a.len() == 1 => Ty::Ref(Box::new(a.into_iter().next().unwrap())),
                "ptr" if a.len() == 1 => Ty::Ptr(Box::new(a.into_iter().next().unwrap())),
                "list" | "map" | "set" => Ty::Coll(id, a),
                _ => Ty::App(id, a),
            });
        }
        if id == "string" {
            return Ok(Ty::Text);
        }
        if id == "scalar" {
            return Ok(Ty::Char);
        }
        Ok(if is_prim(&id) { Ty::Prim(id) } else { Ty::Name(id) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_is_a_list_whose_host_form_is_unsized() {
        let t = Ty::parse("ref<target type=list<element type=T> as 'std::primitive::slice'>").unwrap();
        assert_eq!(t.rust(false, Some("'p0")).unwrap(), "&'p0 [T]");
        assert_eq!(t.rust(true, Some("'p0")).unwrap(), "&'p0 mut [T]");
        assert_eq!(t.show(), "ref<target type=list<element type=T> as 'std::primitive::slice'>");
        assert!(t.has_ref());
        // a Vec is the same `list` with another host form
        assert_eq!(Ty::parse("list<element type=u8> as 'std::vec::Vec'").unwrap().rust(false, None).unwrap(), "std::vec::Vec<u8>");
    }

    #[test]
    fn a_pointer_reads_or_writes_by_its_access() {
        let t = Ty::parse("ptr<target type=u8>").unwrap();
        assert_eq!(t.show(), "ptr<target type=u8>");
        assert!(!t.has_ref() && t.has_ptr());
        // `*const` is `rights: read`; `*mut` is no access at all, where a reference without one reads.
        assert!(!t.writes(Some("read")) && t.writes(None) && t.writes(Some("write")));
        assert!(!Ty::parse("ref<target type=u8>").unwrap().writes(None));
        assert_eq!(t.rust(false, None).unwrap(), "*const u8");
        assert_eq!(t.rust(true, None).unwrap(), "*mut u8");
        assert_eq!(Ty::parse("ptr<target type=string>").unwrap().rust(false, None).unwrap(), "*const str");
        assert_eq!(Ty::parse("ptr<target type=list<element type=u8> as 'std::primitive::slice'>").unwrap().rust(true, None).unwrap(), "*mut [u8]");
        assert_eq!(Ty::parse("list<element type=ptr<target type=Node>> as 'Vec'").unwrap().rust(false, None).unwrap(), "Vec<*const Node>");
    }

    #[test]
    fn a_refinement_is_kept_in_the_string_and_erased_in_rust() {
        let s = r"scalar where value matches /^\x{10FFFF}$/ as 'char'";
        let t = Ty::parse(s).unwrap();
        assert_eq!(t.show(), s);
        assert_eq!(t.rust(false, None).unwrap(), "char");
        assert_eq!(Ty::parse("string where count(scalars(value)) <= 8").unwrap().show(), "string where count(scalars(value)) <= 8");
        assert!(Ty::parse("string where ").is_err());
        // the char form alone is still a char
        assert_eq!(Ty::parse("scalar as 'char'").unwrap(), Ty::As(Box::new(Ty::Char), "char".into()));
    }

    #[test]
    fn existentials_lifetime_arguments_and_associated_types_round_trip() {
        for t in [
            "some Display",
            "some Into<string>",
            "some Iterator { Item = (u8, ref<target type=string>) } + Clone + '_",
            "any std::error::Error + Send + Sync as 'std::boxed::Box'",
            "ref<target type=any Fn(ref<target type=u8>) -> bool>",
            "any FnMut(ref<target type=string>) -> (u64 as 'usize')? + 'static as 'Box'",
            "(u8,)",
            "ref<target type=any Iterator { Item = u8 }>",
            "Chars<'a>",
            "ref<target type=list<element type=Node<'_>> as 'Vec'>",
            "Self::Item",
            "some Foo<'a, u8> { A = u8, B = list<element type=u8> as 'Vec' }",
        ] {
            assert_eq!(Ty::parse(t).unwrap().show(), t, "{t}");
        }
        // a quoted interface is the same interface; a quoted host path is not a lifetime
        assert_eq!(Ty::parse("some 'std::iter::Iterator' { Item = u8 }").unwrap().show(), "some std::iter::Iterator { Item = u8 }");
        assert_eq!(Ty::parse("u8 as 'v1'").unwrap().show(), "u8 as 'v1'");
        assert!(Ty::parse("some").is_ok_and(|t| t == Ty::Name("some".into())), "`some` alone is a name");
        assert!(Ty::parse("some Iterator { Item u8 }").is_err() && Ty::parse("some Iterator { Item = u8").is_err());
        assert!(Ty::parse("some ").is_err());
    }

    #[test]
    fn existentials_print_as_rust_and_lower_to_generics() {
        let rust = |t: &str| Ty::parse(t).unwrap().rust(false, None).unwrap();
        assert_eq!(rust("some Iterator { Item = (u8, ref<target type=string>) } + Clone + '_"), "impl Iterator<Item = (u8, &str)> + Clone + '_");
        assert_eq!(rust("some Into<string>"), "impl Into<String>");
        assert_eq!(rust("ref<target type=any Fn(ref<target type=u8>) -> bool>"), "&dyn Fn(&u8) -> bool");
        assert_eq!(rust("any Error + Send + Sync as 'std::boxed::Box'"), "std::boxed::Box<dyn Error + Send + Sync>");
        assert_eq!(rust("ref<target type=any Error + Send>"), "&(dyn Error + Send)");
        // the references of a bound are shared whatever the slot's access is; the slot's own are not
        assert_eq!(Ty::parse("ref<target type=any Iterator { Item = ref<target type=u8> }>").unwrap().rust(true, Some("'p0")).unwrap(), "&'p0 mut dyn Iterator<Item = &u8>");
        let t = Ty::parse("ref<target type=some Iterator { Item = ref<target type=u8> }>").unwrap();
        assert!(t.has_ref() && t.bound_borrows());
        assert!(!Ty::parse("some Iterator { Item = ref<target type=u8> }").unwrap().has_ref());
        assert!(!Ty::parse("some Fn(ref<target type=u8>) -> bool").unwrap().bound_borrows(), "the arguments of call sugar are higher-ranked");
        let mut lower = Lower { inner: "'__e".into(), params: vec![] };
        let t = Ty::parse("(some Iterator { Item = ref<target type=u8> }, some Clone)").unwrap();
        assert_eq!(t.rust_lowered(false, None, &mut lower).unwrap(), "(__S0, __S1)");
        assert_eq!(lower.params, vec!["__S0: Iterator<Item = &'__e u8>".to_string(), "__S1: Clone".to_string()]);
        assert_eq!(Ty::parse("some Iterator { Item = u8 }").unwrap().bounds_rust("'e").unwrap().unwrap(), "Iterator<Item = u8>");
        assert!(Ty::parse("u8").unwrap().bounds_rust("'e").is_none());
        assert!(Ty::parse("list<element type=some Clone> as 'Vec'").unwrap().has_some());
    }
}

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
    Opt(Box<Ty>),
    Tuple(Vec<Ty>),
    Coll(String, Vec<Ty>),
    As(Box<Ty>, String),
    /// A refinement beyond the one `Char` has: the predicate after `where` (or after `and`, on a `Char`), erased in Rust.
    Refined(Box<Ty>, String),
}

const PRIMS: [&str; 14] =
    ["bool", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64", "_"];
pub const RESERVED: [&str; 5] = ["text", "list", "map", "set", "ref"];

pub fn is_prim(s: &str) -> bool {
    PRIMS.contains(&s)
}

fn join(v: &[Ty]) -> String {
    v.iter().map(Ty::show).collect::<Vec<_>>().join(", ")
}

impl Ty {
    /// The string form, in the constraint language's own type syntax.
    pub fn show(&self) -> String {
        match self {
            Ty::Prim(p) => p.clone(),
            Ty::Text => "text".into(),
            Ty::Char => "text where length(value) == 1".into(),
            Ty::Name(n) => n.clone(),
            Ty::App(n, a) => format!("{n}<{}>", join(a)),
            Ty::Ref(t) => format!("ref<{}>", t.show()),
            Ty::Opt(t) => format!("{}?", t.show()),
            Ty::Tuple(v) if v.len() == 1 => format!("({},)", v[0].show()),
            Ty::Tuple(v) => format!("({})", join(v)),
            Ty::Coll(k, a) => format!("{k}<{}>", join(a)),
            Ty::As(t, w) => format!("{} as '{w}'", t.show()),
            Ty::Refined(t, p) if matches!(**t, Ty::Char) => format!("{} and {p}", t.show()),
            Ty::Refined(t, p) => format!("{} where {p}", t.show()),
        }
    }

    pub fn has_ref(&self) -> bool {
        match self {
            Ty::Ref(_) => true,
            Ty::Opt(t) | Ty::As(t, _) | Ty::Refined(t, _) => t.has_ref(),
            Ty::App(_, v) | Ty::Tuple(v) | Ty::Coll(_, v) => v.iter().any(Ty::has_ref),
            _ => false,
        }
    }

    /// Whether the outermost wrapper is shared ownership.
    pub fn shares(&self) -> bool {
        matches!(self, Ty::As(_, w) if w.rsplit("::").next().is_some_and(|l| l == "Rc" || l == "Arc"))
    }

    /// The Rust text. Every `ref` in the type is printed with `mutable` and `lifetime`.
    pub fn rust(&self, mutable: bool, lifetime: Option<&str>) -> R<String> {
        self.rust_in(mutable, lifetime, false)
    }

    fn rust_in(&self, m: bool, lt: Option<&str>, under_ref: bool) -> R<String> {
        let list = |v: &[Ty]| -> R<String> {
            let parts: R<Vec<String>> = v.iter().map(|t| t.rust_in(m, lt, false)).collect();
            Ok(parts?.join(", "))
        };
        Ok(match self {
            Ty::Prim(p) => p.clone(),
            Ty::Text if under_ref => "str".into(),
            Ty::Text => "String".into(),
            Ty::Char => "char".into(),
            Ty::Name(n) => n.clone(),
            Ty::App(n, a) => format!("{n}<{}>", list(a)?),
            Ty::Ref(t) => {
                let mut s = String::from("&");
                if let Some(l) = lt {
                    let _ = write!(s, "{l} ");
                }
                if m {
                    s.push_str("mut ");
                }
                s + &t.rust_in(m, lt, true)?
            }
            Ty::Opt(t) => format!("Option<{}>", t.rust_in(m, lt, false)?),
            Ty::Tuple(v) if v.len() == 1 => format!("({},)", list(v)?),
            Ty::Tuple(v) => format!("({})", list(v)?),
            Ty::Coll(k, _) => return Err(format!("slot missing: host type name of `{k}<..>` (`as`)")),
            Ty::Refined(t, _) => t.rust_in(m, lt, under_ref)?,
            Ty::As(t, w) => match (&**t, w.as_str()) {
                (_, "usize" | "isize" | "char") => w.clone(),
                (Ty::Text, "String") => "String".into(),
                // a slice is a `list` whose host form is the unsized `[T]`, which only a reference can name
                (Ty::Coll(k, a), "std::primitive::slice") if k == "list" => format!("[{}]", list(a)?),
                (Ty::Coll(_, a), _) => format!("{w}<{}>", list(a)?),
                (t, _) => format!("{w}<{}>", t.rust_in(m, lt, false)?),
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
    fn args(&mut self) -> R<Vec<Ty>> {
        let mut v = vec![self.ty()?];
        while self.eat(",") {
            v.push(self.ty()?);
        }
        self.expect(">")?;
        Ok(v)
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
                let on = if matches!(t, Ty::Char) { "and " } else { "where " };
                if !self.eat(on) {
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
                while self.eat(",") {
                    if self.eat(")") {
                        return Ok(Ty::Tuple(v));
                    }
                    v.push(self.ty()?);
                }
                self.expect(")")?;
            }
            return Ok(Ty::Tuple(v));
        }
        let id = self.ident();
        if id.is_empty() {
            return Err(format!("bad type at {}", self.i));
        }
        let generic = self.s.get(self.i) == Some(&b'<');
        if generic {
            self.i += 1;
            let a = self.args()?;
            return Ok(match id.as_str() {
                "ref" if a.len() == 1 => Ty::Ref(Box::new(a.into_iter().next().unwrap())),
                "list" | "map" | "set" => Ty::Coll(id, a),
                _ => Ty::App(id, a),
            });
        }
        if id == "text" {
            let save = self.i;
            if self.eat("where length(value) == 1") {
                return Ok(Ty::Char);
            }
            self.i = save;
            return Ok(Ty::Text);
        }
        Ok(if is_prim(&id) { Ty::Prim(id) } else { Ty::Name(id) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_is_a_list_whose_host_form_is_unsized() {
        let t = Ty::parse("ref<list<T> as 'std::primitive::slice'>").unwrap();
        assert_eq!(t.rust(false, Some("'p0")).unwrap(), "&'p0 [T]");
        assert_eq!(t.rust(true, Some("'p0")).unwrap(), "&'p0 mut [T]");
        assert_eq!(t.show(), "ref<list<T> as 'std::primitive::slice'>");
        assert!(t.has_ref());
        // a Vec is the same `list` with another host form
        assert_eq!(Ty::parse("list<u8> as 'std::vec::Vec'").unwrap().rust(false, None).unwrap(), "std::vec::Vec<u8>");
    }

    #[test]
    fn a_refinement_is_kept_in_the_string_and_erased_in_rust() {
        let s = r"text where length(value) == 1 and value matches /^\x{10FFFF}$/ as 'char'";
        let t = Ty::parse(s).unwrap();
        assert_eq!(t.show(), s);
        assert_eq!(t.rust(false, None).unwrap(), "char");
        assert_eq!(Ty::parse("text where length(value) <= 8").unwrap().show(), "text where length(value) <= 8");
        assert!(Ty::parse("text where ").is_err());
        // the char form alone is still a char
        assert_eq!(Ty::parse("text where length(value) == 1 as 'char'").unwrap(), Ty::As(Box::new(Ty::Char), "char".into()));
    }
}

//! A light syntactic check of the clause text of a library file: not a parser of language 15, but enough to catch
//! what a prefix check cannot. Brackets balance and quotes close; the clause starts with a keyword of its kind; a host
//! path is quoted (language 15's `STABLE`) and looks like one; a failure named by `fails` is declared in the file; and
//! every other name is a parameter, `result`, `self`, a generic, a variant of a subject of the file, a name the clause
//! binds (`all x`, `is V(x)`), or a word of the expression language.
use std::collections::{BTreeMap, BTreeSet};

/// What a clause may name besides the words of the language: the subject's own names and the file's.
#[derive(Default)]
pub struct Scope {
    /// Parameters, generics, `Self`, and the variants of the file's types.
    pub names: BTreeSet<String>,
    /// The failures the file declares.
    pub failures: BTreeSet<String>,
    /// The payload fields each failure declares: `fails F { field: e } when p` may fill only these.
    pub payload: BTreeMap<String, BTreeSet<String>>,
}

/// Words of language 4 and 6, and of the clause forms that carry no expression.
const WORDS: &[&str] = &[
    "and", "or", "not", "implies", "iff", "if", "then", "else", "exists", "is", "in", "matches", "fits", "finite", "all", "any", "old",
    "result", "self", "true", "false", "none", "value", "size", "count", "length", "byte_length", "utf16_length", "utf8", "sum", "min",
    "max", "same", "approx", "total_order", "distinct", "sorted", "by", "subset", "view", "called", "exactly", "at", "most", "least",
    "once", "before", "return", "only", "on", "when", "sign_bit", "is_nan", "payload", "caller", "some", "implements", "borrows",
    "owns", "shares", "from", "for", "as",
];

/// Words only a clause of one kind may use: what a `budget` bounds and what a `layout` states (language 6.7, 6.1).
fn kind_words(kind: &str) -> &'static [&'static str] {
    match kind {
        "budget" => &["allocates", "retains", "work", "bytes", "sent", "round_trips", "per", "call", "instance", "frame", "B", "KB", "MB", "O", "p50", "p99"],
        "layout" => &["c", "packed", "transparent", "align"],
        _ => &[],
    }
}

/// Type words of language 3.4, which an associated type or a bound may use.
const TYPE_WORDS: &[&str] = &["int", "char", "text", "bytes", "decimal", "time", "instant", "duration", "list", "set", "map", "ref"];

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Quoted(String),
    /// `'a`, after `for` or another lifetime: a name here means a lifetime of the subject, not a host path.
    Lifetime(String),
    Punct(char),
    Other,
}

fn lex(t: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = t.chars().collect();
    let (mut i, mut out, mut open): (usize, Vec<Tok>, Vec<char>) = (0, vec![], vec![]);
    while i < cs.len() {
        let c = cs[i];
        let regex = c == '/' && matches!(out.last(), Some(Tok::Word(w)) if w == "matches");
        if c.is_whitespace() {
            i += 1;
        } else if c == '\'' && lifetime_at(&cs, i) && lifetime_context(&out) {
            let n = cs[i + 1..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').count();
            out.push(Tok::Lifetime(cs[i + 1..i + 1 + n].iter().collect()));
            i += n + 1;
        } else if c == '\'' || c == '"' || regex {
            let end = (i + 1..cs.len()).find(|&j| cs[j] == c && cs[j - 1] != '\\').ok_or(format!("unterminated `{c}` in `{t}`"))?;
            out.push(if c == '\'' { Tok::Quoted(cs[i + 1..end].iter().collect()) } else { Tok::Other });
            i = end + 1;
        } else if c.is_alphabetic() || c == '_' {
            let n = cs[i..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').count();
            let w: String = cs[i..i + n].iter().collect();
            if cs[i + n..].starts_with(&[':', ':']) {
                return Err(format!("host path `{w}::…` is not quoted: write it as `'std::…'` (language 15's STABLE)"));
            }
            out.push(Tok::Word(w));
            i += n;
        } else if c.is_ascii_digit() {
            // a number, which may have a fraction but ends before `..`
            while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_' || (cs[i] == '.' && cs.get(i + 1).is_some_and(char::is_ascii_digit))) {
                i += 1;
            }
            out.push(Tok::Other);
        } else {
            if "([{".contains(c) {
                open.push(c);
            } else if let Some(want) = [('(', ')'), ('[', ']'), ('{', '}')].iter().find(|p| p.1 == c).map(|p| p.0) {
                if open.pop() != Some(want) {
                    return Err(format!("unbalanced `{c}` in `{t}`"));
                }
            }
            out.push(Tok::Punct(c));
            i += 1;
        }
    }
    match open.last() {
        Some(o) => Err(format!("unclosed `{o}` in `{t}`")),
        None => Ok(out),
    }
}

/// A quote that opens a lifetime name no quote closes: `'a`, and not the host path `'std::x'` or `'v1'`.
fn lifetime_at(cs: &[char], i: usize) -> bool {
    let n = cs[i + 1..].iter().take_while(|x| x.is_alphanumeric() || **x == '_').count();
    n > 0 && !cs[i + 1].is_ascii_digit() && !matches!(cs.get(i + 1 + n), Some('\'' | ':'))
}

/// Where a lifetime may stand: after `for`, and in a list of lifetimes.
fn lifetime_context(out: &[Tok]) -> bool {
    match out.last() {
        Some(Tok::Word(w)) => w == "for",
        Some(Tok::Punct(',')) => matches!(out.len().checked_sub(2).and_then(|i| out.get(i)), Some(Tok::Lifetime(_))),
        _ => false,
    }
}

fn is_word(t: Option<&Tok>, w: &str) -> bool {
    matches!(t, Some(Tok::Word(x)) if x == w)
}

fn is_host_path(p: &str) -> bool {
    p.contains("::") && ["std::", "core::", "alloc::", "<"].iter().any(|r| p.starts_with(r))
}

/// Everything wrong with one clause's text. `heads` are the keywords its kind may start with (none: no check).
pub fn lint(kind: &str, text: &str, heads: &[&str], scope: &Scope) -> Vec<String> {
    let head = heads.iter().filter(|h| text == **h || text.starts_with(&format!("{h} ")) || text.starts_with(&format!("{h}:"))).max_by_key(|h| h.len());
    if !heads.is_empty() && head.is_none() {
        let first = text.split_whitespace().next().unwrap_or("");
        return vec![format!("a `{kind}` clause starts with {} , not `{first}`", heads.iter().map(|h| format!("`{h}`")).collect::<Vec<_>>().join(" or "))];
    }
    let toks = match lex(&text[head.map_or(0, |h| h.len())..]) {
        Ok(t) => t,
        Err(e) => return vec![e],
    };
    let mut errs = vec![];
    // `fails F when p` names a declared failure; `fails panic when p` is the one the Rust host has
    let failure = if kind == "panics" { Some("panic") } else if kind == "fails" { Some(toks.first().map_or("", |t| if let Tok::Word(w) = t { w } else { "" })) } else { None };
    if let Some(f) = failure.filter(|f| !scope.failures.contains(*f)) {
        errs.push(format!("failure `{f}` is not declared in the file's `failures`: `{text}`"));
    }
    // `fails F { field: e, … } when p` fills the payload F declares: its field names are not expressions
    let mut skip: BTreeSet<usize> = BTreeSet::new();
    let at = usize::from(kind == "fails");
    if toks.get(at) == Some(&Tok::Punct('{')) {
        let declared = failure.and_then(|f| scope.payload.get(f));
        let mut depth = 0;
        for (i, t) in toks.iter().enumerate().skip(at) {
            match t {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => depth -= 1,
                Tok::Word(w) if depth == 1 && toks.get(i + 1) == Some(&Tok::Punct(':')) && matches!(toks[i - 1], Tok::Punct('{' | ',')) => {
                    skip.insert(i);
                    if !declared.is_some_and(|d| d.contains(w)) {
                        errs.push(format!("`{w}` is not a payload field of failure `{}`: `{text}`", failure.unwrap_or("")));
                    }
                }
                _ => {}
            }
            if depth == 0 {
                break;
            }
        }
    }
    // a clause names itself: `ensure name :`, `law name :`, `callback name :`
    let named = matches!(toks.first(), Some(Tok::Word(_))) && toks.get(1) == Some(&Tok::Punct(':')) && kind != "fails";
    let extra = kind_words(kind);
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    // `implements 'I' { Item = T }` and `some 'I' { Item = T }` name the associated types of the interface
    let assoc_clause = kind == "implements" || toks.iter().any(|t| matches!(t, Tok::Word(w) if w == "some" || w == "any" || w == "implements"));
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Lifetime(l) if !scope.names.contains(l) => errs.push(format!("unknown lifetime `'{l}` in `{text}`")),
            Tok::Word(_) if assoc_clause && i > 0 && matches!(toks[i - 1], Tok::Punct('{' | ',')) && toks.get(i + 1) == Some(&Tok::Punct('=')) && toks.get(i + 2) != Some(&Tok::Punct('=')) => {}
            Tok::Quoted(p) if !is_host_path(p) => errs.push(format!("`'{p}'` is not a quoted host path (`'std::…'`)")),
            Tok::Word(w) => {
                // `all x : T : p` and `any x in c : p` bind x; `v is Variant(x)` binds x
                if i > 0 && (is_word(toks.get(i - 1), "all") || is_word(toks.get(i - 1), "any")) || (i > 2 && is_word(toks.get(i - 3), "is") && toks[i - 1] == Tok::Punct('(')) {
                    bound.insert(w);
                }
                let own = (named && i == 0) || (kind == "fails" && i == 0);
                let field = i > 0 && toks[i - 1] == Tok::Punct('.');
                if !(skip.contains(&i) || extra.contains(&w.as_str()) || WORDS.contains(&w.as_str()) || bound.contains(w.as_str()) || scope.names.contains(w) || own || field || (assoc_clause && (crate::types::is_prim(w) || TYPE_WORDS.contains(&w.as_str())))) {
                    errs.push(format!("unknown name `{w}` in `{text}`"));
                }
            }
            _ => {}
        }
    }
    errs
}

/// What a `given` item may name, for one subject: the generics with the methods their bounds are known to have, the
/// build settings the host profile lists, and whether the subject is a method of a trait (which has a default body).
pub struct GivenCtx<'a> {
    /// Each generic type parameter; `Some(methods)` when every bound of it is known to the file, `None` when a method cannot be checked.
    pub generics: BTreeMap<String, Option<BTreeSet<String>>>,
    /// `Self` of a trait, an impl or a type: a name that is not a generic.
    pub has_self: bool,
    pub trait_method: bool,
    pub builds: &'a BTreeSet<String>,
}

/// The effects and promises a `given` item may state of the implementations it names (language 6.12).
const GIVEN_PREDICATES: [&[&str]; 3] = [&["never", "panic"], &["terminate"], &["effects", "none"]];

/// Everything wrong with the `given` items of one clause. An item is `default`, `build SETTING`, or `P.m, P.n never panic |
/// terminate | effects none`: a bound's parameter, a method of that bound, and what its implementation promises.
pub fn lint_given(items: &[&str], ctx: &GivenCtx) -> Vec<String> {
    let mut errs = vec![];
    for item in items {
        let spaced = item.replace(',', " , ");
        let w: Vec<&str> = spaced.split_whitespace().collect();
        match w.as_slice() {
            [] => errs.push("an empty `given` item".to_string()),
            ["default"] if ctx.trait_method => {}
            ["default"] => errs.push(format!("`given default`: the subject is not a method of a trait, so it has no default body to name: `{item}`")),
            ["build", setting] => {
                if !ctx.builds.contains(*setting) {
                    errs.push(format!("`given build {setting}`: not a build setting of the host profile ({})", ctx.builds.iter().cloned().collect::<Vec<_>>().join(", ")));
                }
            }
            ["build", ..] => errs.push(format!("`given build` names one setting: `{item}`")),
            _ => {
                let Some(cut) = GIVEN_PREDICATES.iter().find(|p| w.len() > p.len() && w[w.len() - p.len()..] == ***p) else {
                    errs.push(format!("a `given` item is `default`, `build SETTING`, or methods then `never panic`, `terminate` or `effects none`: `{item}`"));
                    continue;
                };
                let refs = &w[..w.len() - cut.len()];
                for (i, r) in refs.iter().enumerate() {
                    if i % 2 == 1 {
                        if *r != "," {
                            errs.push(format!("`given` names methods separated by commas: `{item}`"));
                        }
                        continue;
                    }
                    let Some((g, m)) = r.split_once('.').filter(|(g, m)| !g.is_empty() && !m.is_empty() && !m.contains('.')) else {
                        errs.push(format!("`{r}` is not `Param.method` in `given {item}`"));
                        continue;
                    };
                    match ctx.generics.get(g) {
                        None if g == "Self" && ctx.has_self => {}
                        None => errs.push(format!("`given {item}`: `{g}` is not a generic type parameter of the subject")),
                        Some(Some(ms)) if !ms.contains(m) => errs.push(format!("`given {item}`: `{m}` is not a method of a bound of `{g}` (known: {})", ms.iter().cloned().collect::<Vec<_>>().join(", "))),
                        Some(_) => {}
                    }
                }
                if refs.len().is_multiple_of(2) {
                    errs.push(format!("`given` names methods separated by commas: `{item}`"));
                }
            }
        }
    }
    errs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope {
            names: ["k", "K", "V", "msg", "Self", "Less", "Greater", "Err", "Ok"].iter().map(|s| s.to_string()).collect(),
            failures: ["panic".to_string()].into(),
            payload: [("panic".to_string(), ["message".to_string(), "location".to_string()].into())].into(),
        }
    }

    fn bad(kind: &str, head: &str, text: &str) -> String {
        lint(kind, text, &[head], &scope()).join("; ")
    }

    #[test]
    fn well_formed_clauses_pass() {
        for (k, h, t) in [
            ("ensure", "ensure", "ensure lookup : result == self[k]"),
            ("ensure", "ensure", "ensure stored : all k2 in old(self) : k2 != k implies self[k2] == old(self[k2])"),
            ("law", "law", "law dual : all a : Self : all b : Self : ('std::cmp::Ord::cmp'(a, b) is Less) iff ('std::cmp::Ord::cmp'(b, a) is Greater)"),
            ("panics", "fails panic when", "fails panic when self is Err(e) and e == e"),
            ("effects", "calls only", "calls only 'std::cmp::Ord::max'"),
            ("panics", "fails panic", "fails panic { message: msg, location: caller } when self is Err(e)"),
            ("ensure", "ensure", "ensure unsigned : not sign_bit(result) and not is_nan(self)"),
            ("budget", "budget", "budget allocates at most 0 B per call when k == 0"),
            ("layout", "size", "size 0"),
            ("layout", "layout", "layout transparent"),
            ("callback", "callback", "callback f : called at most once before return"),
            ("ensure", "ensure", "ensure f : self matches /\\p{White_Space}/ and finite self implies result == (if self < 0.0 then -self else self)"),
        ] {
            assert_eq!(bad(k, h, t), "", "{t}");
        }
    }

    #[test]
    fn the_forms_of_interfaces_implementations_and_lifetimes_pass() {
        let mut sc = scope();
        sc.names.extend(["a", "b", "Item"].map(String::from));
        let ok = |kind: &str, head: &str, t: &str| assert_eq!(lint(kind, t, &[head], &sc), Vec::<String>::new(), "{t}");
        ok("implements", "implements", "implements 'std::iter::Iterator' { Item = char }, 'std::clone::Clone'");
        ok("implements", "implements", "implements 'std::convert::From<u8>' { Item = 'std::string::String', Other = list<u8> }");
        ok("hold", "hold", "hold : borrows for 'a");
        ok("hold", "hold", "hold : borrows for 'a, 'b");
        ok("hold", "hold", "hold result : borrows from self");
        ok("ensure", "ensure", "ensure ok : result is some 'std::iter::Iterator' { Item = u8 }");
        // an associated type's name is not a name to look up, a lifetime is looked up, and a host path is still quoted
        let bad = |kind: &str, head: &str, t: &str| lint(kind, t, &[head], &sc).join("; ");
        assert!(bad("hold", "hold", "hold : borrows for 'zz").contains("unknown lifetime `'zz`"));
        assert!(bad("implements", "implements", "implements 'std::iter::Iterator' { Item = nope }").contains("unknown name `nope`"));
        assert!(bad("implements", "implements", "implements std::iter::Iterator { Item = char }").contains("not quoted"));
        assert!(bad("implements", "implements", "implements 'x' { Item = char }").contains("not a quoted host path"));
        assert!(bad("ensure", "ensure", "ensure x : result == 'v1").contains("unterminated"), "a quote that is not after `for` is not a lifetime");
    }

    #[test]
    fn what_a_prefix_check_missed_is_caught() {
        assert!(bad("ensure", "ensure", "ensure x : (result == k").contains("unclosed"));
        assert!(bad("ensure", "ensure", "ensure x : result == k)").contains("unbalanced"));
        assert!(bad("ensure", "ensure", "ensure x : result == 'k").contains("unterminated"));
        assert!(bad("law", "law", "law x : std::cmp::Ord::cmp(a, b)").contains("not quoted"));
        assert!(bad("effects", "calls only", "calls only 'max'").contains("not a quoted host path"));
        assert!(bad("ensure", "ensure", "ensure x : nope == k").contains("unknown name `nope`"));
        assert!(bad("ensure", "ensure", "require x : true").contains("starts with `ensure`"));
        assert!(bad("fails", "fails", "fails Boom when true").contains("not declared"));
        assert!(lint("panics", "fails panic when true", &["fails panic when"], &Scope::default()).join(";").contains("not declared"));
        assert!(bad("panics", "fails panic", "fails panic { text: k } when true").contains("not a payload field of failure `panic`"));
        assert!(bad("panics", "fails panic", "fails panic { message: nope } when true").contains("unknown name `nope`"));
        assert!(bad("budget", "budget", "budget c at most").contains("unknown name `c`"));
        assert!(bad("layout", "layout", "layout allocates").contains("unknown name `allocates`"));
        assert!(bad("ensure", "ensure", "ensure x : allocates == k").contains("unknown name `allocates`"));
        // a name bound by a quantifier or a pattern is known only after it
        assert!(bad("ensure", "ensure", "ensure x : e == k and (self is Err(e))").contains("unknown name `e`"));
    }

    #[test]
    fn given_items_name_generics_methods_and_builds() {
        let builds: BTreeSet<String> = ["overflow_checks", "panic=unwind"].map(String::from).into();
        let ms: BTreeSet<String> = ["hash", "eq"].map(String::from).into();
        let ctx = GivenCtx { generics: [("Q".to_string(), Some(ms)), ("K".to_string(), None)].into(), has_self: false, trait_method: false, builds: &builds };
        let g = |items: &[&str]| lint_given(items, &ctx).join("; ");
        assert_eq!(g(&["Q.hash, Q.eq never panic", "K.anything terminate", "Q.eq effects none", "build overflow_checks", "build panic=unwind"]), "");
        assert!(g(&["Z.hash never panic"]).contains("`Z` is not a generic"));
        assert!(g(&["Q.nope never panic"]).contains("`nope` is not a method of a bound of `Q`"));
        assert!(g(&["build debug_assertions"]).contains("not a build setting"));
        assert!(g(&["default"]).contains("no default body"));
        assert!(g(&["Q.hash"]).contains("a `given` item is"));
        assert!(g(&["Q.hash Q.eq never panic"]).contains("separated by commas"));
        assert!(g(&["never panic"]).contains("a `given` item is"));
        assert!(g(&["Q never panic"]).contains("not `Param.method`"));
        let ctx = GivenCtx { trait_method: true, has_self: true, ..ctx };
        assert_eq!(lint_given(&["default", "Self.eq terminate"], &ctx), Vec::<String>::new());
    }
}

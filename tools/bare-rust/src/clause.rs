//! A light syntactic check of the clause text of a library file: not a parser of language 15, but enough to catch
//! what a prefix check cannot. Brackets balance and quotes close; the clause starts with a keyword of its kind; a host
//! path is quoted (language 15's `STABLE`) and looks like one; a failure named by `fails` is declared in the file; and
//! every other name is a parameter, `result`, `self`, a generic, a variant of a subject of the file, a name the clause
//! binds (`all x`, `is V(x)`), or a word of the expression language.
use std::collections::BTreeSet;

/// What a clause may name besides the words of the language: the subject's own names and the file's.
#[derive(Default)]
pub struct Scope {
    /// Parameters, generics, `Self`, and the variants of the file's types.
    pub names: BTreeSet<String>,
    /// The failures the file declares.
    pub failures: BTreeSet<String>,
}

/// Words of language 4 and 6, and of the clause forms that carry no expression.
const WORDS: &[&str] = &[
    "and", "or", "not", "implies", "iff", "if", "then", "else", "exists", "is", "in", "matches", "fits", "finite", "all", "any", "old",
    "result", "self", "true", "false", "none", "value", "size", "count", "length", "byte_length", "utf16_length", "utf8", "sum", "min",
    "max", "same", "approx", "total_order", "distinct", "sorted", "by", "subset", "view", "called", "exactly", "at", "most", "least",
    "once", "before", "return", "only", "on", "when",
];

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Quoted(String),
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
    // a clause names itself: `ensure name :`, `law name :`, `callback name :`
    let named = matches!(toks.first(), Some(Tok::Word(_))) && toks.get(1) == Some(&Tok::Punct(':')) && kind != "fails";
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Quoted(p) if !is_host_path(p) => errs.push(format!("`'{p}'` is not a quoted host path (`'std::…'`)")),
            Tok::Word(w) => {
                // `all x : T : p` and `any x in c : p` bind x; `v is Variant(x)` binds x
                if i > 0 && (is_word(toks.get(i - 1), "all") || is_word(toks.get(i - 1), "any")) || (i > 2 && is_word(toks.get(i - 3), "is") && toks[i - 1] == Tok::Punct('(')) {
                    bound.insert(w);
                }
                let own = (named && i == 0) || (kind == "fails" && i == 0);
                let field = i > 0 && toks[i - 1] == Tok::Punct('.');
                if !(WORDS.contains(&w.as_str()) || bound.contains(w.as_str()) || scope.names.contains(w) || own || field) {
                    errs.push(format!("unknown name `{w}` in `{text}`"));
                }
            }
            _ => {}
        }
    }
    errs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope {
            names: ["k", "K", "V", "Self", "Less", "Greater", "Err", "Ok"].iter().map(|s| s.to_string()).collect(),
            failures: ["panic".to_string()].into(),
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
            ("callback", "callback", "callback f : called at most once before return"),
            ("ensure", "ensure", "ensure f : self matches /\\p{White_Space}/ and finite self implies result == (if self < 0.0 then -self else self)"),
        ] {
            assert_eq!(bad(k, h, t), "", "{t}");
        }
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
        // a name bound by a quantifier or a pattern is known only after it
        assert!(bad("ensure", "ensure", "ensure x : e == k and (self is Err(e))").contains("unknown name `e`"));
    }
}

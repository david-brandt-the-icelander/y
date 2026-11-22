//! From source text to logical lines: each a statement or a declaration's head, with its tokens, its indentation, and
//! the documentation block and comments above it.
//!
//! A logical line goes on over the next physical lines while a bracket is open, and also while the line does not end
//! in `:` and the next line that is not blank or a comment is indented deeper than the logical line's first: so a
//! function's `where` clause, and a condition broken after `and`, belong to the line they continue.
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A name or a keyword.
    Ident,
    /// A number, with any letters that follow it: `6`, `0.2400`, `10s`, `1min`.
    Number,
    /// A string, `"…"`, its quotes included.
    Str,
    /// A character, `'_'`, its quotes included.
    Char,
    /// A string with a prefix, `path"…"` or `pattern"…"`.
    Prefixed,
    /// An operator or a bracket.
    Punct,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
    pub line: usize,
    /// Byte offsets in the source.
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub struct Comment {
    pub line: usize,
    pub text: String,
    /// Whether it is alone on its line, not after code.
    pub alone: bool,
    /// The blank lines right above it.
    pub blank: usize,
    pub indent: usize,
    /// What the nearest line above that is not blank is: `comment`, `head` (code that ends in `:`), `doc` (the rule
    /// that closes a documentation block) or `code`; `None` when there is none.
    pub after: Option<&'static str>,
}

/// The lines between two dashed rules, without the indentation of the rules.
#[derive(Clone, Debug)]
pub struct Doc {
    pub line: usize,
    pub end: usize,
    pub lines: Vec<(usize, String)>,
    /// The blank lines right above its opening rule.
    pub blank: usize,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub indent: usize,
    pub line: usize,
    /// The last physical line it takes.
    pub end: usize,
    pub toks: Vec<Token>,
    /// The documentation block right above it, if any.
    pub doc: Option<Doc>,
    /// The comment lines right above it (after its documentation block).
    pub comments: Vec<Comment>,
    /// The blank lines right above it.
    pub blank: usize,
    /// The blank lines right above it and the comment lines right above it: its `blank` when no comment is right
    /// above it.
    pub gap: usize,
}

impl Line {
    pub fn opens_block(&self) -> bool {
        self.toks.last().is_some_and(|t| t.kind == Kind::Punct && t.text == ":")
    }
}

/// A line of three or more dashes and nothing else, which opens or closes a documentation block.
pub fn is_rule(t: &str) -> bool {
    t.len() >= 3 && t.chars().all(|c| c == '-')
}

fn indent_of(l: &str) -> usize {
    l.len() - l.trim_start_matches(' ').len()
}

/// The logical lines of a source, and every comment in it.
pub fn lines(src: &str) -> Result<(Vec<Line>, Vec<Comment>), Error> {
    let mut phys: Vec<(usize, usize, &str)> = vec![];
    let mut off = 0;
    for (i, l) in src.split('\n').enumerate() {
        phys.push((i + 1, off, l.strip_suffix('\r').unwrap_or(l)));
        off += l.len() + 1;
    }
    let (mut out, mut all_comments) = (vec![], vec![]);
    let (mut doc, mut comments): (Option<Doc>, Vec<Comment>) = (None, vec![]);
    let mut i = 0;
    while i < phys.len() {
        let (no, _, text) = phys[i];
        let t = text.trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        if text.starts_with('\t') || text.trim_start_matches(' ').starts_with('\t') {
            return Err(Error::new(no, "a tab in the indentation: Y indents with spaces"));
        }
        if is_rule(t) {
            let close = (i + 1..phys.len()).find(|&j| is_rule(phys[j].2.trim()));
            let Some(j) = close else { return Err(Error::new(no, "a documentation block with no closing rule")) };
            let ind = indent_of(text);
            let lines = phys[i + 1..j]
                .iter()
                .map(|(n, _, l)| (*n, if indent_of(l) >= ind { l[ind..].to_string() } else { l.trim_start().to_string() }))
                .collect();
            doc = Some(Doc { line: no, end: phys[j].0, lines, blank: blanks(&phys, i) });
            comments.clear();
            i = j + 1;
            continue;
        }
        if let Some(rest) = t.strip_prefix("--") {
            let c = Comment { line: no, text: rest.trim().to_string(), alone: true, blank: 0, indent: 0, after: None };
            all_comments.push(c.clone());
            comments.push(c);
            i += 1;
            continue;
        }
        let indent = indent_of(text);
        let (mut toks, mut depth) = (vec![], 0i32);
        let mut end;
        loop {
            let (n, o, l) = phys[i];
            lex(l, n, o, &mut toks, &mut depth, &mut all_comments)?;
            end = n;
            i += 1;
            if depth > 0 {
                if i >= phys.len() {
                    return Err(Error::new(no, "a bracket that is not closed"));
                }
                continue;
            }
            if toks.last().is_some_and(|t: &Token| t.kind == Kind::Punct && t.text == ":") {
                break;
            }
            // the next line that is not blank or a comment, when it is indented deeper, continues this one
            let mut k = i;
            while k < phys.len() && (phys[k].2.trim().is_empty() || (phys[k].2.trim().starts_with("--") && !is_rule(phys[k].2.trim()))) {
                k += 1;
            }
            if k < phys.len() && !is_rule(phys[k].2.trim()) && indent_of(phys[k].2) > indent {
                for &(n, _, l) in &phys[i..k] {
                    if !l.trim().is_empty() {
                        all_comments.push(Comment { line: n, text: l.trim()[2..].trim().to_string(), alone: true, blank: 0, indent: 0, after: None });
                    }
                }
                i = k;
                continue;
            }
            break;
        }
        let first = no - 1;
        let (blank, gap) = (blanks(&phys, first), gap(&phys, first));
        out.push(Line { indent, line: no, end, toks, doc: doc.take(), comments: std::mem::take(&mut comments), blank, gap });
    }
    // each comment's layout: whether it is alone on its line, its indentation, and what is above it
    for c in all_comments.iter_mut() {
        let k = c.line - 1;
        let text = phys[k].2;
        c.alone = text.trim_start().starts_with("--");
        c.indent = indent_of(text);
        c.blank = blanks(&phys, k);
        let above = k - c.blank;
        c.after = (above > 0).then(|| {
            let t = phys[above - 1].2.trim();
            if is_rule(t) {
                "doc"
            } else if t.starts_with("--") {
                "comment"
            } else if t.ends_with(':') {
                "head"
            } else {
                "code"
            }
        });
    }
    Ok((out, all_comments))
}

/// The blank lines right above the physical line at index `i`.
fn blanks(phys: &[(usize, usize, &str)], i: usize) -> usize {
    (0..i).rev().take_while(|&k| phys[k].2.trim().is_empty()).count()
}

/// The blank lines right above the comment lines right above the physical line at index `i`.
fn gap(phys: &[(usize, usize, &str)], i: usize) -> usize {
    let mut k = i;
    while k > 0 && phys[k - 1].2.trim().starts_with("--") && !is_rule(phys[k - 1].2.trim()) {
        k -= 1;
    }
    blanks(phys, k)
}

/// The tokens of one physical line, appended to `toks`; a comment at its end goes to `comments`.
pub fn lex(l: &str, no: usize, off: usize, toks: &mut Vec<Token>, depth: &mut i32, comments: &mut Vec<Comment>) -> Result<(), Error> {
    let b = l.as_bytes();
    let mut i = 0;
    let push = |toks: &mut Vec<Token>, kind, s: usize, e: usize| toks.push(Token { kind, text: l[s..e].to_string(), line: no, start: off + s, end: off + e });
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\t' {
            i += 1;
        } else if c == b'-' && b.get(i + 1) == Some(&b'-') {
            comments.push(Comment { line: no, text: l[i + 2..].trim().to_string(), alone: false, blank: 0, indent: 0, after: None });
            break;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            if b.get(i) == Some(&b'"') {
                i = string_end(l, i, no)?;
                push(toks, Kind::Prefixed, s, i);
            } else {
                // a region's or a task's address, `f#txn`, in a test's `faults`: one token
                if b.get(i) == Some(&b'#') && b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_') {
                    i += 1;
                    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                        i += 1;
                    }
                }
                push(toks, Kind::Ident, s, i);
            }
        } else if c.is_ascii_digit() {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            while i < b.len() && b[i].is_ascii_alphabetic() {
                i += 1;
            }
            push(toks, Kind::Number, s, i);
        } else if c == b'"' {
            let s = i;
            i = string_end(l, i, no)?;
            push(toks, Kind::Str, s, i);
        } else if c == b'\'' && char_end(b, i).is_some() {
            let e = char_end(b, i).unwrap();
            push(toks, Kind::Char, i, e);
            i = e;
        } else {
            let two = l.get(i..i + 2).unwrap_or("");
            let n = if ["==", "!=", "<=", ">=", ":="].contains(&two) { 2 } else { 1 };
            if n == 1 && !b"()[]{},:.=<>+-*/%'".contains(&c) {
                let ch = l[i..].chars().next().unwrap();
                return Err(Error::new(no, &format!("unexpected character `{ch}`")));
            }
            match c {
                b'(' | b'[' | b'{' => *depth += 1,
                b')' | b']' | b'}' => {
                    *depth -= 1;
                    if *depth < 0 {
                        return Err(Error::new(no, &format!("`{}` closes no bracket", c as char)));
                    }
                }
                _ => {}
            }
            push(toks, Kind::Punct, i, i + n);
            i += n;
        }
    }
    Ok(())
}

/// The end of the string that starts at `s` (its opening quote), past its closing quote.
fn string_end(l: &str, s: usize, no: usize) -> Result<usize, Error> {
    let b = l.as_bytes();
    let mut i = s + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Err(Error::new(no, "a string that is not closed on its line"))
}

/// The end of a character literal that starts at `s`: `'x'` or `'\x'`. `None` when the quote starts no literal, as in
/// a test's `covers f > a'many`.
fn char_end(b: &[u8], s: usize) -> Option<usize> {
    if b.get(s + 1) == Some(&b'\\') {
        return (b.get(s + 3) == Some(&b'\'')).then_some(s + 4);
    }
    // one character, which may take several bytes
    let rest = std::str::from_utf8(&b[s + 1..]).ok()?;
    let ch = rest.chars().next()?;
    if ch == '\'' {
        return None;
    }
    let e = s + 1 + ch.len_utf8();
    (b.get(e) == Some(&b'\'')).then_some(e + 1)
}

//! `matches(pattern"…")`: the regular expressions of a refined type's constraints and of `string.matches` (Y § 3.1,
//! § 3.3), as a small matcher.
//!
//! Y § 3.1 says a `pattern"…"` literal is a regular expression checked at compile time, and fixes no dialect. This is
//! the subset the lowering takes, and the Y compiler checks each literal against it with [`check`] while it builds:
//!
//! | Form | Matches |
//! |---|---|
//! | a character | itself; `\` makes any of `\ . [ ] ( ) | * + ? ^ $ -` itself |
//! | `.` | any one character |
//! | `[a-z0-9_]`, `[^…]` | one character in the class, or not in it: single characters and ranges |
//! | `\d`, `\w`, `\s` | an ASCII digit; an ASCII letter, digit or `_`; a space, tab or line end |
//! | `^`, `$` | the start, the end of the text |
//! | `e*`, `e+`, `e?` | `e` any number of times, at least once, at most once (greedy) |
//! | `a|b` | `a` or `b` |
//! | `(…)` | a group |
//!
//! The text is matched character by character (Unicode scalar values), and a pattern matches when it matches some part
//! of the text, as a regular expression's search does: `^` and `$` anchor it.

#[derive(Clone, Debug, PartialEq)]
enum Atom {
    Char(char),
    Any,
    /// A class: whether it is negated, and its ranges.
    Class(bool, Vec<(char, char)>),
    Group(Box<Node>),
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Repeat {
    One,
    Many,
    Some,
    Maybe,
}

/// Alternatives, each a sequence of atoms with how often each repeats.
#[derive(Clone, Debug, PartialEq)]
struct Node(Vec<Vec<(Atom, Repeat)>>);

struct Parser<'a> {
    cs: Vec<char>,
    i: usize,
    src: &'a str,
}

impl Parser<'_> {
    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("pattern\"{}\": {what} at character {}", self.src, self.i + 1))
    }

    fn alternatives(&mut self) -> Result<Node, String> {
        let mut alts = vec![self.sequence()?];
        while self.cs.get(self.i) == Some(&'|') {
            self.i += 1;
            alts.push(self.sequence()?);
        }
        Ok(Node(alts))
    }

    fn sequence(&mut self) -> Result<Vec<(Atom, Repeat)>, String> {
        let mut out = vec![];
        while let Some(&c) = self.cs.get(self.i) {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            let repeat = match self.cs.get(self.i) {
                Some('*') => Repeat::Many,
                Some('+') => Repeat::Some,
                Some('?') => Repeat::Maybe,
                _ => Repeat::One,
            };
            if repeat != Repeat::One {
                if matches!(atom, Atom::Start | Atom::End) {
                    return self.err("an anchor that repeats");
                }
                self.i += 1;
                if matches!(self.cs.get(self.i), Some('*' | '+' | '?' | '{')) {
                    return self.err("a repeat of a repeat, or a counted repeat, which the subset does not have");
                }
            }
            out.push((atom, repeat));
        }
        Ok(out)
    }

    fn escape(&mut self) -> Result<Atom, String> {
        self.i += 1;
        let Some(&c) = self.cs.get(self.i) else { return self.err("a `\\` at the end") };
        self.i += 1;
        Ok(match c {
            'd' => Atom::Class(false, vec![('0', '9')]),
            'w' => Atom::Class(false, vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')]),
            's' => Atom::Class(false, vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')]),
            '\\' | '.' | '[' | ']' | '(' | ')' | '|' | '*' | '+' | '?' | '^' | '$' | '-' | '/' => Atom::Char(c),
            _ => {
                self.i -= 1;
                return self.err(&format!("`\\{c}`, an escape the subset does not have"));
            }
        })
    }

    fn atom(&mut self) -> Result<Atom, String> {
        let c = self.cs[self.i];
        match c {
            '(' => {
                self.i += 1;
                let inner = self.alternatives()?;
                if self.cs.get(self.i) != Some(&')') {
                    return self.err("a `(` that is not closed");
                }
                self.i += 1;
                Ok(Atom::Group(Box::new(inner)))
            }
            '[' => {
                self.i += 1;
                let negated = self.cs.get(self.i) == Some(&'^');
                if negated {
                    self.i += 1;
                }
                let mut ranges = vec![];
                loop {
                    let Some(&c) = self.cs.get(self.i) else { return self.err("a `[` that is not closed") };
                    if c == ']' && !ranges.is_empty() {
                        self.i += 1;
                        break;
                    }
                    let lo = if c == '\\' {
                        match self.escape()? {
                            Atom::Char(c) => c,
                            Atom::Class(false, rs) => {
                                ranges.extend(rs);
                                continue;
                            }
                            _ => return self.err("an escape inside a class"),
                        }
                    } else {
                        self.i += 1;
                        c
                    };
                    if self.cs.get(self.i) == Some(&'-') && self.cs.get(self.i + 1).is_some_and(|c| *c != ']') {
                        let hi = self.cs[self.i + 1];
                        if hi < lo {
                            return self.err(&format!("the range `{lo}-{hi}`, which runs backwards"));
                        }
                        self.i += 2;
                        ranges.push((lo, hi));
                    } else {
                        ranges.push((lo, lo));
                    }
                }
                Ok(Atom::Class(negated, ranges))
            }
            '\\' => self.escape(),
            '.' => {
                self.i += 1;
                Ok(Atom::Any)
            }
            '^' => {
                self.i += 1;
                Ok(Atom::Start)
            }
            '$' => {
                self.i += 1;
                Ok(Atom::End)
            }
            '*' | '+' | '?' => self.err(&format!("a `{c}` that repeats nothing")),
            ']' | '{' | '}' => self.err(&format!("`{c}`, which the subset does not have unescaped")),
            _ => {
                self.i += 1;
                Ok(Atom::Char(c))
            }
        }
    }
}

fn parse(pattern: &str) -> Result<Node, String> {
    let mut p = Parser { cs: pattern.chars().collect(), i: 0, src: pattern };
    let n = p.alternatives()?;
    if p.i < p.cs.len() {
        return p.err("a `)` that closes no group");
    }
    Ok(n)
}

/// Whether a pattern is one the subset has. The Y compiler calls this on each `pattern"…"` while it builds, so that a
/// pattern outside it does not compile.
pub fn check(pattern: &str) -> Result<(), String> {
    parse(pattern).map(|_| ())
}

fn one(a: &Atom, t: &[char], at: usize) -> Option<usize> {
    let c = *t.get(at)?;
    let ok = match a {
        Atom::Char(x) => *x == c,
        Atom::Any => true,
        Atom::Class(neg, rs) => rs.iter().any(|(lo, hi)| *lo <= c && c <= *hi) != *neg,
        _ => false,
    };
    ok.then_some(at + 1)
}

/// The positions a node can end at, matched from `at`, each passed to `k` until it accepts one.
fn node(n: &Node, t: &[char], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    n.0.iter().any(|seq| sequence(seq, t, at, k))
}

fn sequence(seq: &[(Atom, Repeat)], t: &[char], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    let Some(((atom, repeat), rest)) = seq.split_first() else { return k(at) };
    let mut next = |p: usize| sequence(rest, t, p, k);
    match repeat {
        Repeat::One => single(atom, t, at, &mut next),
        Repeat::Maybe => single(atom, t, at, &mut next) || next(at),
        Repeat::Many | Repeat::Some => {
            // greedy: as many as match, then fewer; each pass must move, so an empty group cannot loop
            fn more(atom: &Atom, t: &[char], at: usize, count: usize, min: usize, next: &mut dyn FnMut(usize) -> bool) -> bool {
                let further = single(atom, t, at, &mut |p| p > at && more(atom, t, p, count + 1, min, next));
                further || (count >= min && next(at))
            }
            more(atom, t, at, 0, if *repeat == Repeat::Some { 1 } else { 0 }, &mut next)
        }
    }
}

fn single(a: &Atom, t: &[char], at: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match a {
        Atom::Start => at == 0 && k(at),
        Atom::End => at == t.len() && k(at),
        Atom::Group(g) => node(g, t, at, k),
        _ => one(a, t, at).is_some_and(|p| k(p)),
    }
}

/// Whether `text` matches `pattern`: whether the pattern matches some part of it. A pattern outside the subset is a
/// fault, as the compiler has checked each one.
pub fn matches(pattern: &str, text: &str) -> bool {
    let n = parse(pattern).unwrap_or_else(|e| crate::fault("y_runtime::pattern::matches", &e));
    let t: Vec<char> = text.chars().collect();
    (0..=t.len()).any(|start| node(&n, &t, start, &mut |_| true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_anchors_and_repeats() {
        assert!(matches("^[a-z]*$", "gallows"));
        assert!(matches("^[a-z]*$", ""));
        assert!(!matches("^[a-z]*$", "Gallows"));
        assert!(matches("^[a-z]+$", "a"));
        assert!(!matches("^[a-z]+$", ""));
        assert!(matches("^colou?r$", "color") && matches("^colou?r$", "colour") && !matches("^colou?r$", "colouur"));
        assert!(matches("^[^0-9]*$", "abc") && !matches("^[^0-9]*$", "a1"));
        assert!(matches("\\d\\d", "x42y") && !matches("^\\d\\d$", "x42y"));
        assert!(matches("^\\w+@\\w+\\.\\w+$", "a_b@c.de"));
        assert!(matches("^a.c$", "abc") && !matches("^a.c$", "ac"));
    }

    #[test]
    fn alternation_and_groups() {
        assert!(matches("^(cat|dog)s?$", "dogs") && matches("^(cat|dog)s?$", "cat") && !matches("^(cat|dog)s?$", "cow"));
        assert!(matches("^(ab)+$", "ababab") && !matches("^(ab)+$", "aba"));
        assert!(matches("^(a|ab)c$", "abc"));
        assert!(matches("^(a*)*$", "aaa"));
        assert!(matches("^[a-c-]+$", "a-b"));
    }

    #[test]
    fn patterns_outside_the_subset_are_refused() {
        for p in ["a{2}", "(a", "a)", "*a", "a**", "[z-a]", "[abc", "\\q", "^*"] {
            assert!(check(p).is_err(), "{p}");
        }
        assert!(check("^[a-z]*$").is_ok());
    }
}

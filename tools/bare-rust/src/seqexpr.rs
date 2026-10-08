//! The expression sublanguage that sequence, tuple, move and callback clauses use (language 6.13 to 6.15), parsed and
//! evaluated over `seq::Value`. It is a checker's evaluator for sampled runs, not a parser of language 15: a form it
//! does not know is `Unsupported`, and the clause is then counted unchecked, never failed.
use crate::seq::{named_map, named_pred, seq_eq, Seq, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Pat {
    /// `none`, `some(x)`, `Occupied(e)`, `Occupied`: a variant by name with names for its fields.
    Variant(String, Vec<String>),
    /// `(a, b)`, with `_` for a part not wanted.
    Tuple(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ex {
    Int(i64),
    Bool(bool),
    Str(String),
    Name(String),
    List(Vec<Ex>),
    Tuple(Vec<Ex>),
    Call(String, Vec<Ex>),
    Index(Box<Ex>, Box<Ex>),
    Slice(Box<Ex>, Option<Box<Ex>>, Option<Box<Ex>>),
    Field(Box<Ex>, usize),
    Bin(&'static str, Box<Ex>, Box<Ex>),
    Not(Box<Ex>),
    Neg(Box<Ex>),
    If(Box<Ex>, Box<Ex>, Box<Ex>),
    Exists(Box<Ex>),
    Is(Box<Ex>, Pat),
    Old(Box<Ex>),
    Moved(Box<Ex>, Box<Ex>),
    Final(Box<Ex>),
}

#[derive(Debug, PartialEq)]
pub enum EvalErr {
    /// No finite prefix decides (an infinite sequence), or an operator with no value there.
    Undecided(String),
    /// A form this evaluator does not know.
    Unsupported(String),
    /// Refused at build by the language (`.2` of a pair, a pattern of the wrong length).
    Build(String),
    /// A name with no value here: a bug in the clause where it is evaluated.
    Unbound(String),
}

impl EvalErr {
    pub fn text(&self) -> String {
        match self {
            EvalErr::Undecided(s) => format!("undecided: {s}"),
            EvalErr::Unsupported(s) => format!("unsupported: {s}"),
            EvalErr::Build(s) => format!("refused at build: {s}"),
            EvalErr::Unbound(s) => format!("no value for `{s}` here"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Int(i64),
    Str(String),
    Word(String),
    Quoted(String),
    Sym(&'static str),
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let st = i;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            let t: String = cs[st..i].iter().collect();
            out.push(Tok::Int(t.parse().map_err(|_| format!("number {t}"))?));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            out.push(Tok::Word(cs[st..i].iter().collect()));
        } else if c == '"' {
            let st = i + 1;
            i += 1;
            while i < cs.len() && cs[i] != '"' {
                i += 1;
            }
            if i >= cs.len() {
                return Err("an unclosed string".into());
            }
            out.push(Tok::Str(cs[st..i].iter().collect()));
            i += 1;
        } else if c == '\'' {
            let st = i + 1;
            i += 1;
            while i < cs.len() && cs[i] != '\'' {
                i += 1;
            }
            if i >= cs.len() {
                return Err("an unclosed quoted name".into());
            }
            out.push(Tok::Quoted(cs[st..i].iter().collect()));
            i += 1;
        } else {
            let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
            let sym = match two.as_str() {
                "==" => Some("=="),
                "!=" => Some("!="),
                "<=" => Some("<="),
                ">=" => Some(">="),
                "++" => Some("++"),
                ".." => Some(".."),
                _ => None,
            };
            if let Some(s) = sym {
                out.push(Tok::Sym(s));
                i += 2;
                continue;
            }
            let one = match c {
                '(' => "(",
                ')' => ")",
                '[' => "[",
                ']' => "]",
                ',' => ",",
                '.' => ".",
                '<' => "<",
                '>' => ">",
                '+' => "+",
                '-' => "-",
                '*' => "*",
                _ => return Err(format!("unexpected `{c}`")),
            };
            out.push(Tok::Sym(one));
            i += 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Parser

struct P {
    t: Vec<Tok>,
    i: usize,
}

type PR<T> = Result<T, String>;

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(x)) if x == w)
    }
    fn sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }
    fn eat_word(&mut self, w: &str) -> bool {
        let r = self.word(w);
        if r {
            self.i += 1;
        }
        r
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        let r = self.sym(s);
        if r {
            self.i += 1;
        }
        r
    }
    fn expect_sym(&mut self, s: &str) -> PR<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(format!("`{s}` expected at token {}: {:?}", self.i, self.peek()))
        }
    }

    fn expr(&mut self) -> PR<Ex> {
        let l = self.or()?;
        if self.eat_word("implies") {
            let r = self.expr()?;
            return Ok(Ex::Bin("implies", Box::new(l), Box::new(r)));
        }
        if self.eat_word("iff") {
            let r = self.expr()?;
            return Ok(Ex::Bin("iff", Box::new(l), Box::new(r)));
        }
        Ok(l)
    }

    fn or(&mut self) -> PR<Ex> {
        let mut l = self.and()?;
        while self.eat_word("or") {
            l = Ex::Bin("or", Box::new(l), Box::new(self.and()?));
        }
        Ok(l)
    }

    fn and(&mut self) -> PR<Ex> {
        let mut l = self.not()?;
        while self.eat_word("and") {
            l = Ex::Bin("and", Box::new(l), Box::new(self.not()?));
        }
        Ok(l)
    }

    fn not(&mut self) -> PR<Ex> {
        if self.eat_word("not") {
            return Ok(Ex::Not(Box::new(self.not()?)));
        }
        self.cmp()
    }

    fn cmp(&mut self) -> PR<Ex> {
        let l = self.concat()?;
        for op in ["==", "!=", "<=", ">=", "<", ">"] {
            if self.eat_sym(op) {
                let r = self.concat()?;
                let op: &'static str = match op {
                    "==" => "==",
                    "!=" => "!=",
                    "<=" => "<=",
                    ">=" => ">=",
                    "<" => "<",
                    _ => ">",
                };
                return Ok(Ex::Bin(op, Box::new(l), Box::new(r)));
            }
        }
        if self.eat_word("is") {
            return Ok(Ex::Is(Box::new(l), self.pat()?));
        }
        if self.word("moved") {
            self.i += 1;
            if !self.eat_word("from") {
                return Err("`moved from` expected".into());
            }
            return Ok(Ex::Moved(Box::new(l), Box::new(self.concat()?)));
        }
        Ok(l)
    }

    fn pat(&mut self) -> PR<Pat> {
        if self.eat_sym("(") {
            let mut names = vec![];
            loop {
                match self.peek().cloned() {
                    Some(Tok::Word(w)) => {
                        self.i += 1;
                        names.push(w);
                    }
                    t => return Err(format!("a name in a pattern, not {t:?}")),
                }
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            return Ok(Pat::Tuple(names));
        }
        let Some(Tok::Word(n)) = self.peek().cloned() else { return Err("a pattern".into()) };
        self.i += 1;
        let mut names = vec![];
        if self.eat_sym("(") {
            loop {
                match self.peek().cloned() {
                    Some(Tok::Word(w)) => {
                        self.i += 1;
                        names.push(w);
                    }
                    t => return Err(format!("a name in a pattern, not {t:?}")),
                }
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
        }
        Ok(Pat::Variant(n, names))
    }

    fn concat(&mut self) -> PR<Ex> {
        let mut l = self.add()?;
        while self.eat_sym("++") {
            l = Ex::Bin("++", Box::new(l), Box::new(self.add()?));
        }
        Ok(l)
    }

    fn add(&mut self) -> PR<Ex> {
        let mut l = self.unary()?;
        loop {
            let op = if self.eat_sym("+") {
                "+"
            } else if self.eat_sym("-") {
                "-"
            } else if self.eat_sym("*") {
                "*"
            } else {
                return Ok(l);
            };
            l = Ex::Bin(op, Box::new(l), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> PR<Ex> {
        if self.eat_sym("-") {
            return Ok(Ex::Neg(Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> PR<Ex> {
        let mut e = self.primary()?;
        loop {
            if self.eat_sym("[") {
                if self.eat_sym("..") {
                    let j = if self.sym("]") { None } else { Some(Box::new(self.add()?)) };
                    self.expect_sym("]")?;
                    e = Ex::Slice(Box::new(e), None, j);
                    continue;
                }
                let i = self.add()?;
                if self.eat_sym("..") {
                    let j = if self.sym("]") { None } else { Some(Box::new(self.add()?)) };
                    self.expect_sym("]")?;
                    e = Ex::Slice(Box::new(e), Some(Box::new(i)), j);
                } else {
                    self.expect_sym("]")?;
                    e = Ex::Index(Box::new(e), Box::new(i));
                }
            } else if self.sym(".") && matches!(self.t.get(self.i + 1), Some(Tok::Int(_))) {
                self.i += 1;
                let Some(Tok::Int(k)) = self.peek().cloned() else { unreachable!() };
                self.i += 1;
                e = Ex::Field(Box::new(e), k as usize);
            } else {
                return Ok(e);
            }
        }
    }

    fn args(&mut self) -> PR<Vec<Ex>> {
        let mut a = vec![];
        if self.eat_sym(")") {
            return Ok(a);
        }
        loop {
            a.push(self.expr()?);
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(a)
    }

    fn primary(&mut self) -> PR<Ex> {
        let Some(t) = self.peek().cloned() else { return Err("an expression is expected".into()) };
        self.i += 1;
        match t {
            Tok::Int(i) => Ok(Ex::Int(i)),
            Tok::Str(s) => Ok(Ex::Str(s)),
            Tok::Quoted(q) => {
                self.expect_sym("(")?;
                Ok(Ex::Call(format!("'{q}'"), self.args()?))
            }
            Tok::Sym("(") => {
                let first = self.expr()?;
                if self.eat_sym(",") {
                    let mut v = vec![first];
                    loop {
                        v.push(self.expr()?);
                        if !self.eat_sym(",") {
                            break;
                        }
                    }
                    self.expect_sym(")")?;
                    return Ok(Ex::Tuple(v));
                }
                self.expect_sym(")")?;
                Ok(first)
            }
            Tok::Sym("[") => {
                let mut v = vec![];
                if !self.eat_sym("]") {
                    loop {
                        v.push(self.expr()?);
                        if !self.eat_sym(",") {
                            break;
                        }
                    }
                    self.expect_sym("]")?;
                }
                Ok(Ex::List(v))
            }
            Tok::Word(w) => match w.as_str() {
                "true" => Ok(Ex::Bool(true)),
                "false" => Ok(Ex::Bool(false)),
                "if" => {
                    let c = self.expr()?;
                    if !self.eat_word("then") {
                        return Err("`then` expected".into());
                    }
                    let a = self.expr()?;
                    if !self.eat_word("else") {
                        return Err("`else` expected".into());
                    }
                    let b = self.expr()?;
                    Ok(Ex::If(Box::new(c), Box::new(a), Box::new(b)))
                }
                "exists" => Ok(Ex::Exists(Box::new(self.postfix()?))),
                "final" if self.sym("(") => {
                    self.i += 1;
                    let e = self.expr()?;
                    self.expect_sym(")")?;
                    Ok(Ex::Final(Box::new(e)))
                }
                "old" if self.sym("(") => {
                    self.i += 1;
                    let e = self.expr()?;
                    self.expect_sym(")")?;
                    Ok(Ex::Old(Box::new(e)))
                }
                _ if self.sym("(") => {
                    self.i += 1;
                    Ok(Ex::Call(w, self.args()?))
                }
                _ => Ok(Ex::Name(w)),
            },
            t => Err(format!("unexpected {t:?}")),
        }
    }
}

/// Parse an expression of the clause language (the sublanguage above).
pub fn parse(s: &str) -> Result<Ex, String> {
    let mut p = P { t: lex(s)?, i: 0 };
    let e = p.expr()?;
    if p.i < p.t.len() {
        return Err(format!("unexpected {:?} after the expression", p.t[p.i]));
    }
    Ok(e)
}

/// The text after the first top-level `:` of a clause that names itself (`ensure name : expr`), or the text itself.
pub fn clause_body(text: &str) -> &str {
    match text.split_once(" : ") {
        Some((_, b)) => b,
        None => text,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Evaluator

/// Names at the point of the clause (`now`) and at entry (`old`).
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub now: HashMap<String, Value>,
    pub old: HashMap<String, Value>,
    /// Names at the end of a run (`final`).
    pub fin: HashMap<String, Value>,
    in_old: bool,
    in_final: bool,
}

impl Env {
    pub fn new(now: HashMap<String, Value>, old: HashMap<String, Value>) -> Env {
        Env { now, old, fin: HashMap::new(), in_old: false, in_final: false }
    }

    fn get(&self, n: &str) -> Option<&Value> {
        if self.in_final {
            self.fin.get(n).or_else(|| self.now.get(n))
        } else if self.in_old {
            self.old.get(n).or_else(|| self.now.get(n))
        } else {
            self.now.get(n)
        }
    }

    pub fn has(&self, n: &str) -> bool {
        self.now.contains_key(n)
    }
}

type ER = Result<Value, EvalErr>;

fn boolean(v: Value, what: &str) -> Result<bool, EvalErr> {
    match v {
        Value::Bool(b) => Ok(b),
        v => Err(EvalErr::Unsupported(format!("{what} is not a boolean: {v:?}"))),
    }
}

fn list_of(v: Value, what: &str) -> Result<Seq, EvalErr> {
    match v {
        Value::List(s) => Ok(s),
        Value::Str(s) => Ok(Seq::scalars(&s)),
        v => Err(EvalErr::Unsupported(format!("{what} is not a list: {v:?}"))),
    }
}

fn int_of(v: Value, what: &str) -> Result<i64, EvalErr> {
    match v {
        Value::Int(i) => Ok(i),
        Value::Some(b) => int_of(*b, what),
        v => Err(EvalErr::Unsupported(format!("{what} is not an integer: {v:?}"))),
    }
}

fn ident(e: &Ex) -> Option<&str> {
    if let Ex::Name(n) = e {
        Some(n)
    } else {
        None
    }
}

pub fn eval(e: &Ex, env: &mut Env) -> ER {
    match e {
        Ex::Int(i) => Ok(Value::Int(*i)),
        Ex::Bool(b) => Ok(Value::Bool(*b)),
        Ex::Str(s) => Ok(Value::Str(s.clone())),
        Ex::Name(n) => match n.as_str() {
            "none" => Ok(Value::None),
            _ => env.get(n).cloned().ok_or_else(|| EvalErr::Unbound(n.clone())),
        },
        Ex::List(v) => Ok(Value::list(v.iter().map(|x| eval(x, env)).collect::<Result<_, _>>()?)),
        Ex::Tuple(v) => Ok(Value::Tuple(v.iter().map(|x| eval(x, env)).collect::<Result<_, _>>()?)),
        Ex::Field(x, k) => match eval(x, env)? {
            Value::Tuple(p) if *k < p.len() => Ok(p[*k].clone()),
            Value::Tuple(p) => Err(EvalErr::Build(format!("a tuple of {} has no component {k}", p.len()))),
            v => Err(EvalErr::Unsupported(format!("`.{k}` of {v:?}"))),
        },
        Ex::Index(x, i) => {
            let xs = list_of(eval(x, env)?, "an indexed value")?;
            Ok(xs.index(int_of(eval(i, env)?, "an index")?))
        }
        Ex::Slice(x, i, j) => {
            let xs = list_of(eval(x, env)?, "a sliced value")?;
            let mut end = |b: &Option<Box<Ex>>| -> Result<Option<i64>, EvalErr> { b.as_ref().map(|b| int_of(eval(b, env)?, "a slice end")).transpose() };
            let (i, j) = (end(i)?, end(j)?);
            Ok(Value::List(xs.slice(i, j)))
        }
        Ex::Neg(x) => Ok(Value::Int(-int_of(eval(x, env)?, "a negated value")?)),
        Ex::Not(x) => Ok(Value::Bool(!boolean(eval(x, env)?, "`not`")?)),
        Ex::If(c, a, b) => {
            if boolean(eval(c, env)?, "an `if` condition")? {
                eval(a, env)
            } else {
                eval(b, env)
            }
        }
        Ex::Exists(x) => Ok(Value::Bool(!matches!(eval(x, env)?, Value::None))),
        Ex::Old(x) => {
            let saved = env.in_old;
            env.in_old = true;
            let r = eval(x, env);
            env.in_old = saved;
            r
        }
        Ex::Final(x) => {
            let saved = env.in_final;
            env.in_final = true;
            let r = eval(x, env);
            env.in_final = saved;
            r
        }
        Ex::Is(x, pat) => {
            let v = eval(x, env)?;
            Ok(Value::Bool(bind(pat, v, env)?))
        }
        Ex::Moved(a, b) => {
            let a = eval(a, env)?;
            let saved = env.in_old;
            env.in_old = true;
            let b = eval(b, env);
            env.in_old = saved;
            match (a, b?) {
                (Value::Ent(_, x), Value::Ent(_, y)) | (Value::Id(x), Value::Id(y)) => Ok(Value::Bool(x == y)),
                (a, b) => Err(EvalErr::Unsupported(format!("`moved from` of {a:?} and {b:?}: they carry no identity"))),
            }
        }
        Ex::Bin(op, a, b) => match *op {
            "and" => Ok(Value::Bool(boolean(eval(a, env)?, "`and`")? && boolean(eval(b, env)?, "`and`")?)),
            "or" => Ok(Value::Bool(boolean(eval(a, env)?, "`or`")? || boolean(eval(b, env)?, "`or`")?)),
            "implies" => Ok(Value::Bool(!boolean(eval(a, env)?, "`implies`")? || boolean(eval(b, env)?, "`implies`")?)),
            "iff" => Ok(Value::Bool(boolean(eval(a, env)?, "`iff`")? == boolean(eval(b, env)?, "`iff`")?)),
            _ => {
                let (x, y) = (eval(a, env)?, eval(b, env)?);
                binary(op, x, y)
            }
        },
        Ex::Call(f, args) => call(f, args, env),
    }
}

fn binary(op: &str, x: Value, y: Value) -> ER {
    let decide = |r: Option<bool>| r.map(Value::Bool).ok_or_else(|| EvalErr::Undecided("no finite prefix decides the equality".into()));
    let order = |x: &Value, y: &Value| x.cmp_dec(y).ok_or_else(|| EvalErr::Unsupported(format!("no order between {x:?} and {y:?}")));
    match op {
        "==" => decide(x.eq_dec(&y)),
        "!=" => decide(x.eq_dec(&y).map(|b| !b)),
        "<=" => Ok(Value::Bool(order(&x, &y)?.is_le())),
        ">=" => Ok(Value::Bool(order(&x, &y)?.is_ge())),
        "<" => Ok(Value::Bool(order(&x, &y)?.is_lt())),
        ">" => Ok(Value::Bool(order(&x, &y)?.is_gt())),
        "++" => Ok(Value::List(list_of(x, "`++`")?.concat(&list_of(y, "`++`")?))),
        "+" | "-" | "*" => {
            let (a, b) = (int_of(x, "an operand")?, int_of(y, "an operand")?);
            Ok(Value::Int(match op {
                "+" => a.wrapping_add(b),
                "-" => a.wrapping_sub(b),
                _ => a.wrapping_mul(b),
            }))
        }
        _ => Err(EvalErr::Unsupported(format!("operator {op}"))),
    }
}

/// Match a pattern, binding its names in `env.now`.
fn bind(pat: &Pat, v: Value, env: &mut Env) -> Result<bool, EvalErr> {
    match pat {
        Pat::Tuple(names) => match v {
            Value::Tuple(parts) if parts.len() == names.len() => {
                for (n, p) in names.iter().zip(parts) {
                    if n != "_" {
                        env.now.insert(n.clone(), p);
                    }
                }
                Ok(true)
            }
            Value::Tuple(parts) => Err(EvalErr::Build(format!("a pattern of {} names against a tuple of {}", names.len(), parts.len()))),
            v => Err(EvalErr::Unsupported(format!("a tuple pattern against {v:?}"))),
        },
        Pat::Variant(n, names) => match (n.as_str(), v) {
            ("none", v) => Ok(matches!(v, Value::None)),
            ("some" | "Some", Value::Some(x)) if names.len() <= 1 => {
                if let Some(nm) = names.first() {
                    env.now.insert(nm.clone(), *x);
                }
                Ok(true)
            }
            ("some" | "Some", Value::None) => Ok(false),
            (_, Value::Variant(m, fields)) => {
                if *n != m {
                    return Ok(false);
                }
                if !names.is_empty() && names.len() != fields.len() {
                    return Err(EvalErr::Build(format!("variant {m} has {} fields, the pattern {}", fields.len(), names.len())));
                }
                for (nm, f) in names.iter().zip(fields) {
                    if nm != "_" {
                        env.now.insert(nm.clone(), f);
                    }
                }
                Ok(true)
            }
            (_, v) => Err(EvalErr::Unsupported(format!("pattern {n} against {v:?}"))),
        },
    }
}

fn call(f: &str, args: &[Ex], env: &mut Env) -> ER {
    let ev = |i: usize, env: &mut Env| -> ER { eval(args.get(i).ok_or_else(|| EvalErr::Build(format!("`{f}` needs argument {i}")))?, env) };
    let arity = |n: usize| -> Result<(), EvalErr> {
        if args.len() == n {
            Ok(())
        } else {
            Err(EvalErr::Build(format!("`{f}` takes {n} arguments, not {}", args.len())))
        }
    };
    match f {
        "size" | "count" | "length" => {
            arity(1)?;
            let xs = list_of(ev(0, env)?, "the argument of `size`")?;
            xs.len().map(|n| Value::Int(n as i64)).ok_or_else(|| EvalErr::Undecided("`size` of an infinite sequence has no value".into()))
        }
        "take" | "skip" => {
            arity(2)?;
            let n = int_of(ev(0, env)?, "a count")?;
            let xs = list_of(ev(1, env)?, "a sequence")?;
            Ok(Value::List(if f == "take" { xs.take(n) } else { xs.skip(n) }))
        }
        "reverse" => {
            arity(1)?;
            let xs = list_of(ev(0, env)?, "a sequence")?;
            xs.reverse().map(Value::List).ok_or_else(|| EvalErr::Undecided("`reverse` of an infinite sequence has no value".into()))
        }
        "zip" => {
            arity(2)?;
            Ok(Value::List(list_of(ev(0, env)?, "a sequence")?.zip(&list_of(ev(1, env)?, "a sequence")?)))
        }
        "enumerate" => {
            arity(1)?;
            Ok(Value::List(list_of(ev(0, env)?, "a sequence")?.enumerate()))
        }
        "scalars" => {
            arity(1)?;
            match ev(0, env)? {
                Value::Str(s) => Ok(Value::List(Seq::scalars(&s))),
                v => Err(EvalErr::Unsupported(format!("`scalars` of {v:?}"))),
            }
        }
        "map" | "filter" => {
            arity(2)?;
            let name = ident(&args[0]).ok_or_else(|| EvalErr::Build(format!("`{f}` takes the name of a function")))?;
            let xs = list_of(ev(1, env)?, "a sequence")?;
            if f == "map" {
                let m = named_map(name).ok_or_else(|| EvalErr::Unsupported(format!("named function `{name}`")))?;
                Ok(Value::List(xs.map(m)))
            } else {
                let p = named_pred(name).ok_or_else(|| EvalErr::Unsupported(format!("named predicate `{name}`")))?;
                Ok(Value::List(xs.filter(p)))
            }
        }
        // the view of an iterator is the list the harness bound; a collection's items are its list
        "present" => {
            arity(2)?;
            let xs = list_of(ev(0, env)?, "a sequence")?;
            Ok(Value::Bool(xs.present(int_of(ev(1, env)?, "a position")?)))
        }
        "select" | "while_true" => {
            arity(2)?;
            let (bs, xs) = (list_of(ev(0, env)?, "a list of booleans")?, list_of(ev(1, env)?, "a sequence")?);
            Ok(Value::List(if f == "select" { bs.select(&xs) } else { bs.while_true(&xs) }))
        }
        "flatten" => {
            arity(1)?;
            let xss = list_of(ev(0, env)?, "a list of lists")?;
            match xss.flatten() {
                Some(x) => Ok(Value::List(x)),
                None if xss.len().is_none() => Err(EvalErr::Undecided("`flatten` of an infinite list has no value".into())),
                None => Err(EvalErr::Unsupported("`flatten` of a list whose items are not lists".into())),
            }
        }
        "yields" | "deref" | "current" => {
            arity(1)?;
            ev(0, env)
        }
        "same" => {
            arity(2)?;
            match (ev(0, env)?, ev(1, env)?) {
                (Value::Id(a), Value::Id(b)) => Ok(Value::Bool(a == b)),
                (a, b) => Err(EvalErr::Unsupported(format!("`same` of {a:?} and {b:?}"))),
            }
        }
        q if q.starts_with('\'') && q.contains("into_iter") => {
            arity(1)?;
            ev(0, env)
        }
        _ => Err(EvalErr::Unsupported(format!("function `{f}`"))),
    }
}

/// A `let` clause: `let name = e` or `let (a, b) = e`, binding in `env.now`.
pub fn run_let(text: &str, env: &mut Env) -> Result<(), EvalErr> {
    let rest = text.strip_prefix("let ").ok_or_else(|| EvalErr::Unsupported("not a let".into()))?;
    let (lhs, rhs) = rest.split_once('=').ok_or_else(|| EvalErr::Unsupported("let without `=`".into()))?;
    let v = eval(&parse(rhs.trim()).map_err(EvalErr::Unsupported)?, env)?;
    let lhs = lhs.trim();
    if lhs.starts_with('(') {
        let pat = P { t: lex(lhs).map_err(EvalErr::Unsupported)?, i: 0 }.pat().map_err(EvalErr::Unsupported)?;
        bind(&pat, v, env)?;
    } else {
        if env.now.contains_key(lhs) {
            return Err(EvalErr::Build(format!("`{lhs}` is already bound")));
        }
        env.now.insert(lhs.to_string(), v);
    }
    Ok(())
}

/// Parse and evaluate to a boolean.
pub fn holds(text: &str, env: &mut Env) -> Result<bool, EvalErr> {
    let e = parse(text).map_err(EvalErr::Unsupported)?;
    boolean(eval(&e, env)?, "the clause")
}

/// Whether two sequences are equal on a prefix, for the harness's own use.
pub fn same_prefix(a: &Seq, b: &Seq) -> Option<bool> {
    seq_eq(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &[i64]) -> Value {
        Value::list(v.iter().map(|i| Value::Int(*i)).collect())
    }

    fn env(now: &[(&str, Value)], old: &[(&str, Value)]) -> Env {
        Env::new(now.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), old.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
    }

    #[test]
    fn clauses_of_the_library_file_evaluate() {
        let mut e = env(&[("result", ints(&[1, 2])), ("self", ints(&[1, 2, 3])), ("n", Value::Int(2))], &[("self", ints(&[1, 2, 3]))]);
        assert_eq!(holds("yields(result) == take(n, old(yields(self)))", &mut e), Ok(true));
        assert_eq!(holds("yields(result) == skip(n, old(yields(self)))", &mut e), Ok(false));
        let mut e = env(&[("self", ints(&[2, 3])), ("result", Value::some(Value::Int(1)))], &[("self", ints(&[1, 2, 3]))]);
        assert_eq!(holds("result == old(yields(self))[0]", &mut e), Ok(true));
        assert_eq!(holds("exists result implies yields(self) == old(yields(self))[1..]", &mut e), Ok(true));
    }

    #[test]
    fn the_added_operators_and_final_and_let() {
        let mut e = env(&[("keep", Value::list(vec![Value::Bool(true), Value::Bool(false)])), ("r", Value::Tuple(vec![ints(&[1]), ints(&[2])]))], &[("self", ints(&[5, 6, 7]))]);
        assert_eq!(holds("select(keep, old(yields(self))) == [5]", &mut e), Ok(true));
        assert_eq!(holds("while_true(keep, old(yields(self))) == [5]", &mut e), Ok(true));
        assert_eq!(holds("present(old(yields(self)), 2) and not present(old(yields(self)), 3)", &mut e), Ok(true));
        assert_eq!(holds("flatten([[1], [2, 3]]) == [1, 2, 3]", &mut e), Ok(true));
        e.fin.insert("self".into(), ints(&[7]));
        assert_eq!(holds("final(yields(self)) == skip(size(keep), old(yields(self)))", &mut e), Ok(true));
        assert_eq!(run_let("let (a, b) = r", &mut e), Ok(()));
        assert_eq!(holds("a ++ b == [1, 2]", &mut e), Ok(true));
        assert!(matches!(run_let("let a = 1", &mut e), Err(EvalErr::Build(_))));
    }

    #[test]
    fn tuple_parts_and_patterns() {
        let t = Value::Tuple(vec![ints(&[1]), ints(&[2, 3])]);
        let mut e = env(&[("result", t), ("mid", Value::Int(1))], &[("self", ints(&[1, 2, 3]))]);
        assert_eq!(holds("result.0 == old(self)[..mid] and result.1 == old(self)[mid..]", &mut e), Ok(true));
        assert_eq!(holds("result is (head, tail) implies size(head) + size(tail) == old(size(self))", &mut e), Ok(true));
        assert!(matches!(holds("size(result.2) == 0", &mut e), Err(EvalErr::Build(_))));
        assert!(matches!(holds("result is (a, b, c)", &mut e), Err(EvalErr::Build(_))));
    }

    #[test]
    fn infinite_sequences_are_refuted_never_confirmed() {
        let c = Value::List(Seq::counter(0, 1));
        let mut e = env(&[("result", c)], &[]);
        assert_eq!(holds("take(3, yields(result)) == [0, 1, 2]", &mut e), Ok(true));
        assert!(matches!(holds("yields(result) == yields(result)", &mut e), Err(EvalErr::Undecided(_))));
        assert!(matches!(holds("size(result) == 3", &mut e), Err(EvalErr::Undecided(_))));
        assert_eq!(holds("skip(1, result) == result", &mut e), Ok(false));
    }
}

//! From logical lines to the tree: lines nest by indentation into blocks, and each line is parsed by what it starts
//! with and the block it is in.
use crate::lex::{self, Doc, Kind, Line, Token};
use crate::tree::{Node, Value};
use crate::Error;

/// A line and the lines of the block it opens.
struct Entry<'a> {
    line: &'a Line,
    children: Vec<Entry<'a>>,
}

impl Entry<'_> {
    /// The last physical line the entry takes, its block's included.
    fn end(&self) -> usize {
        self.children.last().map_or(self.line.end, Entry::end)
    }
}

fn build<'a>(lines: &'a [Line], pos: &mut usize, indent: usize) -> Result<Vec<Entry<'a>>, Error> {
    let mut out = vec![];
    while *pos < lines.len() {
        let l = &lines[*pos];
        if l.indent < indent {
            break;
        }
        if l.indent > indent {
            let dedent = out.last().is_some_and(|e: &Entry| !e.children.is_empty());
            return Err(Error::new(l.line, if dedent { "indented less than the block above it, and more than the block that holds it" } else { "indented deeper than its block, and no line ending in `:` opens a block for it" }));
        }
        *pos += 1;
        let mut e = Entry { line: l, children: vec![] };
        if *pos < lines.len() && lines[*pos].indent > indent {
            if !l.opens_block() {
                return Err(Error::new(lines[*pos].line, "indented deeper than its block, and no line ending in `:` opens a block for it"));
            }
            let inner = lines[*pos].indent;
            e.children = build(lines, pos, inner)?;
        } else if l.opens_block() {
            return Err(Error::new(l.line, "a line ending in `:` opens a block, and no indented line follows it"));
        }
        out.push(e);
    }
    Ok(out)
}

thread_local! {
    /// The symbols of the units the file being parsed declares, which a number may carry: `12km`.
    static UNITS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The symbols a source's `unit` declarations give: `unit kilometre symbol km is 1000 metre`.
pub fn unit_symbols(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            (w.first() == Some(&"unit") && w.get(2) == Some(&"symbol")).then(|| w.get(3).map(|s| s.to_string())).flatten()
        })
        .collect()
}

/// Parses a Y source file: a module, its `main`, or its tests.
pub fn parse(src: &str) -> Result<Node, Error> {
    parse_with_units(src, &[])
}

/// As `parse`, where the file may also write the quantities of units another file declares: a test file, those of its
/// module (`crate::parse_file`).
pub fn parse_with_units(src: &str, more: &[String]) -> Result<Node, Error> {
    UNITS.with(|u| *u.borrow_mut() = unit_symbols(src).into_iter().chain(more.iter().cloned()).collect());
    let (lines, comments) = lex::lines(src)?;
    let mut pos = 0;
    let entries = build(&lines, &mut pos, 0)?;
    if pos < lines.len() {
        return Err(Error::new(lines[pos].line, "indented less than its block, and more than the block that holds it"));
    }
    let decls = decls(&entries, src, Context::File)?;
    let comments: Vec<Node> = comments.into_iter().map(|c| Node::new("comment", c.line).with("text", c.text).with("alone", c.alone).with("indent", c.indent).with("blank", c.blank).with("after", c.after)).collect();
    Ok(Node::new("file", 1).with("decls", decls).with("comments", comments))
}

/// A line's tokens, read from left to right.
struct P<'a> {
    t: &'a [Token],
    i: usize,
    src: &'a str,
    line: usize,
}

impl<'a> P<'a> {
    fn new(l: &'a Line, src: &'a str) -> P<'a> {
        P { t: &l.toks, i: 0, src, line: l.line }
    }
    fn peek(&self) -> Option<&'a Token> {
        self.t.get(self.i)
    }
    fn peek_at(&self, k: usize) -> Option<&'a Token> {
        self.t.get(self.i + k)
    }
    /// Whether the token `k` ahead is the word or the operator `s`.
    fn is_at(&self, k: usize, s: &str) -> bool {
        self.peek_at(k).is_some_and(|t| (t.kind == Kind::Ident || t.kind == Kind::Punct) && t.text == s)
    }
    fn is(&self, s: &str) -> bool {
        self.is_at(0, s)
    }
    fn ident_at(&self, k: usize) -> bool {
        self.peek_at(k).is_some_and(|t| t.kind == Kind::Ident)
    }
    fn done(&self) -> bool {
        self.i >= self.t.len()
    }
    /// The line of the next token, or of the last one when there is none.
    fn here(&self) -> usize {
        self.peek().or(self.t.last()).map_or(self.line, |t| t.line)
    }
    fn err<T>(&self, what: &str) -> Result<T, Error> {
        let found = self.peek().map_or("the end of the line".to_string(), |t| format!("`{}`", t.text));
        Err(Error::new(self.here(), &format!("expected {what}, found {found}")))
    }
    fn next(&mut self) -> &'a Token {
        self.i += 1;
        &self.t[self.i - 1]
    }
    fn eat(&mut self, s: &str) -> bool {
        let yes = self.is(s);
        if yes {
            self.i += 1;
        }
        yes
    }
    fn expect(&mut self, s: &str) -> Result<(), Error> {
        if self.eat(s) {
            Ok(())
        } else {
            self.err(&format!("`{s}`"))
        }
    }
    /// After `else`: an `else` names its outcome, and does not open an `if` (Y § 3.5).
    fn else_named(&self) -> Result<(), Error> {
        if self.is("if") {
            return Err(Error::new(self.here(), "an `else if` ladder: write a `conditional`, which names each outcome"));
        }
        if self.done() || self.is(":") {
            return Err(Error::new(self.here(), "an `else` without a name: name the outcome it stands for"));
        }
        Ok(())
    }

    fn ident(&mut self) -> Result<String, Error> {
        if self.ident_at(0) {
            Ok(self.next().text.clone())
        } else {
            self.err("a name")
        }
    }
    /// A name with dots: `standard.console`, `word_list_error.empty`.
    fn dotted(&mut self) -> Result<String, Error> {
        let mut s = self.ident()?;
        while self.is(".") && self.ident_at(1) {
            self.i += 1;
            s = format!("{s}.{}", self.ident()?);
        }
        Ok(s)
    }
    /// The end of the line: a `:` when it opens a block, then nothing.
    fn finish(&mut self, block: bool) -> Result<(), Error> {
        // a head that lacks its `:` takes the deeper line below it as its continuation: the error is the head's
        if block && !self.is(":") && self.i > 0 && self.peek().is_some_and(|t| t.line > self.t[self.i - 1].line) {
            return Err(Error::new(self.t[self.i - 1].line, "expected `:` at the end of the line"));
        }
        if block {
            self.expect(":")?;
        }
        if !self.done() {
            return self.err(if block { "the end of the line" } else { "the end of the statement" });
        }
        Ok(())
    }
    /// The source text from token `a` up to, not including, token `b`.
    fn text(&self, a: usize, b: usize) -> String {
        if a >= b {
            return String::new();
        }
        self.src[self.t[a].start..self.t[b - 1].end].to_string()
    }

    // expressions, from the loosest binding to the tightest

    fn expr(&mut self) -> Result<Node, Error> {
        let mut left = self.and_expr()?;
        left = self.claim(left)?;
        while self.is("or") {
            let line = self.next().line;
            if self.eat("fail") {
                left = Node::new("or_fail", line).with("value", left);
            } else {
                let right = self.and_expr()?;
                left = Node::new("binary", line).with("op", "or").with("left", left).with("right", right);
            }
            left = self.claim(left)?;
        }
        Ok(left)
    }

    /// `e forever because "…"`: a wait that may last forever, with its reason.
    fn claim(&mut self, e: Node) -> Result<Node, Error> {
        if !(self.is("forever") && self.is_at(1, "because")) {
            return Ok(e);
        }
        let line = self.next().line;
        self.i += 1;
        let because = self.string("the reason, a string")?;
        Ok(Node::new("forever", line).with("value", e).with("because", because))
    }

    /// A string literal's text, without its quotes.
    fn string(&mut self, what: &str) -> Result<String, Error> {
        match self.peek() {
            Some(t) if t.kind == Kind::Str => {
                self.i += 1;
                Ok(t.text[1..t.text.len() - 1].to_string())
            }
            _ => self.err(what),
        }
    }

    fn and_expr(&mut self) -> Result<Node, Error> {
        let mut left = self.not_expr()?;
        // `and name = …` starts the next clause of a `where`, and is not an operand
        while self.is("and") && !(self.ident_at(1) && self.is_at(2, "=")) {
            let line = self.next().line;
            let right = self.not_expr()?;
            left = Node::new("binary", line).with("op", "and").with("left", left).with("right", right);
        }
        Ok(left)
    }

    fn not_expr(&mut self) -> Result<Node, Error> {
        if self.is("not") {
            let line = self.next().line;
            let e = self.not_expr()?;
            return Ok(Node::new("unary", line).with("op", "not").with("operand", e));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Node, Error> {
        let mut left = self.additive()?;
        while let Some(op) = ["==", "!=", "<=", ">=", "<", ">"].into_iter().find(|o| self.is(o)) {
            let line = self.next().line;
            let right = self.additive()?;
            left = Node::new("binary", line).with("op", op).with("left", left).with("right", right);
        }
        Ok(left)
    }

    fn additive(&mut self) -> Result<Node, Error> {
        let mut left = self.multiplicative()?;
        while let Some(op) = ["+", "-"].into_iter().find(|o| self.is(o)) {
            let line = self.next().line;
            let right = self.multiplicative()?;
            left = Node::new("binary", line).with("op", op).with("left", left).with("right", right);
        }
        Ok(left)
    }

    fn multiplicative(&mut self) -> Result<Node, Error> {
        let mut left = self.unary()?;
        while let Some(op) = ["*", "/", "%"].into_iter().find(|o| self.is(o)) {
            let line = self.next().line;
            let right = self.unary()?;
            left = Node::new("binary", line).with("op", op).with("left", left).with("right", right);
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Node, Error> {
        let line = self.here();
        if self.eat("-") {
            let e = self.unary()?;
            return Ok(Node::new("unary", line).with("op", "-").with("operand", e));
        }
        // `fake enqueue<command_type=c>` in a test: a fake of a capability named by its type
        if self.is("fake") && self.ident_at(1) && self.is_at(2, "<") {
            self.i += 1;
            let t = self.ty()?;
            return Ok(Node::new("fake", line).with("value", t));
        }
        // `mutable x` as an argument, `fake store(…)` in a test, `compile f(…)` in `main`, `submit c(…)` in a workflow
        for (word, kind) in [("mutable", "mutable"), ("fake", "fake"), ("compile", "compile"), ("submit", "submit")] {
            if self.is(word) && (self.ident_at(1) || self.is_at(1, "(") || self.is_at(1, "[")) {
                self.i += 1;
                let e = self.unary()?;
                return Ok(Node::new(kind, line).with("value", e));
            }
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Node, Error> {
        let mut e = self.primary()?;
        loop {
            let line = self.here();
            if self.is(".") {
                self.i += 1;
                let name = self.ident()?;
                e = Node::new("member", line).with("object", e).with("name", name);
            } else if self.is("(") {
                self.i += 1;
                let args = self.args()?;
                e = Node::new("call", line).with("callee", e).with("args", args);
            } else if self.is("[") {
                self.i += 1;
                let index = self.expr()?;
                self.expect("]")?;
                e = Node::new("index", line).with("object", e).with("index", index);
            } else {
                return Ok(e);
            }
        }
    }

    /// A call's arguments, after its `(` and through its `)`: each `value` or `name: value`.
    fn args(&mut self) -> Result<Vec<Node>, Error> {
        let mut out = vec![];
        while !self.eat(")") {
            let line = self.here();
            let mut arg = Node::new("arg", line);
            if self.ident_at(0) && self.is_at(1, ":") {
                arg.set("name", self.next().text.clone());
                self.i += 1;
            } else {
                arg.set("name", Value::Null);
            }
            arg.set("value", self.expr()?);
            out.push(arg);
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(out)
    }

    fn primary(&mut self) -> Result<Node, Error> {
        let Some(t) = self.peek() else { return self.err("an expression") };
        let line = t.line;
        match t.kind {
            Kind::Ident if t.text == "true" || t.text == "false" => {
                self.i += 1;
                Ok(literal(line, "boolean", &t.text, None))
            }
            // `old(x)` in an `ensures`: the value `x` had when the function was called (Y § 3.11)
            Kind::Ident if t.text == "old" && self.is_at(1, "(") => {
                self.i += 2;
                let e = self.expr()?;
                self.expect(")")?;
                Ok(Node::new("old", line).with("value", e))
            }
            // `reveal(secret) because "…"`: a claim that a sensitive value may be seen here (Y § 3.15)
            Kind::Ident if t.text == "reveal" && self.is_at(1, "(") => {
                self.i += 2;
                let e = self.expr()?;
                self.expect(")")?;
                if !self.eat("because") {
                    return self.err("`because` and the reason a sensitive value may be seen here");
                }
                let because = self.string("the reason, a string")?;
                Ok(Node::new("reveal", line).with("value", e).with("because", because))
            }
            Kind::Ident => {
                self.i += 1;
                Ok(Node::new("name", line).with("name", t.text.clone()))
            }
            Kind::Number => {
                self.i += 1;
                let digits = t.text.trim_end_matches(|c: char| c.is_ascii_alphabetic());
                let unit = &t.text[digits.len()..];
                let kind = match unit {
                    "" if digits.contains('.') => "decimal",
                    "" => "integer",
                    "ns" | "us" | "ms" | "s" | "min" | "h" | "d" => "duration",
                    // a quantity of a unit the file declares (Y § 3.3, "Units")
                    u if UNITS.with(|us| us.borrow().iter().any(|x| x == u)) => {
                        return Ok(literal(line, "quantity", &t.text, None).with("number", digits).with("unit", unit));
                    }
                    _ => return Err(Error::new(line, &format!("`{}`: `{unit}` is not a unit of time, nor the symbol of a unit this file declares", t.text))),
                };
                Ok(literal(line, kind, &t.text, None))
            }
            Kind::Str => {
                self.i += 1;
                Ok(literal(line, "string", &t.text, Some(&t.text[1..t.text.len() - 1])))
            }
            Kind::Char => {
                self.i += 1;
                Ok(literal(line, "character", &t.text, Some(&t.text[1..t.text.len() - 1])))
            }
            Kind::Prefixed => {
                self.i += 1;
                let q = t.text.find('"').unwrap();
                let prefix = &t.text[..q];
                let Some(kind) = ["path", "pattern", "url"].into_iter().find(|k| *k == prefix) else {
                    return Err(Error::new(line, &format!("`{prefix}\"…\"`: a string's prefix is `path`, `pattern` or `url`")));
                };
                Ok(literal(line, kind, &t.text, Some(&t.text[q + 1..t.text.len() - 1])))
            }
            Kind::Punct if t.text == "(" => {
                self.i += 1;
                let e = self.expr()?;
                self.expect(")")?;
                // the tree has no node for brackets, but `(a or b) or c` is two operands, and `a or b or c` three
                Ok(e.with("parens", true))
            }
            Kind::Punct if t.text == "[" => {
                self.i += 1;
                let mut items = vec![];
                while !self.eat("]") {
                    items.push(self.expr()?);
                    if !self.eat(",") {
                        self.expect("]")?;
                        break;
                    }
                }
                Ok(Node::new("list", line).with("items", items))
            }
            _ => self.err("an expression"),
        }
    }

    /// A type: `string`, `count<unit_type=rune>`, `list<element_type=decimal<precision=12, scale=2>>`. An argument's
    /// value is a type, or a literal such as `12` or `-1`.
    fn ty(&mut self) -> Result<Node, Error> {
        let (line, start) = (self.here(), self.i);
        let name = self.dotted()?;
        let mut args = vec![];
        if self.eat("<") {
            loop {
                let aline = self.here();
                let an = self.ident()?;
                self.expect("=")?;
                let start = self.i;
                let value = if self.ident_at(0) && !self.is("true") && !self.is("false") { self.ty()? } else { self.unary()? };
                args.push(Node::new("type_arg", aline).with("name", an).with("value", value).with("text", self.text(start, self.i)));
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(">")?;
        }
        Ok(Node::new("type", line).with("name", name).with("args", args).with("text", self.text(start, self.i)))
    }

    /// `name = condition`, and more of them after `and`: a `where` or `ensures` clause, or a type's `where`.
    fn clauses(&mut self, kind: &'static str) -> Result<Vec<Node>, Error> {
        let mut out = vec![];
        loop {
            let line = self.here();
            let name = self.ident()?;
            self.expect("=")?;
            let cond = self.expr()?;
            out.push(Node::new(kind, line).with("name", name).with("condition", cond));
            if !(self.is("and") && self.ident_at(1) && self.is_at(2, "=")) {
                return Ok(out);
            }
            self.i += 1;
        }
    }

    /// What a lambda or a nested function captures, after `captures`: each `name` or `mutable name`.
    fn captures(&mut self) -> Result<Vec<Node>, Error> {
        let mut out = vec![];
        loop {
            let line = self.here();
            let mutable = self.is("mutable") && self.ident_at(1) && self.eat("mutable");
            out.push(Node::new("capture", line).with("name", self.ident()?).with("mutable", mutable));
            if !self.eat(",") {
                return Ok(out);
            }
        }
    }

    /// A parameter list, after its `(` and through its `)`: each `name`, `injected name` or `mutable name`, with an
    /// optional `is type`.
    fn params(&mut self) -> Result<Vec<Node>, Error> {
        let mut out = vec![];
        while !self.eat(")") {
            let line = self.here();
            let injected = self.is("injected") && self.ident_at(1) && self.eat("injected");
            let mutable = self.is("mutable") && self.ident_at(1) && self.eat("mutable");
            let name = self.ident()?;
            let ty = if self.eat("is") { Some(self.ty()?) } else { None };
            out.push(Node::new("param", line).with("name", name).with("injected", injected).with("mutable", mutable).with("type", ty));
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(out)
    }
}

fn literal(line: usize, ty: &str, text: &str, value: Option<&str>) -> Node {
    let mut n = Node::new("literal", line).with("type", ty).with("text", text);
    if let Some(v) = value {
        n.set("value", v);
    }
    n
}

#[derive(Clone, Copy, PartialEq)]
enum Context {
    File,
    Facet,
    /// A trait's declarations: its methods' signatures, without bodies.
    Trait,
    /// An implementation's methods.
    Implementation,
}

/// A callable's head and body, after its keyword: `name(params) returns r fails e where … ensures …:` and its block.
/// One without a body ends in `native because "…"` (§ 3.18), or is a trait's method (`signature`).
fn callable(p: &mut P, e: &Entry, src: &str, line: usize, kind: &'static str, signature: bool) -> Result<Node, Error> {
    let name = p.ident()?;
    p.expect("(")?;
    let params = p.params()?;
    let (mut returns, mut fails, mut wheres, mut ensures, mut captures) = (None, vec![], vec![], vec![], vec![]);
    loop {
        if p.eat("returns") {
            returns = Some(p.ident()?);
        } else if p.eat("fails") {
            fails.push(p.dotted()?);
            while p.eat(",") {
                fails.push(p.dotted()?);
            }
        } else if p.eat("where") {
            wheres.extend(p.clauses("where")?);
        } else if p.eat("ensures") {
            ensures.extend(p.clauses("ensures")?);
        } else if p.eat("captures") {
            captures = p.captures()?;
        } else {
            break;
        }
    }
    let native = if p.is("native") && p.is_at(1, "because") {
        p.i += 2;
        Some(p.string("the reason, a string")?)
    } else {
        None
    };
    let bodiless = native.is_some() || signature;
    if signature && p.is(":") {
        return Err(Error::new(line, "a trait's method is a signature: it has no body, and its implementations give one"));
    }
    p.finish(!bodiless)?;
    if bodiless && !e.children.is_empty() {
        return Err(Error::new(e.children[0].line.line, "a native function or a trait's method has no body"));
    }
    let mut n = Node::new(kind, line)
        .with("name", name)
        .with("params", params)
        .with("returns", returns)
        .with("fails", fails)
        .with("where", wheres)
        .with("ensures", ensures)
        .with("body", stmts(&e.children, src)?);
    if !captures.is_empty() {
        n.set("captures", captures);
    }
    if let Some(b) = native {
        n.set("native", b);
    }
    Ok(n)
}

/// `allow(Y2031) because "…"`, after its first word.
fn allow(p: &mut P, line: usize) -> Result<Node, Error> {
    p.i += 2;
    let start = p.i;
    while !p.done() && !p.is(")") {
        p.i += 1;
    }
    let code = p.text(start, p.i);
    p.expect(")")?;
    if !p.eat("because") {
        return p.err("`because` and the reason the lint does not apply");
    }
    let because = p.string("the reason, a string")?;
    p.finish(false)?;
    Ok(Node::new("allow", line).with("code", code).with("because", because))
}

/// A command's `policy:` block: each line `name: value`, and `on_abandon: compensate c(…)` or `on_abandon: park`.
fn policy_node(e: &Entry, src: &str) -> Result<Node, Error> {
    let mut entries = vec![];
    for c in &e.children {
        let mut q = P::new(c.line, src);
        let cline = q.here();
        let name = q.ident()?;
        q.expect(":")?;
        let value = if q.is("compensate") && !q.is_at(1, ":") {
            let l = q.next().line;
            Node::new("compensate", l).with("value", q.expr()?)
        } else {
            q.expr()?
        };
        q.finish(false)?;
        entries.push(Node::new("policy_entry", cline).with("name", name).with("value", value));
    }
    Ok(Node::new("policy", e.line.line).with("entries", entries))
}

/// A constraint of a collection's or a mapping's declaration: `ordered by position`, `offers back_add, front_take`,
/// `unique`, `unordered`, `count fixed`, `count at most 10`, `costs constant for cursor insert`, `values many`.
fn collection_clause(e: &Entry, src: &str, shape: &str) -> Result<Node, Error> {
    let mut p = P::new(e.line, src);
    let line = p.here();
    let word = p.ident()?;
    let mut n = Node::new("collection_clause", line).with("word", word.clone());
    match word.as_str() {
        "ordered" => {
            p.expect("by")?;
            let by = p.ident()?;
            let ok = if shape == "mapping" { ["position", "insertion", "key"].contains(&by.as_str()) } else { ["position", "insertion", "value"].contains(&by.as_str()) };
            if !ok {
                return Err(Error::new(line, &format!("`ordered by {by}`: a {shape} is ordered by {}", if shape == "mapping" { "`position`, `insertion` or `key`" } else { "`position`, `insertion` or `value`" })));
            }
            n.set("by", by);
            if p.eat("using") {
                n.set("using", p.ident()?);
            }
        }
        "offers" => {
            let mut what = vec![p.ident()?];
            while p.eat(",") {
                what.push(p.ident()?);
            }
            n.set("offers", what);
        }
        "count" => {
            if p.eat("fixed") {
                n.set("fixed", true);
            } else {
                p.expect("at")?;
                p.expect("most")?;
                n.set("at_most", p.unary()?);
            }
        }
        "costs" => {
            while !p.done() {
                p.i += 1;
            }
        }
        "values" => p.expect("many")?,
        "unique" | "unordered" => {}
        _ => return Err(Error::new(line, &format!("`{word}` is not a constraint of a {shape}'s declaration: `ordered by`, `unordered`, `unique`, `count`, `offers`, `costs`{}", if shape == "mapping" { ", `values many`" } else { "" }))),
    }
    let start_text = P::new(e.line, src);
    n.set("text", start_text.text(0, start_text.t.len()));
    p.finish(false)?;
    Ok(n)
}

/// A test's `faults:` section: each line a fault to force (Y § 3.14): `conflict at f#txn, times: 2`, `crash after
/// commit f#txn`, `expire at f#txn`, `cancel at f#task`.
fn faults(entries: &[Entry], src: &str) -> Result<Vec<Node>, Error> {
    let mut out = vec![];
    for e in entries {
        let mut p = P::new(e.line, src);
        let line = p.here();
        let kind = p.ident()?;
        let at = match kind.as_str() {
            "conflict" | "expire" | "cancel" => {
                p.expect("at")?;
                p.ident()?
            }
            "crash" => {
                p.expect("after")?;
                p.expect("commit")?;
                p.ident()?
            }
            _ => return Err(Error::new(line, &format!("`{kind}` is not a fault a test forces: `conflict at`, `crash after commit`, `expire at`, `cancel at`"))),
        };
        if !at.contains('#') {
            return Err(Error::new(line, &format!("`{at}`: a fault is forced at a region or a task, `function#name`")));
        }
        let mut n = Node::new("fault", line).with("fault", kind).with("at", at);
        if p.eat(",") {
            p.expect("times")?;
            p.expect(":")?;
            n.set("times", p.unary()?);
        }
        p.finish(false)?;
        out.push(n.with("end", e.end()).with("blank", e.line.blank).with("gap", e.line.gap));
    }
    Ok(out)
}

/// A documentation block: its text, and its entries. An entry is a line that starts with a name and `: ` (`game: The
/// game, as game.`, `fails guess_error.empty: …`), with the indented lines that continue it. An entry that ends in `as
/// type.` has that `type`, with the lines it is on; any other has none.
fn doc_node(d: &Doc) -> Node {
    let text = d.lines.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n");
    // each entry, and its text's pieces with their lines: the rest of its first line, then each line that continues it
    let mut entries: Vec<(Node, Vec<(usize, String)>)> = vec![];
    let mut open = false;
    for (n, l) in &d.lines {
        if l.trim().is_empty() {
            open = false;
            continue;
        }
        if open && l.starts_with(' ') {
            let (e, pieces) = entries.last_mut().unwrap();
            let t = format!("{} {}", e.str("text").unwrap(), l.trim());
            e.set("text", t);
            pieces.push((*n, l.trim().to_string()));
            continue;
        }
        open = false;
        if let Some((name, rest)) = l.split_once(": ") {
            let words: Vec<&str> = name.split(' ').collect();
            let is_name = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            if (words.len() == 1 && is_name(words[0])) || (words.len() == 2 && words[0] == "fails" && is_name(words[1])) {
                entries.push((Node::new("doc_entry", *n).with("name", name).with("text", rest.trim()), vec![(*n, rest.trim().to_string())]));
                open = true;
            }
        }
    }
    let entries: Vec<Node> = entries.into_iter().map(|(e, pieces)| e.with("end", pieces.last().unwrap().0).with("type", entry_type(&pieces))).collect();
    Node::new("doc", d.line).with("end", d.end).with("blank", d.blank).with("text", text).with("entries", entries)
}

/// The type an entry ends with, `… as list<element_type=word>.`; `None` when it ends otherwise. Only the text after
/// the last word `as` is read, since the prose before it may hold what Y does not (`` `_ o _ e` ``).
fn entry_type(pieces: &[(usize, String)]) -> Option<Node> {
    let word_as = |t: &str| {
        t.match_indices("as").map(|(i, _)| i).filter(|&i| {
            let b = t.as_bytes();
            (i == 0 || b[i - 1] == b' ') && (i + 2 == t.len() || b[i + 2] == b' ')
        }).last()
    };
    let k = pieces.iter().rposition(|(_, t)| word_as(t).is_some())?;
    let mut rest: Vec<(usize, &str)> = vec![(pieces[k].0, &pieces[k].1[word_as(&pieces[k].1)? + 2..])];
    rest.extend(pieces[k + 1..].iter().map(|(n, t)| (*n, t.as_str())));
    let src = rest.iter().map(|(_, t)| *t).collect::<Vec<_>>().join("\n");
    let (mut toks, mut depth, mut comments, mut off) = (vec![], 0, vec![], 0);
    for (n, t) in &rest {
        lex::lex(t, *n, off, &mut toks, &mut depth, &mut comments).ok()?;
        off += t.len() + 1;
    }
    let mut p = P { t: &toks, i: 0, src: &src, line: rest[0].0 };
    let ty = p.ty().ok()?;
    p.eat(".");
    p.done().then_some(ty)
}

/// A declaration's node, with what is above it: its documentation block, its comment, and where it ends.
fn declared(mut n: Node, e: &Entry, public: Option<&str>) -> Node {
    n.set("visibility", public);
    n.set("end", e.end());
    n.set("blank", e.line.blank);
    n.set("gap", e.line.gap);
    n.set("doc", e.line.doc.as_ref().map(doc_node));
    let comment = e.line.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\n");
    n.set("comment", (!e.line.comments.is_empty()).then_some(comment));
    n
}

fn decls(entries: &[Entry], src: &str, ctx: Context) -> Result<Vec<Node>, Error> {
    let mut out = vec![];
    for e in entries {
        let mut p = P::new(e.line, src);
        let line = p.here();
        let vis = if p.is("public") || p.is("private") { Some(p.next().text.clone()) } else { None };
        let Some(word) = p.peek().filter(|t| t.kind == Kind::Ident).map(|t| t.text.as_str()) else { return p.err("a declaration") };
        let n = match word {
            "module" | "from" if vis.is_none() && ctx == Context::File => {
                p.i += 1;
                if word == "module" {
                    let name = p.dotted()?;
                    p.finish(false)?;
                    Node::new("module", line).with("name", name)
                } else {
                    let from = p.dotted()?;
                    p.expect("import")?;
                    let mut names = vec![p.dotted()?];
                    while p.eat(",") {
                        names.push(p.dotted()?);
                    }
                    p.finish(false)?;
                    Node::new("import", line).with("from", from).with("names", names)
                }
            }
            "facet" if ctx == Context::File => {
                p.i += 1;
                let name = p.ident()?;
                p.finish(true)?;
                Node::new("facet", line).with("name", name).with("decls", decls(&e.children, src, Context::Facet)?)
            }
            "constants" | "service" => {
                p.i += 1;
                let name = p.ident()?;
                p.finish(true)?;
                let mut items = vec![];
                for c in &e.children {
                    let mut q = P::new(c.line, src);
                    let cline = q.here();
                    let cname = q.ident()?;
                    let ty = if q.eat("is") { Some(q.ty()?) } else { None };
                    q.expect("=")?;
                    let value = q.expr()?;
                    q.finish(false)?;
                    items.push(declared(Node::new(if word == "service" { "setting" } else { "constant" }, cline).with("name", cname).with("type", ty).with("value", value), c, None));
                }
                Node::new(if word == "service" { "service" } else { "constants" }, line).with("name", name).with("items", items)
            }
            "type" => {
                p.i += 1;
                let name = p.ident()?;
                // `type http native because "…"`: a type whose representation the backend gives (Y § 3.18)
                if p.is("native") && p.is_at(1, "because") {
                    p.i += 2;
                    let because = p.string("the reason, a string")?;
                    p.finish(false)?;
                    let n = Node::new("type_decl", line).with("name", name).with("type", Value::Null).with("where", Vec::<Node>::new()).with("cases", Vec::<Node>::new()).with("native", because);
                    out.push(declared(n, e, vis.as_deref()));
                    continue;
                }
                let ty = if p.eat("is") { Some(p.ty()?) } else { None };
                // `type quantity is int64 where positive = (value > 0)`: the restrictions on the head's line (§ 3.3)
                let (mut wheres, mut cases) = (vec![], vec![]);
                if ty.is_some() && p.eat("where") {
                    wheres.extend(p.clauses("where")?);
                }
                p.finish(!e.children.is_empty())?;
                // `type queue is collection:` and its constraints, one to a line (Y § 3.13)
                let shape = ty.as_ref().and_then(|t| t.str("name")).filter(|n| *n == "collection" || *n == "mapping").map(str::to_string);
                if let Some(shape) = shape {
                    let mut clauses = vec![];
                    for c in &e.children {
                        clauses.push(collection_clause(c, src, &shape)?);
                    }
                    let n = Node::new("type_decl", line).with("name", name).with("type", ty).with("where", Vec::<Node>::new()).with("cases", Vec::<Node>::new()).with("collection", clauses);
                    out.push(declared(n, e, vis.as_deref()));
                    continue;
                }
                for c in &e.children {
                    let mut q = P::new(c.line, src);
                    let cline = q.here();
                    if q.eat("where") {
                        wheres.extend(q.clauses("where")?);
                    } else if q.eat("case") {
                        let cname = q.ident()?;
                        let fields = if q.eat("(") { q.params()? } else { vec![] };
                        cases.push(declared(Node::new("case", cline).with("name", cname).with("fields", fields), c, None));
                    } else {
                        return q.err("`where` or `case`");
                    }
                    q.finish(false)?;
                }
                Node::new("type_decl", line).with("name", name).with("type", ty).with("where", wheres).with("cases", cases)
            }
            "structure" | "schema" => {
                p.i += 1;
                let name = p.ident()?;
                let version = if word == "schema" { Some(p.ident()?) } else { None };
                p.finish(true)?;
                let mut members = vec![];
                for c in &e.children {
                    members.push(declared(member(c, src)?, c, None));
                }
                let mut n = Node::new(if word == "schema" { "schema" } else { "structure" }, line).with("name", name);
                if word == "schema" {
                    n.set("version", version);
                }
                n.with("members", members)
            }
            "migrate" => {
                p.i += 1;
                let name = p.ident()?;
                let from = p.ident()?;
                p.expect("to")?;
                let to = p.ident()?;
                p.finish(true)?;
                Node::new("migrate", line).with("name", name).with("from", from).with("to", to).with("body", stmts(&e.children, src)?)
            }
            "function" | "transition" | "handler" | "workflow" => {
                p.i += 1;
                callable(&mut p, e, src, line, if word == "function" { "function" } else if word == "transition" { "transition" } else if word == "handler" { "handler" } else { "workflow" }, ctx == Context::Trait)?
            }
            // `mutable function cancel(reason)`: a method that changes `me` (Y § 3.6)
            "mutable" if p.is_at(1, "function") && (ctx == Context::Implementation || ctx == Context::Trait) => {
                p.i += 2;
                callable(&mut p, e, src, line, "function", ctx == Context::Trait)?.with("mutable", true)
            }
            "enumeration" => {
                p.i += 1;
                let name = p.ident()?;
                let ty = if p.eat("is") { Some(p.ty()?) } else { None };
                p.finish(true)?;
                let mut items = vec![];
                for c in &e.children {
                    let mut q = P::new(c.line, src);
                    let cline = q.here();
                    let cname = q.ident()?;
                    let value = if q.eat("=") { Some(q.expr()?) } else { None };
                    q.finish(false)?;
                    items.push(declared(Node::new("constant", cline).with("name", cname).with("type", Value::Null).with("value", value), c, None));
                }
                Node::new("enumeration", line).with("name", name).with("type", ty).with("items", items)
            }
            // `state order:` with its shared fields and its cases (Y § 3.11, "Typestate")
            "state" => {
                p.i += 1;
                let name = p.ident()?;
                p.finish(true)?;
                let (mut members, mut cases) = (vec![], vec![]);
                for c in &e.children {
                    let mut q = P::new(c.line, src);
                    let cline = q.here();
                    if q.eat("case") {
                        let cname = q.ident()?;
                        let fields = if q.eat("(") { q.params()? } else { vec![] };
                        q.finish(false)?;
                        cases.push(declared(Node::new("case", cline).with("name", cname).with("fields", fields), c, None));
                    } else {
                        members.push(declared(member(c, src)?, c, None));
                    }
                }
                Node::new("state", line).with("name", name).with("members", members).with("cases", cases)
            }
            "trait" if ctx != Context::Trait && ctx != Context::Implementation => {
                p.i += 1;
                let name = p.ident()?;
                p.finish(true)?;
                Node::new("trait", line).with("name", name).with("decls", decls(&e.children, src, Context::Trait)?)
            }
            // `implementation describable for shape:` and its methods
            "implementation" if ctx != Context::Trait && ctx != Context::Implementation => {
                p.i += 1;
                let tr = p.ident()?;
                p.expect("for")?;
                let ty = p.ident()?;
                p.finish(true)?;
                Node::new("implementation", line).with("trait", tr).with("type", ty).with("decls", decls(&e.children, src, Context::Implementation)?)
            }
            // `unit metre symbol m is integer`, `unit kilometre symbol km is 1000 metre`
            "unit" => {
                p.i += 1;
                let name = p.ident()?;
                p.expect("symbol")?;
                let symbol = p.ident()?;
                p.expect("is")?;
                let mut n = Node::new("unit", line).with("name", name).with("symbol", symbol);
                if p.peek().is_some_and(|t| t.kind == Kind::Number) {
                    let factor = p.next().text.clone();
                    n.set("factor", factor);
                    n.set("of", p.ident()?);
                    n.set("type", Value::Null);
                } else {
                    n.set("factor", Value::Null);
                    n.set("of", Value::Null);
                    n.set("type", p.ty()?);
                }
                p.finish(false)?;
                n
            }
            // `command charge_card:` with its fields and its delivery `policy:` (Y § 3.10)
            "command" => {
                p.i += 1;
                let name = p.ident()?;
                p.finish(true)?;
                let (mut members, mut policy) = (vec![], None);
                for c in &e.children {
                    let q = P::new(c.line, src);
                    if q.is("policy") && q.is_at(1, ":") {
                        policy = Some(policy_node(c, src)?);
                    } else {
                        members.push(declared(member(c, src)?, c, None));
                    }
                }
                let Some(policy) = policy else { return Err(Error::new(line, &format!("command `{name}` has no `policy:`: a command's delivery policy is not optional (Y § 3.10)"))) };
                Node::new("command", line).with("name", name).with("members", members).with("policy", policy)
            }
            // `allow(Y2031) because "…"`: a lint that does not apply to the declaration below (Y § 3.15)
            "allow" if p.is_at(1, "(") => allow(&mut p, line)?,
            "rule" => {
                p.i += 1;
                let name = p.ident()?;
                p.expect("(")?;
                let params = p.params()?;
                p.expect("=")?;
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("rule", line).with("name", name).with("params", params).with("value", value)
            }
            "test" if ctx == Context::File => {
                p.i += 1;
                let name = p.ident()?;
                let mut covers = None;
                if p.eat("covers") {
                    let start = p.i;
                    while !p.done() && !(p.is(":") && p.i + 1 == p.t.len()) {
                        if p.is(",") {
                            return Err(Error::new(p.here(), "a test covers one path"));
                        }
                        p.i += 1;
                    }
                    if p.i == start {
                        return p.err("the path the test covers");
                    }
                    covers = Some(path(&p.text(start, p.i), p.t[start].line));
                }
                p.finish(true)?;
                let mut sections = vec![];
                for c in &e.children {
                    let mut q = P::new(c.line, src);
                    let cline = q.here();
                    let Some(s) = ["given", "inject", "faults", "when", "then"].into_iter().find(|s| q.is(s)) else { return q.err("`given:`, `inject:`, `faults:`, `when:` or `then:`") };
                    q.i += 1;
                    q.finish(true)?;
                    let body = if s == "faults" { faults(&c.children, src)? } else { stmts(&c.children, src)? };
                    sections.push(Node::new("section", cline).with("name", s).with("end", c.end()).with("body", body));
                }
                Node::new("test", line).with("name", name).with("covers", covers).with("sections", sections)
            }
            _ => return p.err(if ctx == Context::File { "a declaration" } else { "a declaration in a facet" }),
        };
        out.push(declared(n, e, vis.as_deref()));
    }
    Ok(out)
}

/// The path a test covers, `f > a'many > b.c`: its text and its steps.
fn path(text: &str, line: usize) -> Node {
    let steps: Vec<String> = text.split('>').map(|s| s.trim().to_string()).collect();
    Node::new("path", line).with("text", text).with("steps", steps)
}


/// A member of a structure or a schema: `[key] name is type`, `derived name is type = value`, or `invariant name =
/// condition`.
fn member(e: &Entry, src: &str) -> Result<Node, Error> {
    let mut p = P::new(e.line, src);
    let line = p.here();
    let n = if p.is("invariant") && p.ident_at(1) {
        p.i += 1;
        let name = p.ident()?;
        p.expect("=")?;
        Node::new("invariant", line).with("name", name).with("condition", p.expr()?)
    } else if p.is("derived") && p.ident_at(1) {
        p.i += 1;
        let name = p.ident()?;
        p.expect("is")?;
        let ty = p.ty()?;
        p.expect("=")?;
        Node::new("derived", line).with("name", name).with("type", ty).with("value", p.expr()?)
    } else {
        let key = p.is("key") && p.ident_at(1) && p.eat("key");
        // `transient cache is …`: a field equality, hashing and serialisation leave out (Y § 3.3)
        let transient = p.is("transient") && p.ident_at(1) && p.eat("transient");
        let name = p.ident()?;
        p.expect("is")?;
        let n = Node::new("field", line).with("name", name).with("key", key).with("type", p.ty()?);
        if transient { n.with("transient", true) } else { n }
    };
    p.finish(false)?;
    Ok(n)
}

/// The statements of a block. An `else` joins the `if`, `for` or `while` before it, and an `orelse region` the region
/// before it.
fn stmts(entries: &[Entry], src: &str) -> Result<Vec<Node>, Error> {
    let mut out: Vec<Node> = vec![];
    for e in entries {
        let mut p = P::new(e.line, src);
        let line = p.here();
        let block = !e.children.is_empty();
        if p.is("else") {
            p.i += 1;
            p.else_named()?;
            let name = p.ident()?;
            p.finish(block)?;
            let els = Node::new("else", line).with("name", name).with("end", e.end()).with("body", stmts(&e.children, src)?);
            let Some(prev) = out.last_mut().filter(|n| matches!(n.kind, "if" | "for" | "while") && n.get("else") == Some(&Value::Null)) else {
                return Err(Error::new(line, "an `else` that follows no `if`, `for` or `while` without one"));
            };
            if prev.kind == "if" && !block {
                return Err(Error::new(line, "an `if`'s `else` opens a block"));
            }
            prev.set("else", els);
            prev.set("end", e.end());
            continue;
        }
        if p.is("orelse") {
            p.i += 1;
            p.expect("region")?;
            let alt = region(&mut p, e, src, line)?;
            let Some(prev) = out.last_mut().filter(|n| n.kind == "region") else {
                return Err(Error::new(line, "an `orelse region` that follows no region"));
            };
            if let Some(Value::List(alts)) = prev.get_mut("orelse") {
                alts.push(Value::Node(alt));
            }
            prev.set("end", e.end());
            continue;
        }
        let word = p.peek().filter(|t| t.kind == Kind::Ident).map_or("", |t| t.text.as_str());
        // a keyword starts a statement only where a name could not: `if x = …`, not `if = …`
        let keyword = !(p.is_at(1, "=") || p.is_at(1, ":=") || p.is_at(1, ".") || p.is_at(1, "(") || p.is_at(1, "["));
        let n = match word {
            "if" | "while" if keyword => {
                p.i += 1;
                let name = p.ident()?;
                p.expect("=")?;
                let cond = p.expr()?;
                p.finish(true)?;
                Node::new(if word == "if" { "if" } else { "while" }, line).with("name", name).with("condition", cond).with("body", stmts(&e.children, src)?).with("else", Value::Null)
            }
            "for" if keyword => {
                p.i += 1;
                let name = p.ident()?;
                p.expect("in")?;
                let iter = p.expr()?;
                p.finish(true)?;
                Node::new("for", line).with("name", name).with("iterable", iter).with("body", stmts(&e.children, src)?).with("else", Value::Null)
            }
            "conditional" | "compare" if keyword => {
                p.i += 1;
                let label = p.ident()?;
                let mut n = Node::new(if word == "compare" { "compare" } else { "conditional" }, line).with("label", label.clone());
                if word == "compare" {
                    p.expect("=")?;
                    n.set("left", p.expr()?);
                    p.expect("with")?;
                    n.set("right", p.expr()?);
                } else {
                    let subject = if p.eat("=") { Some(p.expr()?) } else { None };
                    n.set("subject", subject);
                }
                p.finish(true)?;
                let (mut conds, mut els) = (vec![], None);
                for c in &e.children {
                    let mut q = P::new(c.line, src);
                    let cline = q.here();
                    if els.is_some() {
                        return Err(Error::new(cline, &format!("nothing follows the `else` of `{word} {label}` in its block")));
                    }
                    let is_else = q.eat("else");
                    if is_else {
                        q.else_named()?;
                    } else {
                        q.expect("condition")?;
                    }
                    let name = q.dotted()?;
                    if name.split_once('.').is_none_or(|(l, _)| l != label) {
                        return Err(Error::new(cline, &format!("`{name}`: an outcome of `{word} {label}` is named `{label}.…`")));
                    }
                    let test = if is_else {
                        None
                    } else {
                        q.expect("=")?;
                        Some(q.expr()?)
                    };
                    q.finish(true)?;
                    let body = stmts(&c.children, src)?;
                    if is_else {
                        els = Some(Node::new("else", cline).with("name", name).with("end", c.end()).with("body", body));
                    } else {
                        conds.push(Node::new("condition", cline).with("name", name).with("test", test).with("end", c.end()).with("body", body));
                    }
                }
                n.with("conditions", conds).with("else", els)
            }
            "region" if keyword => {
                p.i += 1;
                region(&mut p, e, src, line)?
            }
            "arena" if keyword => {
                p.i += 1;
                let name = p.ident()?;
                p.finish(true)?;
                Node::new("arena", line).with("name", name).with("body", stmts(&e.children, src)?)
            }
            "task" if keyword => {
                p.i += 1;
                let name = p.ident()?;
                p.expect("=")?;
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("task", line).with("name", name).with("value", value)
            }
            "inject" if p.is_at(1, "(") => {
                p.i += 2;
                let mut bindings = vec![];
                while !p.eat(")") {
                    let bline = p.here();
                    let name = p.ident()?;
                    p.expect("=")?;
                    bindings.push(Node::new("binding", bline).with("name", name).with("value", p.expr()?));
                    if !p.eat(",") {
                        p.expect(")")?;
                        break;
                    }
                }
                p.finish(true)?;
                Node::new("inject", line).with("bindings", bindings).with("body", stmts(&e.children, src)?)
            }
            // `log warning "the hold {hold} expired"` and `metric holds_expired = count` (Y § 3.7, "Observability")
            "log" if keyword && p.ident_at(1) => {
                p.i += 1;
                let level = p.ident()?;
                if !["debug", "info", "warning", "error"].contains(&level.as_str()) {
                    return Err(Error::new(line, &format!("`log {level}`: a log entry's severity is `debug`, `info`, `warning` or `error`")));
                }
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("log", line).with("level", level).with("value", value)
            }
            "metric" if keyword && p.ident_at(1) && p.is_at(2, "=") => {
                p.i += 1;
                let name = p.ident()?;
                p.i += 1;
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("metric", line).with("name", name).with("value", value)
            }
            // `assert charged_once = (…)`: `fail not_reachable` wherever it does not hold (Y § 3.4)
            "assert" if keyword && p.ident_at(1) && p.is_at(2, "=") => {
                p.i += 1;
                let c = p.clauses("assert")?.remove(0);
                p.finish(false)?;
                Node::new("assert", line).with("name", c.str("name").unwrap()).with("condition", c.node("condition").unwrap().clone())
            }
            // `assume name = p because "…"`: a claim the prover may take as true (Y § 3.15)
            "assume" if keyword && p.ident_at(1) && p.is_at(2, "=") => {
                p.i += 1;
                let c = p.clauses("assume")?.remove(0);
                if !p.eat("because") {
                    return p.err("`because` and the reason the prover may take it as true");
                }
                let because = p.string("the reason, a string")?;
                p.finish(false)?;
                Node::new("assume", line).with("name", c.str("name").unwrap()).with("condition", c.node("condition").unwrap().clone()).with("because", because)
            }
            "allow" if p.is_at(1, "(") => allow(&mut p, line)?,
            // `enqueue charge_card(…)`: a command submitted with the transaction it is in (Y § 3.10)
            "enqueue" if keyword && p.ident_at(1) => {
                p.i += 1;
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("enqueue", line).with("value", value)
            }
            // `nested function helper(x) returns y captures z:`, a callable only its function calls (Y § 3.1)
            "nested" if p.is_at(1, "function") => {
                p.i += 2;
                let f = callable(&mut p, e, src, line, "function", false)?;
                let f = f.with("doc", e.line.doc.as_ref().map(doc_node));
                Node::new("nested", line).with("function", f)
            }
            // `name = lambda name(…) returns r captures c:`, a callable bound to a name (Y § 3.1)
            _ if p.ident_at(0) && p.is_at(1, "=") && p.is_at(2, "lambda") => {
                let name = p.ident()?;
                p.i += 2;
                let f = callable(&mut p, e, src, line, "lambda", false)?;
                if f.str("name") != Some(name.as_str()) {
                    return Err(Error::new(line, &format!("a lambda is bound to its own name: `{name} = lambda {name}(…)`")));
                }
                let f = f.with("doc", e.line.doc.as_ref().map(doc_node));
                Node::new("let", line).with("name", name).with("mutable", false).with("value", f)
            }
            "return" if !p.is_at(1, "=") => {
                p.i += 1;
                let mut n = Node::new("return", line);
                if !p.done() && p.ident_at(0) && p.is_at(1, "=") {
                    n.set("name", p.ident()?);
                    p.i += 1;
                    n.set("value", p.expr()?);
                } else if !p.done() {
                    n.set("name", Value::Null);
                    n.set("value", p.expr()?);
                }
                p.finish(false)?;
                n
            }
            "fail" if keyword => {
                p.i += 1;
                let value = if p.done() { None } else { Some(p.expr()?) };
                p.finish(false)?;
                Node::new("fail", line).with("value", value)
            }
            "continue" | "break" | "retry" if p.peek_at(1).is_none() => {
                p.i += 1;
                Node::new(match word {
                    "continue" => "continue",
                    "break" => "break",
                    _ => "retry",
                }, line)
            }
            "mutable" if p.ident_at(1) && p.is_at(2, "=") => {
                p.i += 1;
                let name = p.ident()?;
                p.i += 1;
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("let", line).with("name", name).with("mutable", true).with("value", value)
            }
            _ if p.ident_at(0) && p.is_at(1, "=") => {
                let name = p.ident()?;
                p.i += 1;
                let value = p.expr()?;
                p.finish(false)?;
                Node::new("let", line).with("name", name).with("mutable", false).with("value", value)
            }
            _ => {
                let e1 = p.expr()?;
                if p.eat(":=") {
                    let value = p.expr()?;
                    p.finish(false)?;
                    Node::new("assign", line).with("target", e1).with("value", value)
                } else {
                    p.finish(false)?;
                    Node::new("expr", line).with("value", e1)
                }
            }
        };
        let opens = n.get("body").is_some() || n.get("conditions").is_some() || matches!(n.kind, "nested") || n.node("value").is_some_and(|v| v.kind == "lambda");
        let n = if block && !opens { return Err(Error::new(line, "this statement opens no block")) } else { n };
        out.push(n.with("end", e.end()).with("blank", e.line.blank).with("gap", e.line.gap));
    }
    Ok(out)
}

/// A region, after `region` or `orelse region`: `name is c1, c2, …:` and its block. Each constraint keeps its text as
/// written; `kind` is its first word, and some have more: `waits until name = condition`, `within d or fail`,
/// `detach because "…"`, `isolation level`.
fn region(p: &mut P, e: &Entry, src: &str, line: usize) -> Result<Node, Error> {
    let name = p.ident()?;
    p.expect("is")?;
    let mut constraints = vec![];
    loop {
        let start = p.i;
        let cline = p.here();
        let kind = p.ident()?;
        let mut c = Node::new("constraint", cline).with("word", kind.clone());
        match kind.as_str() {
            "waits" => {
                p.expect("until")?;
                let w = p.clauses("until")?;
                let w = w.into_iter().next().unwrap();
                c.set("name", w.str("name").unwrap());
                c.set("condition", w.node("condition").unwrap().clone());
            }
            "within" => {
                c.set("bound", p.unary()?);
                let or_fail = p.is("or") && p.is_at(1, "fail");
                if or_fail {
                    p.i += 2;
                }
                c.set("or_fail", or_fail);
            }
            "detach" | "forever" => {
                p.expect("because")?;
                c.set("because", p.string("the reason, a string")?);
            }
            "isolation" => {
                c.set("level", p.ident()?);
            }
            _ => {
                if p.eat("(") {
                    c.set("args", p.args()?);
                }
            }
        }
        c.set("text", p.text(start, p.i));
        constraints.push(c);
        if !p.eat(",") {
            break;
        }
    }
    p.finish(true)?;
    Ok(Node::new("region", line).with("name", name).with("constraints", constraints).with("body", stmts(&e.children, src)?).with("orelse", Vec::<Node>::new()).with("end", e.end()))
}

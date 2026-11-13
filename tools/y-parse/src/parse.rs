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

/// Parses a Y source file: a module, its `main`, or its tests.
pub fn parse(src: &str) -> Result<Node, Error> {
    let (lines, comments) = lex::lines(src)?;
    let mut pos = 0;
    let entries = build(&lines, &mut pos, 0)?;
    if pos < lines.len() {
        return Err(Error::new(lines[pos].line, "indented less than its block, and more than the block that holds it"));
    }
    let decls = decls(&entries, src, Context::File)?;
    let comments: Vec<Node> = comments.into_iter().map(|c| Node::new("comment", c.line).with("text", c.text)).collect();
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
        // `mutable x` as an argument, `fake store(…)` in a test, `compile f(…)` in `main`
        for (word, kind) in [("mutable", "mutable"), ("fake", "fake"), ("compile", "compile")] {
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
                    _ => return Err(Error::new(line, &format!("`{}`: `{unit}` is not a unit of time", t.text))),
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
                Ok(e)
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
        let line = self.here();
        let name = self.dotted()?;
        let mut args = vec![];
        if self.eat("<") {
            loop {
                let aline = self.here();
                let an = self.ident()?;
                self.expect("=")?;
                let value = if self.ident_at(0) && !self.is("true") && !self.is("false") { self.ty()? } else { self.unary()? };
                args.push(Node::new("type_arg", aline).with("name", an).with("value", value));
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(">")?;
        }
        Ok(Node::new("type", line).with("name", name).with("args", args))
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
}

/// A documentation block: its text, and its entries. An entry is a line that starts with a name and `: ` (`game: The
/// game, as game.`, `fails guess_error.empty: …`), with the indented lines that continue it.
fn doc_node(d: &Doc) -> Node {
    let text = d.lines.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n");
    let mut entries: Vec<Node> = vec![];
    let mut open = false;
    for (n, l) in &d.lines {
        if l.trim().is_empty() {
            open = false;
            continue;
        }
        if open && l.starts_with(' ') {
            let e = entries.last_mut().unwrap();
            let t = format!("{} {}", e.str("text").unwrap(), l.trim());
            e.set("text", t);
            continue;
        }
        open = false;
        if let Some((name, rest)) = l.split_once(": ") {
            let words: Vec<&str> = name.split(' ').collect();
            let is_name = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            if (words.len() == 1 && is_name(words[0])) || (words.len() == 2 && words[0] == "fails" && is_name(words[1])) {
                entries.push(Node::new("doc_entry", *n).with("name", name).with("text", rest.trim()));
                open = true;
            }
        }
    }
    Node::new("doc", d.line).with("end", d.end).with("text", text).with("entries", entries)
}

/// A declaration's node, with what is above it: its documentation block, its comment, and where it ends.
fn declared(mut n: Node, e: &Entry, public: Option<&str>) -> Node {
    n.set("visibility", public);
    n.set("end", e.end());
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
                let ty = if p.eat("is") { Some(p.ty()?) } else { None };
                p.finish(!e.children.is_empty())?;
                let (mut wheres, mut cases) = (vec![], vec![]);
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
            "function" => {
                p.i += 1;
                let name = p.ident()?;
                p.expect("(")?;
                let params = p.params()?;
                let (mut returns, mut fails, mut wheres, mut ensures) = (None, vec![], vec![], vec![]);
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
                    } else {
                        break;
                    }
                }
                p.finish(true)?;
                Node::new("function", line)
                    .with("name", name)
                    .with("params", params)
                    .with("returns", returns)
                    .with("fails", fails)
                    .with("where", wheres)
                    .with("ensures", ensures)
                    .with("body", stmts(&e.children, src)?)
            }
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
                    let Some(s) = ["given", "inject", "when", "then"].into_iter().find(|s| q.is(s)) else { return q.err("`given:`, `inject:`, `when:` or `then:`") };
                    q.i += 1;
                    q.finish(true)?;
                    sections.push(Node::new("section", cline).with("name", s).with("end", c.end()).with("body", stmts(&c.children, src)?));
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
        let name = p.ident()?;
        p.expect("is")?;
        Node::new("field", line).with("name", name).with("key", key).with("type", p.ty()?)
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
                    if !is_else {
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
        let n = if block && n.get("body").is_none() && n.get("conditions").is_none() { return Err(Error::new(line, "this statement opens no block")) } else { n };
        out.push(n.with("end", e.end()));
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
        let mut c = Node::new("constraint", cline).with("kind", kind.clone());
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

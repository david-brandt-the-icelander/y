//! One small source for each construct the parser knows, and the tree it makes; and malformed sources, with the line
//! of their error.
use y_parse::{parse, Node};

fn tree(src: &str) -> Node {
    parse(src).unwrap_or_else(|e| panic!("{e}\n{src}"))
}

/// The only declaration of a source.
fn decl(src: &str) -> Node {
    tree(src).nodes("decls")[0].clone()
}

/// The statements of a function `f` whose body is `body`, indented by four.
fn body(body: &str) -> Vec<Node> {
    let indented: Vec<String> = body.lines().map(|l| if l.is_empty() { String::new() } else { format!("    {l}") }).collect();
    let f = decl(&format!("function f(x) returns y:\n{}\n", indented.join("\n")));
    f.nodes("body").into_iter().cloned().collect()
}

fn s<'a>(n: &'a Node, key: &str) -> &'a str {
    n.str(key).unwrap_or_else(|| panic!("no {key} in {n:?}"))
}

#[test]
fn a_module_and_its_imports() {
    let t = tree("module main\n\nfrom standard.console import console, console_error\nfrom hangman import setup.game_rules\n");
    let d = t.nodes("decls");
    assert_eq!((d[0].kind, s(d[0], "name")), ("module", "main"));
    assert_eq!((d[1].kind, s(d[1], "from"), d[1].line), ("import", "standard.console", 3));
    assert_eq!(d[2].get("names"), Some(&vec!["setup.game_rules".to_string()].into()));
}

#[test]
fn a_facet_with_its_comment() {
    let f = decl("-- The rules.\npublic facet setup:\n    constants rules:\n        misses is count<unit_type=rune> = 6\n        words = \"a b\"\n");
    assert_eq!((f.kind, s(&f, "name"), s(&f, "visibility"), s(&f, "comment")), ("facet", "setup", "public", "The rules."));
    let c = f.nodes("decls")[0];
    assert_eq!((c.kind, s(c, "name")), ("constants", "rules"));
    let items = c.nodes("items");
    let ty = items[0].node("type").unwrap();
    assert_eq!((s(ty, "name"), s(ty.nodes("args")[0], "name"), s(ty, "text")), ("count", "unit_type", "count<unit_type=rune>"));
    assert_eq!(s(ty.nodes("args")[0].node("value").unwrap(), "name"), "rune");
    assert_eq!(s(items[1].node("value").unwrap(), "value"), "a b");
    assert_eq!(items[1].line, 5);
}

#[test]
fn a_type_with_where_clauses_and_one_with_cases() {
    let t = decl("type word is string:\n    where letters = value.matches(pattern\"^[a-z]*$\") and\n          long = (value.count() >= 4)\n");
    let w = t.nodes("where");
    assert_eq!((w.len(), s(w[0], "name"), s(w[1], "name")), (2, "letters", "long"));
    let lit = w[0].node("condition").unwrap().nodes("args")[0].node("value").unwrap().clone();
    assert_eq!((s(&lit, "type"), s(&lit, "value")), ("pattern", "^[a-z]*$"));
    let t = decl("type play_error:\n    case input_closed\n    case console(error is console_error)\n");
    let cases = t.nodes("cases");
    assert_eq!((s(cases[0], "name"), s(cases[1].nodes("fields")[0], "name")), ("input_closed", "error"));
    // a refined type's restrictions on its head's line (Y § 3.3)
    let t = decl("type quantity is int64 where positive = (value > 0) and small = (value < 10)\n");
    assert_eq!(t.nodes("where").iter().map(|w| s(w, "name")).collect::<Vec<_>>(), ["positive", "small"]);
    let t = decl("type sku is string<character_set=ascii>\n");
    assert_eq!(s(t.node("type").unwrap(), "name"), "string");
}

#[test]
fn a_structure_with_a_derived_value_and_an_invariant() {
    let t = decl("structure game:\n    word is word\n    derived misses is count = list.count()\n    invariant few = (misses <= 6)\n");
    let m = t.nodes("members");
    assert_eq!(m.iter().map(|n| n.kind).collect::<Vec<_>>(), ["field", "derived", "invariant"]);
}

#[test]
fn a_schema_and_its_migration() {
    let t = tree("schema item v2:\n    key sku is sku\n    vat is decimal<precision=5, scale=4>\n\n\nmigrate item v1 to v2:\n    vat := tax.standard\n");
    let d = t.nodes("decls");
    assert_eq!((s(d[0], "version"), d[0].nodes("members")[0].flag("key")), ("v2", true));
    assert_eq!((s(d[1], "from"), s(d[1], "to"), d[1].nodes("body")[0].kind), ("v1", "v2", "assign"));
}

#[test]
fn a_service() {
    let t = decl("service openmoji:\n    address = url\"https://openmoji.org\"\n");
    assert_eq!(s(t.nodes("items")[0].node("value").unwrap(), "type"), "url");
}

#[test]
fn a_function_head_with_its_parameters_returns_fails_where_and_ensures() {
    let f = decl("function guess(mutable game, line, injected console) returns answer fails guess_error\n        where playing = (outcome(game) == outcome.playing)\n        ensures done = game.over():\n    return answer = 1\n");
    let ps = f.nodes("params");
    assert_eq!(ps.iter().map(|p| (s(p, "name"), p.flag("mutable"), p.flag("injected"))).collect::<Vec<_>>(), [("game", true, false), ("line", false, false), ("console", false, true)]);
    assert_eq!((s(&f, "returns"), f.get("fails")), ("answer", Some(&vec!["guess_error".to_string()].into())));
    assert_eq!((s(f.nodes("where")[0], "name"), f.nodes("where")[0].line), ("playing", 2));
    assert_eq!((s(f.nodes("ensures")[0], "name"), f.nodes("ensures")[0].line), ("done", 3));
    assert_eq!((f.nodes("body")[0].line, f.num("end")), (4, Some(4)));
}

#[test]
fn a_rule() {
    let r = decl("rule hanged(game) = (game.misses == rules.allowed)\n");
    assert_eq!((r.kind, s(r.node("value").unwrap(), "op")), ("rule", "=="));
}

#[test]
fn a_documentation_block_and_its_entries() {
    let f = decl("-------\nReads a list.\n\ntext: The words, as\n  string.\n\nfails list_error.empty: None.\n-------\nfunction parse(text) fails list_error:\n    fail list_error.empty\n");
    let d = f.node("doc").unwrap();
    assert_eq!((d.line, d.num("end")), (1, Some(8)));
    let e = d.nodes("entries");
    assert_eq!((s(e[0], "name"), s(e[0], "text"), e[0].line, e[0].num("end")), ("text", "The words, as string.", 4, Some(5)));
    assert_eq!(s(e[1], "name"), "fails list_error.empty");
    assert!(s(d, "text").starts_with("Reads a list."));
    let ty = e[0].node("type").unwrap();
    assert_eq!((s(ty, "name"), ty.line), ("string", 5));
    assert!(e[1].node("type").is_none());
    // only what follows the last `as` is read, so the prose may hold what Y does not
    let f = decl("-------\nshown: Each letter (`_ o _`) as it is, as list<element_type=decimal<scale=2>>.\n-------\nfunction f():\n    return\n");
    let ty = f.node("doc").unwrap().nodes("entries")[0].node("type").unwrap().clone();
    assert_eq!((s(&ty, "text"), s(ty.nodes("args")[0], "text")), ("list<element_type=decimal<scale=2>>", "decimal<scale=2>"));
}

#[test]
fn bindings_and_assignments() {
    let b = body("a = 1\nmutable found = list.new()\nfound.items := found.items + 1\nfound.push(a)");
    assert_eq!(b.iter().map(|n| n.kind).collect::<Vec<_>>(), ["let", "let", "assign", "expr"]);
    assert!(!b[0].flag("mutable") && b[1].flag("mutable"));
    assert_eq!(b[2].node("target").unwrap().kind, "member");
}

#[test]
fn an_if_and_its_else() {
    let b = body("if blank = piece.is_empty():\n    continue\nelse has_text:\n    break\nreturn y = 1");
    assert_eq!(b.len(), 2);
    let e = b[0].node("else").unwrap();
    assert_eq!((s(&b[0], "name"), s(e, "name"), e.line, b[0].num("end")), ("blank", "has_text", 4, Some(5)));
    assert_eq!((b[0].nodes("body")[0].kind, e.nodes("body")[0].kind), ("continue", "break"));
}

#[test]
fn loops_and_their_else() {
    let b = body("for piece in text.split(\" \"):\n    retry\nelse nothing\nwhile going = (x > 0):\n    x := x - 1\nelse never:\n    fail not_reachable");
    assert_eq!((b[0].kind, s(&b[0], "name"), s(b[0].node("else").unwrap(), "name")), ("for", "piece", "nothing"));
    assert!(b[0].node("else").unwrap().nodes("body").is_empty());
    assert_eq!((b[1].kind, s(b[1].node("else").unwrap(), "name")), ("while", "never"));
}

#[test]
fn a_conditional_and_a_compare() {
    let b = body("conditional judged = x:\n    condition judged.a = (judged == 1):\n        return y = 1\n    else judged.b:\n        return y = 2\n\ncompare ranked = v with best:\n    condition ranked.top = equal:\n        continue\n    condition ranked.lower = less:\n        continue");
    assert_eq!((b[0].kind, s(&b[0], "label"), s(b[0].nodes("conditions")[0], "name"), s(b[0].node("else").unwrap(), "name")), ("conditional", "judged", "judged.a", "judged.b"));
    assert_eq!(b[0].node("subject").unwrap().kind, "name");
    assert_eq!((b[1].kind, b[1].nodes("conditions").len(), s(b[1].node("right").unwrap(), "name")), ("compare", 2, "best"));
    assert!(parse("function f():\n    conditional j:\n        condition k.a = x:\n            continue\n").is_err());
}

#[test]
fn regions_and_their_alternatives() {
    let b = body("region take is atomic, waits until enough = (n >= 2):\n    return y = 1\norelse region later is atomic:\n    return y = 2\nregion p is detach because \"why, then\", within 1min or fail, supervise(at_most: 2), isolation snapshot:\n    task first = pack(a)");
    let r = &b[0];
    assert_eq!((s(r, "name"), r.nodes("orelse").len(), s(r.nodes("orelse")[0], "name"), r.num("end")), ("take", 1, "later", Some(5)));
    let cs = r.nodes("constraints");
    assert_eq!((s(cs[1], "word"), s(cs[1], "name"), s(cs[1], "text")), ("waits", "enough", "waits until enough = (n >= 2)"));
    let cs = b[1].nodes("constraints");
    assert_eq!(cs.iter().map(|c| s(c, "text")).collect::<Vec<_>>(), ["detach because \"why, then\"", "within 1min or fail", "supervise(at_most: 2)", "isolation snapshot"]);
    assert_eq!((s(cs[0], "because"), cs[1].flag("or_fail"), s(cs[3], "level")), ("why, then", true, "snapshot"));
    assert_eq!(b[1].nodes("body")[0].kind, "task");
}

#[test]
fn an_arena_an_inject_a_claim_and_or_fail() {
    let b = body("arena scratch:\n    a = scratch.allocate(1)\ninject(\n        console = system.console(),\n        random = system.random()):\n    typed = console.read_line() forever because \"they think\" or fail");
    assert_eq!(b[0].kind, "arena");
    let i = &b[1];
    assert_eq!((i.kind, i.nodes("bindings").len(), i.nodes("bindings")[1].line), ("inject", 2, 6));
    let v = i.nodes("body")[0].node("value").unwrap();
    assert_eq!((v.kind, v.node("value").unwrap().kind, s(v.node("value").unwrap(), "because")), ("or_fail", "forever", "they think"));
}

#[test]
fn returns_and_fails() {
    let b = body("return y = 1\nreturn\nfail e.empty\nfail e.unreadable(sku: s)\nfail");
    assert_eq!(b.iter().map(|n| n.kind).collect::<Vec<_>>(), ["return", "return", "fail", "fail", "fail"]);
    assert_eq!(s(&b[0], "name"), "y");
    assert!(b[4].node("value").is_none());
}

#[test]
fn expressions() {
    let b = body("a = f(x, named: [1, 2.50, 'c', \"s\", path\"a.store\", 10s], mutable g)\nb = not x.y[0] and (-1 * 2 + 3 <= 4) or z\nc = compile f(1)");
    let call = b[0].node("value").unwrap();
    let args = call.nodes("args");
    assert_eq!((args.len(), s(args[1], "name"), args[2].node("value").unwrap().kind), (3, "named", "mutable"));
    let items = args[1].node("value").unwrap().nodes("items");
    assert_eq!(items.iter().map(|i| s(i, "type")).collect::<Vec<_>>(), ["integer", "decimal", "character", "string", "path", "duration"]);
    let e = b[1].node("value").unwrap();
    assert_eq!((s(e, "op"), s(e.node("left").unwrap(), "op")), ("or", "and"));
    assert_eq!(e.node("left").unwrap().node("left").unwrap().kind, "unary");
    assert_eq!(b[2].node("value").unwrap().kind, "compile");
    // the brackets the source writes are kept as a mark: `(a or b) or c` is two operands of `or`, `a or b or c` three
    let b = body("x = (a or b) or c\ny = a or b or c");
    let (x, y) = (b[0].node("value").unwrap(), b[1].node("value").unwrap());
    assert!(x.node("left").unwrap().flag("parens") && !y.node("left").unwrap().flag("parens"));
}

/// The blank lines and comments a tree records, for the layout rules of Y § 3.1.
#[test]
fn the_layout_is_recorded() {
    let t = tree("module m\n\n\n-- one\n------\nDoes it.\n------\nfunction f():\n    a = 1\n\n    -- why\n    if b = c:\n        d()\n    else e:\n        -- first\n        g()\n");
    let f = t.nodes("decls")[1];
    assert_eq!((f.num("blank"), f.node("doc").unwrap().num("blank")), (Some(0), Some(0)));
    let body = f.nodes("body");
    assert_eq!((body[1].num("blank"), body[1].num("gap")), (Some(0), Some(1)));
    let cs = t.nodes("comments");
    assert_eq!(cs.iter().map(|c| (c.line, c.num("blank").unwrap(), c.num("indent").unwrap(), c.str("after"))).collect::<Vec<_>>(),
        [(4, 2, 0, Some("code")), (11, 1, 4, Some("code")), (15, 0, 8, Some("head"))]);
}

#[test]
fn comments_are_kept_with_their_lines() {
    let t = tree("-- one\nmodule m\n\n-- two\nfunction f():\n    -- three\n    a = 1  -- four\n");
    let cs: Vec<(usize, &str)> = t.nodes("comments").iter().map(|c| (c.line, s(c, "text"))).collect();
    assert_eq!(cs, [(1, "one"), (4, "two"), (6, "three"), (7, "four")]);
    let alone: Vec<bool> = t.nodes("comments").iter().map(|c| c.flag("alone")).collect();
    assert_eq!(alone, [true, true, true, false]);
    assert_eq!(s(t.nodes("decls")[1], "comment"), "two");
}

#[test]
fn a_test_with_what_it_covers_and_its_sections() {
    let t = decl("-- It works.\ntest it_works\n        covers f > row'many > judged.a:\n    given:\n        a = 1\n    inject:\n            items = fake store(lines: [])\n            forever = fake forever\n    when:\n        b = f(a)\n    then:\n        same = (b == a)\n");
    assert_eq!((t.kind, s(&t, "name"), t.line), ("test", "it_works", 2));
    let c = t.node("covers").unwrap();
    assert_eq!(c.get("steps"), Some(&vec!["f".to_string(), "row'many".into(), "judged.a".into()].into()));
    assert_eq!((s(c, "text"), c.line), ("f > row'many > judged.a", 3));
    assert!(parse("test t covers f > a, g:\n    then:\n        x = y\n").unwrap_err().message.contains("one path"));
    let secs = t.nodes("sections");
    assert_eq!(secs.iter().map(|x| s(x, "name")).collect::<Vec<_>>(), ["given", "inject", "when", "then"]);
    assert_eq!(secs[1].nodes("body")[0].node("value").unwrap().kind, "fake");
}

#[test]
fn the_json_has_kind_line_and_fields() {
    let j = tree("module m\n").to_json();
    assert!(j.starts_with("{\n  \"kind\": \"file\",\n  \"line\": 1,"), "{j}");
    assert!(j.contains("\"name\": \"m\""), "{j}");
}

/// Malformed sources: each error names the line where it went wrong.
#[test]
fn a_malformed_file_gives_an_error_with_its_line() {
    let cases: [(&str, usize, &str); 15] = [
        ("module m\n\nfunction f(x):\n    if a = b:\n        c()\n    else if d = e:\n        c()\n", 6, "`else if` ladder"),
        ("module m\n\nfunction f(x):\n    if a = b:\n        c()\n    else:\n        c()\n", 6, "`else` without a name"),
        ("module m\n\nfunction f(x):\n    conditional j:\n        condition j.a = b:\n            c()\n        else:\n            c()\n", 7, "`else` without a name"),
        ("module m\n\nfunction f(x:\n", 3, "bracket"),
        ("module m\n\nfunction f(x):\n    a = \"open\n", 4, "string"),
        ("module m\n\nfunction f(x):\n    a = 1 +\n", 4, "expected an expression"),
        ("module m\n\nfunction f(x):\n    if a = b\n        c = 1\n", 4, "expected `:`"),
        ("module m\n\nfunction f(x):\n    else b:\n        c = 1\n", 4, "`else`"),
        ("module m\n\nfunction f(x):\n\n", 3, "no indented line"),
        ("module m\n\nfunction f(x):\n        a = 1\n    b = 2\n", 5, "indented less"),
        ("module m\n\n------\nOpen.\n", 3, "documentation block"),
        ("module m\n\nwidget w:\n    a = 1\n", 3, "a declaration"),
        ("module m\n\nfunction f(x):\n    a = 1 # 2\n", 4, "unexpected character"),
        ("module m\n\nfunction f(x):\n    a = 3parsecs\n", 4, "unit of time"),
        ("module m\n\nfunction f(x):\n    orelse region r is atomic:\n        a = 1\n", 4, "orelse"),
    ];
    for (src, line, says) in cases {
        let e = parse(src).expect_err(src);
        assert_eq!(e.line, line, "{src}: {e}");
        assert!(e.message.contains(says), "{src}: {e}");
    }
}

//! The constructs no example uses, each with the tree it makes, and a malformed form of each with its error.
use y_parse::{parse, Node};

fn tree(src: &str) -> Node {
    parse(src).unwrap_or_else(|e| panic!("{e}\n{src}"))
}

fn decl(src: &str) -> Node {
    tree(src).nodes("decls")[0].clone()
}

fn body(body: &str) -> Vec<Node> {
    let indented: Vec<String> = body.lines().map(|l| if l.is_empty() { String::new() } else { format!("    {l}") }).collect();
    decl(&format!("function f(x) returns y:\n{}\n", indented.join("\n"))).nodes("body").into_iter().cloned().collect()
}

fn s<'a>(n: &'a Node, key: &str) -> &'a str {
    n.str(key).unwrap_or_else(|| panic!("no {key} in {n:?}"))
}

/// The error a source gives: its line, and that it says `says`.
fn refused(src: &str, line: usize, says: &str) {
    let e = parse(src).expect_err(src);
    assert_eq!(e.line, line, "{src}: {e}");
    assert!(e.message.contains(says), "{src}: {e}");
}

#[test]
fn an_enumeration() {
    let e = decl("enumeration severity is string:\n    debug = \"DEBUG\"\n    warning = \"WARNING\"\n");
    assert_eq!((e.kind, s(&e, "name"), s(e.node("type").unwrap(), "name")), ("enumeration", "severity", "string"));
    let items = e.nodes("items");
    assert_eq!((s(items[1], "name"), s(items[1].node("value").unwrap(), "value")), ("warning", "WARNING"));
    let e = decl("enumeration level:\n    low\n    high\n");
    assert!(e.node("type").is_none() && e.nodes("items")[0].node("value").is_none());
    refused("enumeration level:\n    low high\n", 2, "expected the end");
}

#[test]
fn a_state_and_a_transition() {
    let t = tree("state order:\n    id is uint32\n\n    case created\n    case paid(receipt is uint32)\n\n\nfunction g():\n    return\n\n\ntransition pay(created, receipt) returns paid:\n    return paid = created.becomes(order.paid(receipt: receipt))\n");
    let st = t.nodes("decls")[0];
    assert_eq!((st.kind, st.nodes("members").len(), st.nodes("cases").len()), ("state", 1, 2));
    assert_eq!(s(st.nodes("cases")[1].nodes("fields")[0], "name"), "receipt");
    let tr = t.nodes("decls")[2];
    assert_eq!((tr.kind, s(tr, "name"), s(tr, "returns")), ("transition", "pay", "paid"));
    refused("state order:\n    case\n", 2, "expected a name");
}

#[test]
fn a_trait_and_an_implementation() {
    let t = tree("trait describable:\n    function describe() returns text\n\n\nimplementation describable for shape:\n    function describe() returns text:\n        return text = \"a shape\"\n\n\n    mutable function grow(by):\n        me.size := me.size + by\n");
    let tr = t.nodes("decls")[0];
    assert_eq!((tr.kind, s(tr.nodes("decls")[0], "name"), tr.nodes("decls")[0].nodes("body").len()), ("trait", "describe", 0));
    let im = t.nodes("decls")[1];
    assert_eq!((im.kind, s(im, "trait"), s(im, "type")), ("implementation", "describable", "shape"));
    let grow = im.nodes("decls")[1];
    assert!(grow.flag("mutable"));
    assert_eq!(s(grow.nodes("body")[0].node("target").unwrap().node("object").unwrap(), "name"), "me");
    refused("trait t:\n    function f() returns r:\n        return r = 1\n", 2, "no body");
    refused("implementation t shape:\n    function f():\n        return\n", 1, "`for`");
}

#[test]
fn units_and_their_quantities() {
    let t = tree("unit metre symbol m is integer\nunit kilometre symbol km is 1000 metre\n\n\nfunction f() returns d:\n    return d = 12km\n");
    let d = t.nodes("decls");
    assert_eq!((s(d[0], "symbol"), s(d[0].node("type").unwrap(), "name")), ("m", "integer"));
    assert_eq!((s(d[1], "factor"), s(d[1], "of")), ("1000", "metre"));
    let lit = d[2].nodes("body")[0].node("value").unwrap().clone();
    assert_eq!((s(&lit, "type"), s(&lit, "number"), s(&lit, "unit")), ("quantity", "12", "km"));
    // a number with the symbol of no unit the file declares is still refused
    refused("function f() returns d:\n    return d = 12parsecs\n", 2, "not a unit of time");
    refused("unit metre m is integer\n", 1, "`symbol`");
}

#[test]
fn a_command_its_policy_a_handler_and_a_workflow() {
    let t = tree("command charge_card:\n    hold is uint32\n\n    policy:\n        deduplicate_by: hold\n        retry: backoff(first: 1s, factor: 2, attempts: 3)\n        on_abandon: compensate release_hold(hold: hold)\n\n\nhandler charge_card(command, injected gateway) returns charged fails charge_error:\n    return charged = 1\n\n\nworkflow fulfil(hold, injected charges) returns shipped:\n    paid = submit charge_card(hold: hold) or fail\n    return shipped = paid\n");
    let d = t.nodes("decls");
    assert_eq!((d[0].kind, d[0].nodes("members").len()), ("command", 1));
    let entries = d[0].node("policy").unwrap().nodes("entries");
    assert_eq!(entries.iter().map(|e| s(e, "name")).collect::<Vec<_>>(), ["deduplicate_by", "retry", "on_abandon"]);
    assert_eq!(entries[2].node("value").unwrap().kind, "compensate");
    assert_eq!((d[1].kind, d[2].kind), ("handler", "workflow"));
    let sub = d[2].nodes("body")[0].node("value").unwrap().node("value").unwrap().clone();
    assert_eq!(sub.kind, "submit");
    refused("command c:\n    a is uint32\n", 1, "no `policy:`");
}

#[test]
fn enqueue_log_metric_assert_assume_allow() {
    let b = body("enqueue charge_card(hold: 1)\nlog warning \"the hold {x} expired\"\nmetric holds_expired = 1\nassert positive = (x > 0)\nassume sorted = x.is_sorted() because \"the store keeps it sorted\"\nallow(too_many_paths) because \"a table of cases\"\nreturn y = x");
    assert_eq!(b.iter().map(|n| n.kind).collect::<Vec<_>>(), ["enqueue", "log", "metric", "assert", "assume", "allow", "return"]);
    assert_eq!((s(&b[1], "level"), s(&b[2], "name"), s(&b[3], "name")), ("warning", "holds_expired", "positive"));
    assert_eq!((s(&b[4], "because"), s(&b[5], "code")), ("the store keeps it sorted", "too_many_paths"));
    refused("function f():\n    log loudly \"x\"\n", 2, "severity");
    refused("function f():\n    assume a = b\n", 2, "`because`");
}

#[test]
fn old_reveal_and_me() {
    let f = decl("function f(mutable account, amount) returns debit\n        ensures debited = (account.balance == old(account.balance) - amount):\n    seen = reveal(secret) because \"the user asked\"\n    return debit = me\n");
    let cond = f.nodes("ensures")[0].node("condition").unwrap().clone();
    assert_eq!(cond.node("right").unwrap().node("left").unwrap().kind, "old");
    let r = f.nodes("body")[0].node("value").unwrap().clone();
    assert_eq!((r.kind, s(&r, "because")), ("reveal", "the user asked"));
    refused("function f():\n    seen = reveal(secret)\n", 2, "`because`");
}

#[test]
fn a_lambda_and_a_nested_function() {
    let b = body("------\nWhether active.\n------\nis_active = lambda is_active(user) returns active captures cutoff, mutable seen:\n    return active = (user.seen > cutoff)\nnested function helper(n) returns m:\n    return m = n\nreturn y = x");
    let l = b[0].node("value").unwrap().clone();
    assert_eq!((b[0].kind, l.kind, s(&l, "name")), ("let", "lambda", "is_active"));
    let caps = l.nodes("captures");
    assert_eq!((s(caps[0], "name"), caps[1].flag("mutable")), ("cutoff", true));
    assert!(l.node("doc").is_some());
    assert_eq!((b[1].kind, s(b[1].node("function").unwrap(), "name")), ("nested", "helper"));
    refused("function f():\n    a = lambda b(x) returns y:\n        return y = x\n", 2, "its own name");
}

#[test]
fn native_functions_and_types() {
    let t = tree("type http native because \"ureq\"\n\n\nfunction get(path) returns text fails http_error\n        native because \"it asks the service\"\n");
    let d = t.nodes("decls");
    assert_eq!((s(d[0], "native"), s(d[1], "native"), d[1].nodes("body").len()), ("ureq", "it asks the service", 0));
    refused("function get(path) returns text native because \"x\":\n    return text = path\n", 1, "expected the end");
}

#[test]
fn a_test_s_faults() {
    let t = decl("test retried:\n    faults:\n        conflict at hold#take, times: 2\n        expire at fetch#within\n        crash after commit hold#take\n        cancel at pack#first\n    when:\n        hold()\n    then:\n        ok = true\n");
    let f = t.nodes("sections")[0];
    assert_eq!(s(f, "name"), "faults");
    let fs = f.nodes("body");
    assert_eq!(fs.iter().map(|x| (s(x, "fault"), s(x, "at"))).collect::<Vec<_>>(), [("conflict", "hold#take"), ("expire", "fetch#within"), ("crash", "hold#take"), ("cancel", "pack#first")]);
    assert_eq!(s(fs[0].node("times").unwrap(), "text"), "2");
    refused("test t:\n    faults:\n        explode at f#r\n    then:\n        ok = true\n", 3, "not a fault");
    refused("test t:\n    faults:\n        conflict at f\n    then:\n        ok = true\n", 3, "`function#name`");
}

#[test]
fn a_fake_named_by_its_type() {
    let t = decl("test t:\n    inject:\n        charges = fake enqueue<command_type=charge_card>\n    then:\n        once = (charges.committed().count() == 1)\n");
    let bind = t.nodes("sections")[0].nodes("body")[0];
    let fake = bind.node("value").unwrap();
    assert_eq!(fake.kind, "fake");
    let ty = fake.node("value").unwrap();
    assert_eq!((ty.kind, s(ty, "name"), s(ty, "text")), ("type", "enqueue", "enqueue<command_type=charge_card>"));
    refused("test t:\n    inject:\n        charges = fake enqueue<command_type>\n", 3, "expected `=`");
}

#[test]
fn a_collection_s_declaration() {
    let t = decl("type backlog is collection:\n    ordered by position\n    offers back_add, front_take\n    unique\n    count at most 10\n    costs constant for cursor insert\n");
    let cs = t.nodes("collection");
    assert_eq!(cs.iter().map(|c| s(c, "word")).collect::<Vec<_>>(), ["ordered", "offers", "unique", "count", "costs"]);
    assert_eq!((s(cs[0], "by"), s(cs[4], "text")), ("position", "costs constant for cursor insert"));
    let m = decl("type index is mapping:\n    ordered by key\n    values many\n");
    assert_eq!(m.nodes("collection")[1].str("word"), Some("values"));
    refused("type q is collection:\n    ordered by key\n", 2, "ordered by");
    refused("type q is collection:\n    shiny\n", 2, "not a constraint");
}

#[test]
fn a_transient_field() {
    let t = decl("structure cache:\n    id is uint32\n    transient hits is uint32\n");
    let m = t.nodes("members");
    assert!(!m[0].flag("transient") && m[1].flag("transient"));
}

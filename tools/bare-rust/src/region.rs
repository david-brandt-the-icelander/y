//! Regions (language 6.23, Y § 3.8): how a callable, or a named part of one, may execute.
//!
//! A region is a subject of kind `region`, named `<function>#<region>`: the stable name of the callable and the
//! region's name in it. Its clauses are of kind `region`, one constraint each: `within d`, `forever because "…"`,
//! `detach because "…"`, `uncancellable`, `atomic`, `isolation serializable` or `isolation snapshot`, `may run again`,
//! `waits until name : p`, `on failure unchanged` and `supervise(…)`, the policy for the failures of the region's tasks.
//! A Y atomic region implies `may run again` and `on failure unchanged` (Y § 3.8), and its lowering states them.
//!
//! Three things are checked:
//! - the clauses are well formed and agree with one another (`check_clauses`, from `libcheck::validate`);
//! - the function the region is in is a subject of the file, and bounds its effects when the region may run again
//!   (`libcheck::validate`);
//! - the bare body of that function marks the region where it starts, `// y: region name is …` or, for an alternative
//!   of an `orelse`, `// y: #name`, and a marker that states the region's constraints states the same ones as its
//!   clauses (`find_marker`, `agree`);
//! - the code the marker starts keeps each constraint (`check_body`): an atomic region runs in a transaction
//!   (`atomic`, `atomic_with`, `atomic_when`, or as an alternative of `orelse`), and, as it may run again, uses no
//!   capability but `shared` state and does not wait; a snapshot one says `Isolation::Snapshot`; `within` starts a
//!   deadline or gives a duration, and every wait in the region takes the deadline; `forever` gives `forever`;
//!   `detach` calls `detach`; `uncancellable` holds cancellation (`.held(`); `waits until name` marks its condition
//!   `// y: waits until name` and retries until it holds; `supervise` runs its tasks with `supervised`.
//!
//! Weave runs the last two, and refuses a file whose region its body does not keep.
use crate::{parse_bare, text};
use serde_json::Value;
use tree_sitter::Node;

/// The words a region clause starts with.
pub const HEADS: &[&str] = &[
    "within",
    "forever because",
    "detach because",
    "uncancellable",
    "atomic",
    "isolation serializable",
    "isolation snapshot",
    "may run again",
    "waits until",
    "on failure unchanged",
    "supervise",
];

/// `crate::m::f#r` to (`crate::m::f`, `r`), if the name has that form.
pub fn split_name(name: &str) -> Option<(&str, &str)> {
    let (f, r) = name.split_once('#')?;
    let ident = |s: &str| !s.is_empty() && s.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (f.split("::").all(ident) && ident(r)).then_some((f, r))
}

/// The head a clause starts with.
pub fn head(text: &str) -> Option<&'static str> {
    HEADS.iter().copied().filter(|h| text == *h || text.starts_with(&format!("{h} ")) || (*h == "supervise" && text.starts_with("supervise("))).max_by_key(|h| h.len())
}

/// The first word of a constraint as a marker or a clause states it: what the two are compared by.
fn word(text: &str) -> &str {
    let w = text.split_whitespace().next().unwrap_or("");
    w.split('(').next().unwrap_or(w)
}

/// Everything wrong with a region's clauses, taken together.
pub fn check_clauses(texts: &[&str]) -> Vec<String> {
    let mut errs = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for t in texts {
        let Some(h) = head(t) else {
            errs.push(format!("a region clause starts with {}, not `{}`", HEADS.iter().map(|h| format!("`{h}`")).collect::<Vec<_>>().join(", "), word(t)));
            continue;
        };
        let key = if h.starts_with("isolation") { "isolation" } else { h };
        if !seen.insert(key) {
            errs.push(format!("the region says `{key}` twice"));
        }
        let rest = t[h.len()..].trim();
        match h {
            "within" if rest.is_empty() => errs.push("`within` names a duration or a bound: `within 5s`, `within rules.fetch_time`".into()),
            "forever because" | "detach because" if !(rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"')) => {
                errs.push(format!("`{h}` gives its reason in quotes: `{h} \"…\"`"))
            }
            "waits until" => {
                // `waits until name : p`, named as Y names it (`waits until name = (p)`)
                let named = rest.split_once(" : ").is_some_and(|(n, p)| crate::region::split_name(&format!("f#{n}")).is_some() && !p.trim().is_empty());
                if !named {
                    errs.push(format!("`waits until` names its condition: `waits until name : p`, not `{t}`"));
                }
            }
            "supervise" if !(rest.starts_with('(') && rest.ends_with(')')) && !t.starts_with("supervise(") => {
                errs.push(format!("`supervise` gives its policy in brackets: `supervise(restart: …)`, not `{t}`"))
            }
            "uncancellable" | "atomic" | "isolation serializable" | "isolation snapshot" | "may run again" | "on failure unchanged" if !rest.is_empty() => {
                errs.push(format!("`{h}` takes nothing after it: `{t}`"))
            }
            _ => {}
        }
    }
    let has = |w: &str| texts.iter().any(|t| head(t) == Some(w) || (w == "isolation" && head(t).is_some_and(|h| h.starts_with("isolation"))));
    if has("isolation") && !has("atomic") {
        errs.push("`isolation` is what an atomic region observes: the region is not `atomic`".into());
    }
    if has("may run again") && !has("atomic") {
        errs.push("`may run again` lets the host re-run an atomic region after a conflict: the region is not `atomic`".into());
    }
    if has("waits until") && !has("atomic") {
        errs.push("`waits until` starts a region once its condition holds, which only an atomic region can wait for: the region is not `atomic`".into());
    }
    if has("uncancellable") && !has("within") {
        errs.push("an `uncancellable` region holds cancellation off for a bounded time: it needs a `within`".into());
    }
    if has("within") && has("forever because") {
        errs.push("a region is `within` a bound or waits `forever`, not both".into());
    }
    errs
}

/// The functions of a bare body by their full paths (`base::module::f`, and `base::module::Type::f` in an impl), with
/// their nodes.
pub fn functions<'t>(root: Node<'t>, src: &str, base: &str) -> Vec<(String, Node<'t>)> {
    fn go<'t>(n: Node<'t>, src: &str, path: &str, out: &mut Vec<(String, Node<'t>)>) {
        let mut c = n.walk();
        for ch in n.named_children(&mut c) {
            match ch.kind() {
                "mod_item" => {
                    if let (Some(name), Some(body)) = (ch.child_by_field_name("name"), ch.child_by_field_name("body")) {
                        go(body, src, &format!("{path}::{}", text(name, src)), out);
                    }
                }
                "impl_item" => {
                    let ty = ch.child_by_field_name("type").map(|t| text(t, src).split('<').next().unwrap_or("").trim().to_string()).unwrap_or_default();
                    if let Some(body) = ch.child_by_field_name("body") {
                        go(body, src, &format!("{path}::{ty}"), out);
                    }
                }
                "function_item" => {
                    if let Some(name) = ch.child_by_field_name("name") {
                        let p = format!("{path}::{}", text(name, src));
                        out.push((p.clone(), ch));
                        if let Some(body) = ch.child_by_field_name("body") {
                            go(body, src, &p, out);
                        }
                    }
                }
                _ => go(ch, src, path, out),
            }
        }
    }
    let mut out = vec![];
    go(root, src, base, &mut out);
    out
}

/// The markers `// y: …` in a node, with their text after `// y:`.
fn markers(n: Node, src: &str, out: &mut Vec<String>) {
    if n.kind() == "line_comment" {
        if let Some(m) = text(n, src).strip_prefix("// y:") {
            out.push(m.trim().to_string());
        }
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        markers(ch, src, out);
    }
}

/// Where the region `function#region` starts in the bare body: `Ok` with the constraints its marker states (none for a
/// `#name` marker), or why it is not there.
pub fn find_marker(brs: &str, base: &str, name: &str) -> Result<Vec<String>, String> {
    let (f, r) = split_name(name).ok_or_else(|| format!("region `{name}` is not named `<function>#<region>`"))?;
    let tree = parse_bare(brs);
    let fns = functions(tree.root_node(), brs, base);
    let (_, node) = fns.iter().find(|(p, _)| p == f).ok_or_else(|| format!("region `{name}`: no function `{f}` in the bare body"))?;
    let mut ms = vec![];
    markers(*node, brs, &mut ms);
    for m in &ms {
        if let Some(h) = m.strip_prefix(&format!("region {r} is ")) {
            return Ok(split_top(h).into_iter().map(str::to_string).collect());
        }
        if m.split_whitespace().any(|w| w == format!("#{r}")) {
            return Ok(vec![]);
        }
    }
    Err(format!("region `{name}`: the body of `{f}` has no `// y: region {r} is …` or `// y: #{r}` where it starts"))
}

/// The code a marker starts: for a marker on a line of its own, what follows it to the end of its block; for one after
/// code on its line, the outermost statement that starts on that line.
fn extent<'t>(comment: Node<'t>, function: Node<'t>) -> Vec<Node<'t>> {
    let row = comment.start_position().row;
    let before = comment.prev_sibling().filter(|p| p.end_position().row == row);
    if before.is_none() {
        let mut out = vec![];
        let mut next = comment.next_sibling();
        while let Some(n) = next {
            if n.kind() != "}" {
                out.push(n);
            }
            next = n.next_sibling();
        }
        return out;
    }
    let mut best = before.unwrap();
    let mut up = comment.parent();
    while let Some(n) = up {
        if n.id() == function.id() || n.kind() == "block" && n.parent().is_some_and(|p| p.id() == function.id()) {
            break;
        }
        if n.start_position().row == row {
            best = n;
        }
        up = n.parent();
    }
    vec![best]
}

/// The region `function#region` of the constraint file `js`, against the bare body: its marker is there and states
/// its clauses' constraints (`find_marker`, `agree`), and the code the marker starts keeps each of them.
pub fn check_body(brs: &str, js: &Value, name: &str) -> Result<(), String> {
    let base = crate::weave::base_path(js);
    let stated = find_marker(brs, &base, name)?;
    let subject = js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "region" && s["name"] == name);
    let clauses: Vec<&str> = subject.into_iter().flat_map(|s| s["clauses"].as_array().into_iter().flatten()).filter_map(|c| c["text"].as_str()).collect();
    if let Some(e) = agree(name, &stated, &clauses).into_iter().next() {
        return Err(e);
    }
    let (f, r) = split_name(name).unwrap_or_default();
    let tree = parse_bare(brs);
    let fns = functions(tree.root_node(), brs, &base);
    let (_, node) = fns.iter().find(|(p, _)| p == f).ok_or_else(|| format!("region `{name}`: no function `{f}`"))?;
    let mut comments = vec![];
    comment_nodes(*node, brs, &mut comments);
    let marker = comments
        .iter()
        .find(|c| {
            let m = text(**c, brs).trim_start_matches("// y:").trim();
            m.starts_with(&format!("region {r} is ")) || m.split_whitespace().any(|w| w.trim_end_matches(',') == format!("#{r}"))
        })
        .ok_or_else(|| format!("region `{name}`: no marker"))?;
    let code: String = extent(*marker, *node).iter().map(|n| text(*n, brs)).collect::<Vec<_>>().join("\n");
    let whole = text(*node, brs);
    let has = |c: &str| clauses.iter().any(|t| head(t) == Some(c));
    let says = |what: &str| format!("region `{name}` says `{what}`, and the code its marker starts");
    if has("atomic") {
        let transaction = ["atomic(", "atomic_with(", "atomic_when(", "orelse(", "orelse_with("].iter().any(|c| code.contains(c));
        // an alternative of an `orelse`: a closure `let r = |tx| …`, passed to the `orelse`
        let alternative = code.trim_start().starts_with(&format!("let {r} = |"))
            && whole.split("orelse").skip(1).any(|after| after.split(';').next().is_some_and(|call| call.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == r)));
        if !transaction && !alternative {
            return Err(format!("{} runs in no transaction (`atomic`, `atomic_with`, `atomic_when`, or an alternative of `orelse`)", says("atomic")));
        }
        // it may run again, so it holds no external effect: no capability but shared state, and no wait
        let params = js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "function" && s["name"] == f).map(|s| s["params"].clone()).unwrap_or_default();
        for p in params.as_array().into_iter().flatten().filter(|p| p["injected"] == true) {
            let (n, ty) = (p["name"].as_str().unwrap_or(""), p["type"].as_str().unwrap_or(""));
            if !ty.contains("Shared<") && code.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == n) {
                return Err(format!("region `{name}` is atomic and may run again, so it holds no external effect, and it uses the capability `{n}`"));
            }
        }
        if code.contains(".await") {
            return Err(format!("region `{name}` is atomic and may run again, so it does not wait, and it awaits"));
        }
    }
    let isolation_snapshot = clauses.contains(&"isolation snapshot");
    if isolation_snapshot && !code.contains("Isolation::Snapshot") && !whole.contains("Isolation::Snapshot") {
        return Err(format!("{} does not run it at `Isolation::Snapshot`", says("isolation snapshot")));
    }
    if has("within") {
        if !code.contains("Deadline::after(") && !code.contains("Duration::") {
            return Err(format!("{} starts no deadline (`Deadline::after`) and gives no duration", says("within")));
        }
        if code.contains("Deadline::after(") {
            for l in code.lines().filter(|l| l.contains(".await")) {
                if !l.contains("deadline") {
                    return Err(format!("region `{name}` is within a deadline, and a wait in it does not take the deadline: `{}`", l.trim()));
                }
            }
        }
    }
    let checks: [(&str, &[&str], &str); 4] = [
        ("forever because", &["forever"], "does not give `forever` to its wait"),
        ("detach because", &["detach("], "does not call `detach`"),
        ("uncancellable", &[".held("], "does not hold cancellation (`.held(`)"),
        ("supervise", &["supervised("], "does not run its tasks with `supervised`"),
    ];
    for (c, needles, missing) in checks {
        if has(c) && !needles.iter().any(|n| code.contains(n)) {
            return Err(format!("{} {missing}", says(c)));
        }
    }
    for t in clauses.iter().filter(|t| head(t) == Some("waits until")) {
        let n = t["waits until".len()..].trim().split(" : ").next().unwrap_or("");
        if !code.contains(&format!("// y: waits until {n}")) || !(code.contains("Step::Retry") || code.contains("atomic_when(")) {
            return Err(format!("{} does not mark its condition `// y: waits until {n}` and retry until it holds", says("waits until")));
        }
    }
    Ok(())
}

fn comment_nodes<'t>(n: Node<'t>, src: &str, out: &mut Vec<Node<'t>>) {
    if n.kind() == "line_comment" && text(n, src).starts_with("// y:") {
        out.push(n);
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        comment_nodes(ch, src, out);
    }
}

/// A marker's constraints against the region's clauses: the same ones, by their first words. What an atomic region
/// implies (`may run again`, `on failure unchanged`) and the default isolation need not be in the marker, as Y does not
/// write them.
pub fn agree(name: &str, stated: &[String], clauses: &[&str]) -> Vec<String> {
    if stated.is_empty() {
        return vec![];
    }
    let a: std::collections::BTreeSet<&str> = stated.iter().map(|s| word(s)).collect();
    let implied = |c: &&str| a.contains("atomic") && (*c == "may run again" || *c == "on failure unchanged" || *c == "isolation serializable");
    let b: std::collections::BTreeSet<&str> = clauses.iter().filter(|c| !implied(c)).map(|s| word(s)).collect();
    let mut errs = vec![];
    for w in a.difference(&b) {
        errs.push(format!("region `{name}`: the bare body's marker says `{w}`, and no clause does"));
    }
    for w in b.difference(&a) {
        errs.push(format!("region `{name}`: a clause says `{w}`, and the bare body's marker does not"));
    }
    errs
}

/// The parts of a list split at its top-level commas: `within 10s or fail, supervise(a: 1, b: 2)` is two.
fn split_top(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start, mut quoted) = (vec![], 0i32, 0, false);
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' | '[' if !quoted => depth += 1,
            ')' | ']' if !quoted => depth -= 1,
            ',' if depth == 0 && !quoted => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_have_a_function_and_a_region() {
        assert_eq!(split_name("shop::checkout::hold#hold_one"), Some(("shop::checkout::hold", "hold_one")));
        assert_eq!(split_name("shop::checkout::hold"), None);
        assert_eq!(split_name("shop::hold#Hold"), None);
    }

    #[test]
    fn clauses_agree_with_one_another() {
        assert!(check_clauses(&["atomic", "isolation snapshot", "may run again", "waits until enough : count(s) > 0"]).is_empty());
        assert!(check_clauses(&["uncancellable", "within 50ms"]).is_empty());
        assert!(check_clauses(&["forever because \"a person may think\""]).is_empty());
        let errs = |t: &[&str]| check_clauses(t).join("\n");
        assert!(errs(&["isolation snapshot"]).contains("not `atomic`"));
        assert!(errs(&["may run again"]).contains("not `atomic`"));
        assert!(errs(&["waits until enough : x > 0"]).contains("not `atomic`"));
        assert!(errs(&["atomic", "waits until x > 0"]).contains("names its condition"));
        assert!(errs(&["uncancellable"]).contains("needs a `within`"));
        assert!(errs(&["within 1s", "forever because \"x\""]).contains("not both"));
        assert!(errs(&["forever because a person may think"]).contains("in quotes"));
        assert!(errs(&["atomic", "atomic"]).contains("twice"));
        assert!(errs(&["quickly"]).contains("starts with"));
        assert!(check_clauses(&["within 10s", "supervise(restart: restart_policy.each_failed, at_most: 2, per: 1min)"]).is_empty());
    }

    #[test]
    fn a_region_is_found_by_its_marker_in_its_function() {
        let brs = "mod stock {\n    fn reserve(w) {\n        let a = |tx| { // y: #take\n            tx.get(w)\n        };\n        // y: region count is atomic, within 5s or fail\n        a\n    }\n}\nfn other() {\n    // y: region lost is atomic\n}\n";
        assert_eq!(find_marker(brs, "c::m", "c::m::stock::reserve#take"), Ok(vec![]));
        assert_eq!(find_marker(brs, "c::m", "c::m::stock::reserve#count"), Ok(vec!["atomic".to_string(), "within 5s or fail".to_string()]));
        assert!(find_marker(brs, "c::m", "c::m::stock::reserve#lost").unwrap_err().contains("has no"));
        assert!(find_marker(brs, "c::m", "c::m::stock::gone#take").unwrap_err().contains("no function"));
        assert!(agree("r", &["atomic".into(), "within 5s or fail".into()], &["atomic", "within 5s"]).is_empty());
        assert!(agree("r", &["atomic".into()], &["atomic", "within 5s"])[0].contains("marker does not"));
        // what atomic implies need not be in the marker; `supervise(…)` must
        assert!(agree("r", &["atomic".into()], &["atomic", "may run again", "on failure unchanged"]).is_empty());
        assert!(agree("r", &["within 1s".into(), "supervise(at_most: 2)".into()], &["within 1s"])[0].contains("says `supervise`"));
    }

    fn constraints(subjects: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"crate": "c 0.0.0", "module": "src/m.rs", "subjects": subjects})
    }

    fn region(name: &str, texts: &[&str]) -> serde_json::Value {
        serde_json::json!({"kind": "region", "name": name, "clauses": texts.iter().map(|t| serde_json::json!({"kind": "region", "text": t, "source": "y/src/m.y:1"})).collect::<Vec<_>>()})
    }

    #[test]
    fn the_code_a_marker_starts_keeps_its_constraints() {
        let shown = "fn show(console, count) {\n    // y: region tally is atomic\n    let n = atomic(\"m::show#tally\", None, |tx| { let out = {\n        Ok(Step::Done(tx.get(count)))\n    }; out });\n    console.write_line(n);\n}\n";
        let show = serde_json::json!({"kind": "function", "name": "c::m::show", "params": [{"name": "console", "type": "Console", "injected": true}, {"name": "count", "type": "Shared<u32>", "injected": true}]});
        let ok = constraints(serde_json::json!([show.clone(), region("c::m::show#tally", &["atomic", "may run again", "on failure unchanged"])]));
        // a marker on a line of its own starts everything after it in its block: here the console's write too, which an
        // atomic region may not hold
        assert!(check_body(shown, &ok, "c::m::show#tally").unwrap_err().contains("uses the capability `console`"));
        // in a block of its own, the region ends before the write
        let fixed = "fn show(console, count) {\n    let n = {\n        // y: region tally is atomic\n        atomic(\"m::show#tally\", None, |tx| Ok(Step::Done(tx.get(count))))\n    };\n    console.write_line(n);\n}\n";
        assert_eq!(check_body(fixed, &ok, "c::m::show#tally"), Ok(()));
        // a region that is not run as a transaction
        let plain = fixed.replace("atomic(\"m::show#tally\", None, |tx| Ok(Step::Done(tx.get(count))))", "count.snapshot()");
        assert!(check_body(&plain, &ok, "c::m::show#tally").unwrap_err().contains("runs in no transaction"));
        // snapshot isolation is said in the code too
        let snap = constraints(serde_json::json!([show, region("c::m::show#tally", &["atomic", "isolation snapshot", "may run again", "on failure unchanged"])]));
        let fixed_snap = fixed.replace("// y: region tally is atomic", "// y: region tally is atomic, isolation snapshot");
        assert!(check_body(&fixed_snap, &snap, "c::m::show#tally").unwrap_err().contains("`Isolation::Snapshot`"));
        let with = fixed_snap.replace("atomic(\"m::show#tally\", None,", "atomic_with(\"m::show#tally\", None, Isolation::Snapshot,");
        assert_eq!(check_body(&with, &snap, "c::m::show#tally"), Ok(()));
    }

    #[test]
    fn a_deadline_is_started_and_given_to_every_wait() {
        let body = "async fn fetch(site) {\n    {\n        // y: region fetch is within 5s or fail\n        let deadline = Deadline::after(FIVE);\n        let a = site.get(\"/a\", &deadline).await?;\n        let b = site.get(\"/b\").await?;\n    }\n}\n";
        let js = constraints(serde_json::json!([{"kind": "function", "name": "c::m::fetch", "params": []}, region("c::m::fetch#fetch", &["within 5s"])]));
        assert!(check_body(body, &js, "c::m::fetch#fetch").unwrap_err().contains("does not take the deadline"));
        assert_eq!(check_body(&body.replace("site.get(\"/b\")", "site.get(\"/b\", &deadline)"), &js, "c::m::fetch#fetch"), Ok(()));
    }
}

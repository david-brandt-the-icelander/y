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
//!   capability but `shared` state, does not wait, and calls nothing that needs `effects` (language 6.22); a snapshot one says `Isolation::Snapshot`; `within` starts a
//!   deadline or gives a duration, and every wait in the region takes the deadline; `forever` gives `forever`;
//!   `detach` calls `detach`; `uncancellable` holds cancellation (`.held(`); `waits until name` marks its condition
//!   `// y: waits until name` and retries until it holds; `supervise` runs its tasks with `supervised`.
//!
//! The code a marker starts is what follows it in its block, up to `// y: end region name` or the block's end, for a
//! marker on a line of its own, and the statement it ends for one after code. Weave runs the last two, and refuses a
//! file whose region its body does not keep.
//!
//! What that code does is read from its syntax: the calls it makes, the names it uses, the waits in it. Which function
//! a call calls is taken from rustc where the woven crate builds (`check_body_with`, with `crate::facts`): weave with
//! cargo checks the regions after the build, so `atomic` bound to a local, renamed or re-exported is still the
//! runtime's transaction, and a method is its receiver's type's. Without a build (`check_body`, a weave without
//! cargo, a crate that does not compile), a call's function is the path it names, through the file's `use`
//! declarations. A call rustc said nothing of (in a test) is read the second way, and a macro is read from its
//! invocation either way (`crate::level`): `println!` in an atomic region is an effect. A function passed in that an
//! atomic region calls is refused, as nothing says what it does, unless rustc's facts and the whole crate's reading
//! (`crate::level::analyse`) say what it may be: the function values and closures of the crate that fit its signature,
//! or the library function that gave it, which then count as what the region calls. The rest is syntax either way: the capabilities used, by parameter name;
//! the deadline a wait takes; `Isolation::Snapshot` and `Step::Retry`, by path.
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

/// The code a marker starts: for a marker on a line of its own, what follows it in its block, up to a marker
/// `// y: end region name` on a line of its own or the block's end; for one after code on its line, the outermost
/// statement that starts on that line.
fn extent<'t>(comment: Node<'t>, function: Node<'t>, region: &str, src: &str) -> Vec<Node<'t>> {
    let row = comment.start_position().row;
    let before = comment.prev_sibling().filter(|p| p.end_position().row == row);
    if before.is_none() {
        let mut out = vec![];
        let mut next = comment.next_sibling();
        while let Some(n) = next {
            if n.kind() == "line_comment" && text(n, src).trim_start_matches("// y:").trim() == format!("end region {region}") {
                break;
            }
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

/// Every node under the nodes, themselves among them.
fn descendants<'t>(nodes: &[Node<'t>]) -> Vec<Node<'t>> {
    fn go<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
        out.push(n);
        let mut c = n.walk();
        for ch in n.children(&mut c) {
            go(ch, out);
        }
    }
    let mut out = vec![];
    for n in nodes {
        go(*n, &mut out);
    }
    out
}

/// Whether an identifier or path among the nodes is `word`, or ends in `::word`.
fn names(nodes: &[Node], src: &str, word: &str) -> bool {
    nodes.iter().any(|n| matches!(n.kind(), "identifier" | "scoped_identifier") && (text(*n, src) == word || text(*n, src).ends_with(&format!("::{word}"))))
}

/// The region `function#region` of the constraint file `js`, against the bare body: its marker is there and states
/// its clauses' constraints (`find_marker`, `agree`), and the code the marker starts keeps each of them. What the code
/// does is read from its syntax: the calls it makes, the names it uses, the waits in it.
pub fn check_body(brs: &str, js: &Value, name: &str) -> Result<(), String> {
    check_body_with(brs, js, name, None, None)
}

/// As `check_body`, with the calls rustc resolved in the bare body (`crate::facts`): a call's function is the one rustc
/// says, whatever the call names it by, and the calls the region makes are counted as `crate::level::uses_with` counts
/// them. A call rustc said nothing of is read from its syntax.
///
/// With `levels`, what the whole crate uses (`crate::level::analyse`): what the region calls is counted as the crate's
/// reading counts it, and a call of a function passed in is held to what it may be, when rustc's facts and the crate
/// say (the closures and function values that take its arguments), rather than refused.
pub fn check_body_with(brs: &str, js: &Value, name: &str, facts: Option<&[crate::facts::Fact]>, levels: Option<&crate::level::CrateLevels>) -> Result<(), String> {
    let facts = facts.filter(|f| !f.is_empty());
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
    let top = extent(*marker, *node, r, brs);
    let all = descendants(&top);
    let rows = (top.first().map_or(0, |n| n.start_position().row) + 1, top.last().map_or(0, |n| n.end_position().row) + 1);
    // names resolve through the file's imports: `atomic` after `use y_runtime::shared::atomic;` is that function
    let imports = crate::level::imports_of(brs, js);
    let resolve = |path: &str| -> String {
        let first = path.split("::").next().unwrap_or("");
        imports.get(first).map_or(path.to_string(), |m| format!("{m}{}", &path[first.len()..]))
    };
    // the function a call calls: the one rustc resolved where the call names it (the last it resolved there, which for
    // `a.b().held()` is `held`), or else the path it names, through the imports
    let path_of = |c: Node| -> Option<String> {
        let f = c.child_by_field_name("function")?;
        let resolved = facts.and_then(|fs| fs.iter().filter(|x| x.offset >= f.start_byte() && x.offset < f.end_byte()).max_by_key(|x| x.offset));
        if let Some(x) = resolved {
            return Some(x.callee.path.clone());
        }
        matches!(f.kind(), "identifier" | "scoped_identifier").then(|| resolve(text(f, brs)))
    };
    let calls: Vec<Node> = all.iter().copied().filter(|n| n.kind() == "call_expression").collect();
    let calls_to = |want: &[&str]| -> Vec<Node> { calls.iter().copied().filter(|c| path_of(*c).is_some_and(|p| want.contains(&p.as_str()))).collect() };
    let paths_in = |n: Node| -> Vec<String> { descendants(&[n]).iter().filter(|x| matches!(x.kind(), "identifier" | "scoped_identifier")).map(|x| resolve(text(*x, brs))).collect() };
    let awaits: Vec<Node> = all.iter().copied().filter(|n| n.kind() == "await_expression").collect();
    let comment_says = |what: &str| all.iter().any(|n| n.kind() == "line_comment" && text(*n, brs).trim_start_matches("// y:").trim() == what);
    let has = |c: &str| clauses.iter().any(|t| head(t) == Some(c));
    let says = |what: &str| format!("region `{name}` says `{what}`, and the code its marker starts");
    let params = js["subjects"].as_array().into_iter().flatten().find(|s| s["kind"] == "function" && s["name"] == f).map(|s| s["params"].clone()).unwrap_or_default();
    let params: Vec<(String, String, bool)> = params
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| (p["name"].as_str().unwrap_or("").to_string(), p["type"].as_str().unwrap_or("").to_string(), p["injected"] == true))
        .collect();
    const TRANSACTIONS: [&str; 5] = ["y_runtime::shared::atomic", "y_runtime::shared::atomic_with", "y_runtime::shared::atomic_when", "y_runtime::shared::orelse", "y_runtime::shared::orelse_with"];
    // the transaction an atomic region runs in: a call in the region, or, for an alternative of an `orelse` (a closure
    // `let r = |tx| …`), the `orelse` of the function that is given it
    let mut transaction = calls_to(&TRANSACTIONS);
    if transaction.is_empty() && top.len() == 1 && top[0].kind() == "let_declaration" {
        let named = top[0].child_by_field_name("pattern").is_some_and(|p| text(p, brs) == r);
        let closure = top[0].child_by_field_name("value").is_some_and(|v| v.kind() == "closure_expression");
        if named && closure {
            transaction = descendants(&[*node])
                .iter()
                .copied()
                .filter(|n| n.kind() == "call_expression" && path_of(*n).is_some_and(|p| p.ends_with("::orelse") || p.ends_with("::orelse_with")) && path_of(*n).is_some_and(|p| TRANSACTIONS.contains(&p.as_str())))
                .filter(|c| c.child_by_field_name("arguments").is_some_and(|a| names(&descendants(&[a]), brs, r)))
                .collect();
        }
    }
    if has("atomic") {
        if transaction.is_empty() {
            return Err(format!("{} runs in no transaction (`y_runtime::shared`'s `atomic`, `atomic_with`, `atomic_when`, or an alternative of `orelse`)", says("atomic")));
        }
        // it may run again, so it holds no external effect: no capability but shared state, no wait, no function value
        // passed in (whose effects nothing says), and nothing it calls that needs `effects` (language 6.22)
        for (n, ty, injected) in &params {
            let used = all.iter().any(|x| x.kind() == "identifier" && text(*x, brs) == n);
            // the right to wait without bound is no effect: an atomic region that is `forever` waits, by `waits until`
            let forever = ty == "Forever" || ty.ends_with("<target_type=Forever>");
            if *injected && !ty.contains("Shared<") && used && !(forever && has("forever because")) {
                return Err(format!("region `{name}` is atomic and may run again, so it holds no external effect, and it uses the capability `{n}`"));
            }
            // a call of it that rustc resolved to the crate's functions and closures that may be passed is held to what they
            // need (below); one it could not is refused
            let resolved = |c: &Node| {
                let Some(g) = c.child_by_field_name("function") else { return false };
                let fact = facts.into_iter().flatten().find(|x| x.offset >= g.start_byte() && x.offset < g.end_byte().max(g.start_byte() + 1) || x.offset == c.start_byte());
                match (fact.map(|x| &x.callee.kind), levels) {
                    (Some(crate::facts::Kind::Indirect(sig, origin)), Some(l)) => l.candidates_for(sig) > 0 || origin.is_some(),
                    _ => false,
                }
            };
            let called = calls.iter().any(|c| c.child_by_field_name("function").is_some_and(|g| g.kind() == "identifier" && text(g, brs) == n) && !resolved(c));
            if !injected && called {
                return Err(format!("region `{name}` is atomic and may run again, and it calls `{n}`, a function passed in, whose effects nothing says"));
            }
        }
        if !awaits.is_empty() {
            return Err(format!("region `{name}` is atomic and may run again, so it does not wait, and it awaits"));
        }
        let file = format!("{}.brs", js["module"].as_str().unwrap_or("").trim_end_matches(".rs"));
        let by_file: crate::facts::Facts = facts.map(|fs| [(file.clone(), fs.to_vec())].into_iter().collect()).unwrap_or_default();
        let effects = match levels {
            Some(l) => l.uses.iter().filter(|u| u.file == file).cloned().collect(),
            None => crate::level::uses_with(&[(file, brs.to_string(), js.clone())], facts.map(|_| &by_file)),
        };
        let waits_inside = |what: &str| what.starts_with("a wait, `waits until");
        // what keeps the region's own constraints is not an effect of its body: the deadline a `within` starts, the
        // `Forever` a `forever` gives its wait, and the hold an `uncancellable` puts on cancellation; nor is the
        // journal's record of a region a workflow opens, made once when it commits, outside its runs (Y § 3.10)
        let forevers: Vec<String> = params.iter().filter(|(_, ty, _)| ty == "Forever" || ty.ends_with("<target_type=Forever>")).map(|(n, ..)| format!("the capability `{n}`")).collect();
        let keeps_constraint = |what: &str| {
            (has("within") && what.contains("y_runtime::Deadline"))
                || (has("forever because") && (what.contains("y_runtime::Forever") || what == "a region that waits forever or is detached" || forevers.iter().any(|c| c == what)))
                || (has("uncancellable") && what.contains("y_runtime::task::Cancellation"))
                || what.contains("y_runtime::journal::region")
        };
        if let Some(u) = effects.iter().find(|u| u.function == f && u.level == 4 && u.line >= rows.0 && u.line <= rows.1 && !waits_inside(&u.what) && !keeps_constraint(&u.what)) {
            return Err(format!("region `{name}` is atomic and may run again, so it holds no external effect, and it has {} at line {} (language 6.22: `effects`)", u.what.trim_end_matches(','), u.line));
        }
    }
    let snapshot = |c: &Node| c.child_by_field_name("arguments").is_some_and(|a| paths_in(a).iter().any(|p| p == "y_runtime::shared::Isolation::Snapshot"));
    if clauses.contains(&"isolation snapshot") && !transaction.iter().any(snapshot) {
        return Err(format!("{} does not run it at `y_runtime::shared::Isolation::Snapshot`", says("isolation snapshot")));
    }
    if has("within") {
        let started = calls_to(&["y_runtime::Deadline::after"]);
        let duration = calls.iter().any(|c| path_of(*c).is_some_and(|p| p.starts_with("std::time::Duration::")));
        if started.is_empty() && !duration {
            return Err(format!("{} starts no deadline (`y_runtime::Deadline::after`) and gives no duration (`std::time::Duration`)", says("within")));
        }
        // the deadline is the name the region binds it to, and every wait in the region takes it
        let bound: Vec<String> = started
            .iter()
            .filter_map(|c| c.parent().filter(|p| p.kind() == "let_declaration").and_then(|p| p.child_by_field_name("pattern")).map(|p| text(p, brs).to_string()))
            .collect();
        if let Some(d) = bound.first() {
            for w in &awaits {
                if !names(&descendants(&[*w]), brs, d) {
                    return Err(format!("region `{name}` is within the deadline `{d}`, and a wait in it does not take it: `{}`", text(*w, brs).lines().next().unwrap_or("")));
                }
            }
        }
    }
    if has("forever because") {
        // `forever` is the capability to wait without bound: a parameter of type `Forever`, given to a call in the region
        let forever: Vec<&String> = params.iter().filter(|(_, ty, _)| ty == "Forever" || ty.ends_with("<target_type=Forever>")).map(|(n, ..)| n).collect();
        let given = calls.iter().any(|c| c.child_by_field_name("arguments").is_some_and(|a| forever.iter().any(|n| names(&descendants(&[a]), brs, n))));
        if !given {
            return Err(format!("{} gives no `Forever` capability to its wait", says("forever because")));
        }
    }
    let checks: [(&str, &[&str], &str); 2] = [
        ("detach because", &["y_runtime::task::detach"], "does not call `y_runtime::task::detach`"),
        ("supervise", &["y_runtime::task::supervised", "y_runtime::task::supervised_in"], "does not run its tasks with `y_runtime::task::supervised`"),
    ];
    for (c, call, missing) in checks {
        if has(c) && calls_to(call).is_empty() {
            return Err(format!("{} {missing}", says(c)));
        }
    }
    if has("uncancellable") {
        // `.held(…)` on a `y_runtime::task::Cancellation`
        let held = calls.iter().any(|c| {
            path_of(*c).as_deref() == Some("y_runtime::task::Cancellation::held")
                || c.child_by_field_name("function").is_some_and(|g| {
                g.kind() == "field_expression"
                    && g.child_by_field_name("field").is_some_and(|m| text(m, brs) == "held")
                    && g.child_by_field_name("value").is_some_and(|v| paths_in(v).iter().any(|p| p.starts_with("y_runtime::task::Cancellation")))
            })
        });
        if !held {
            return Err(format!("{} does not hold cancellation (`y_runtime::task::Cancellation`'s `held`)", says("uncancellable")));
        }
    }
    for t in clauses.iter().filter(|t| head(t) == Some("waits until")) {
        let n = t["waits until".len()..].trim().split(" : ").next().unwrap_or("");
        let retries = all.iter().any(|x| x.kind() == "scoped_identifier" && resolve(text(*x, brs)) == "y_runtime::shared::Step::Retry") || !calls_to(&["y_runtime::shared::atomic_when"]).is_empty();
        if !comment_says(&format!("waits until {n}")) || !retries {
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
        let shown = "use y_runtime::shared::{atomic, atomic_with, Isolation, Step};\nfn show(console, count) {\n    // y: region tally is atomic\n    let n = atomic(\"m::show#tally\", None, |tx| { let out = {\n        Ok(Step::Done(tx.get(count)))\n    }; out });\n    console.write_line(n);\n}\n";
        let show = serde_json::json!({"kind": "function", "name": "c::m::show", "params": [{"name": "console", "type": "Console", "injected": true}, {"name": "count", "type": "Shared<u32>", "injected": true}]});
        let ok = constraints(serde_json::json!([show.clone(), region("c::m::show#tally", &["atomic", "may run again", "on failure unchanged"])]));
        // a marker on a line of its own starts everything after it in its block: here the console's write too, which an
        // atomic region may not hold
        assert!(check_body(shown, &ok, "c::m::show#tally").unwrap_err().contains("uses the capability `console`"));
        // in a block of its own, the region ends before the write
        let fixed = "use y_runtime::shared::{atomic, atomic_with, Isolation, Step};\nfn show(console, count) {\n    let n = {\n        // y: region tally is atomic\n        atomic(\"m::show#tally\", None, |tx| Ok(Step::Done(tx.get(count))))\n    };\n    console.write_line(n);\n}\n";
        assert_eq!(check_body(fixed, &ok, "c::m::show#tally"), Ok(()));
        // a region that is not run as a transaction
        let plain = fixed.replace("atomic(\"m::show#tally\", None, |tx| Ok(Step::Done(tx.get(count))))", "count.snapshot()");
        assert!(check_body(&plain, &ok, "c::m::show#tally").unwrap_err().contains("runs in no transaction"));
        // snapshot isolation is said in the code too
        let snap = constraints(serde_json::json!([show, region("c::m::show#tally", &["atomic", "isolation snapshot", "may run again", "on failure unchanged"])]));
        let fixed_snap = fixed.replace("// y: region tally is atomic", "// y: region tally is atomic, isolation snapshot");
        assert!(check_body(&fixed_snap, &snap, "c::m::show#tally").unwrap_err().contains("`y_runtime::shared::Isolation::Snapshot`"));
        let with = fixed_snap.replace("atomic(\"m::show#tally\", None,", "atomic_with(\"m::show#tally\", None, Isolation::Snapshot,");
        assert_eq!(check_body(&with, &snap, "c::m::show#tally"), Ok(()));
    }

    #[test]
    fn a_deadline_is_started_and_given_to_every_wait() {
        let body = "use y_runtime::Deadline;\nasync fn fetch(site) {\n    {\n        // y: region fetch is within 5s or fail\n        let deadline = Deadline::after(FIVE);\n        let a = site.get(\"/a\", &deadline).await?;\n        let b = site.get(\"/b\").await?;\n    }\n}\n";
        let js = constraints(serde_json::json!([{"kind": "function", "name": "c::m::fetch", "params": []}, region("c::m::fetch#fetch", &["within 5s"])]));
        assert!(check_body(body, &js, "c::m::fetch#fetch").unwrap_err().contains("within the deadline `deadline`, and a wait in it does not take it"));
        // the deadline is found by its binding, whatever it is named
        let renamed = body.replace("let deadline =", "let limit =").replace("&deadline", "&limit");
        assert!(check_body(&renamed, &js, "c::m::fetch#fetch").unwrap_err().contains("within the deadline `limit`"));
        assert_eq!(check_body(&renamed.replace("site.get(\"/b\")", "site.get(\"/b\", &limit)"), &js, "c::m::fetch#fetch"), Ok(()));
        assert_eq!(check_body(&body.replace("site.get(\"/b\")", "site.get(\"/b\", &deadline)"), &js, "c::m::fetch#fetch"), Ok(()));
    }

    #[test]
    fn an_end_marker_ends_a_region_and_calls_are_followed() {
        // `log` uses the console; `tally` calls it from inside its atomic region
        let brs = "use y_runtime::shared::{atomic, Step};\nfn log(console) {\n    console.write_line(\"x\")\n}\nfn tally(count, console) {\n    // y: region add is atomic\n    let n = atomic(\"m::tally#add\", None, |tx| { let out = {\n        log(console);\n        Ok(Step::Done(tx.get(count)))\n    }; out });\n    // y: end region add\n    console.write_line(n);\n}\n";
        let log = serde_json::json!({"kind": "function", "name": "c::m::log", "params": [{"name": "console", "type": "Console", "injected": true}]});
        let tally = serde_json::json!({"kind": "function", "name": "c::m::tally", "params": [{"name": "count", "type": "Shared<u32>", "injected": true}, {"name": "console", "type": "Console", "injected": true}]});
        let js = constraints(serde_json::json!([log, tally, region("c::m::tally#add", &["atomic", "may run again", "on failure unchanged"])]));
        // the console is used in the region, as an argument of the call
        assert!(check_body(brs, &js, "c::m::tally#add").unwrap_err().contains("uses the capability `console`"));
        // without the argument, the call of `log` still needs `effects`, which the region may not hold
        let tally2 = serde_json::json!({"kind": "function", "name": "c::m::tally", "params": [{"name": "count", "type": "Shared<u32>", "injected": true}, {"name": "console", "type": "Console", "injected": true}]});
        let log2 = serde_json::json!({"kind": "function", "name": "c::m::log", "params": [], "clauses": [{"kind": "effects", "text": "effects only console", "source": "y/src/m.y:1"}]});
        let js2 = constraints(serde_json::json!([log2, tally2, region("c::m::tally#add", &["atomic", "may run again", "on failure unchanged"])]));
        let brs2 = brs.replace("fn log(console)", "fn log()").replace("log(console);", "log();");
        let err = check_body(&brs2, &js2, "c::m::tally#add").unwrap_err();
        assert!(err.contains("a call of `log`, which needs `effects`"), "{err}");
        // with the call gone, the region ends at its end marker, before the console's write
        let brs3 = brs2.replace("        log();\n", "");
        assert_eq!(check_body(&brs3, &js2, "c::m::tally#add"), Ok(()));
        assert!(check_body(&brs3.replace("    // y: end region add\n", ""), &js2, "c::m::tally#add").unwrap_err().contains("uses the capability `console`"));
    }

    #[test]
    fn an_atomic_region_calls_no_function_passed_in() {
        let brs = "use y_runtime::shared::{atomic, Step};\nfn apply(count, change) {\n    // y: region add is atomic\n    atomic(\"m::apply#add\", None, |tx| Ok(Step::Done(change(tx.get(count)))))\n}\n";
        let apply = serde_json::json!({"kind": "function", "name": "c::m::apply", "params": [{"name": "count", "type": "Shared<u32>", "injected": true}, {"name": "change", "type": "fn(u32) -> u32"}]});
        let js = constraints(serde_json::json!([apply, region("c::m::apply#add", &["atomic", "may run again", "on failure unchanged"])]));
        assert!(check_body(brs, &js, "c::m::apply#add").unwrap_err().contains("it calls `change`, a function passed in"));
        // an `atomic` that is not the runtime's is no transaction
        let other = brs.replace("use y_runtime::shared::{atomic, Step};", "use mylib::atomic;\nuse y_runtime::shared::Step;");
        assert!(check_body(&other, &js, "c::m::apply#add").unwrap_err().contains("runs in no transaction"));
    }

    #[test]
    fn an_atomic_region_prints_nothing() {
        let brs = "use y_runtime::shared::{atomic, Step};\nfn add(count) {\n    // y: region add is atomic\n    atomic(\"m::add#add\", None, |tx| { println!(\"adding\"); Ok(Step::Done(tx.get(count))) })\n}\n";
        let add = serde_json::json!({"kind": "function", "name": "c::m::add", "params": [{"name": "count", "type": "Shared<u32>", "injected": true}]});
        let js = constraints(serde_json::json!([add, region("c::m::add#add", &["atomic", "may run again", "on failure unchanged"])]));
        let err = check_body(brs, &js, "c::m::add#add").unwrap_err();
        assert!(err.contains("it has the macro `println!` (`std::println`) at line 4"), "{err}");
    }

    #[test]
    fn a_function_passed_in_is_held_to_what_it_may_be() {
        // `apply` calls `change`, passed in; the crate passes `double` (a calculation) or, in `loud`, `shout` (which
        // prints)
        let brs = "use y_runtime::shared::{atomic, Step};\nfn apply(count, change) {\n    // y: region add is atomic\n    atomic(\"m::apply#add\", None, |tx| Ok(Step::Done(change(tx.get(count)))))\n}\nfn double(x) {\n    x * 2\n}\nfn shout(x) {\n    println!(\"{}\", x);\n    x\n}\nfn calm(count) {\n    apply(count, double)\n}\nfn loud(count) {\n    apply(count, shout)\n}\n";
        let apply = serde_json::json!({"kind": "function", "name": "c::m::apply", "params": [{"name": "count", "type": "Shared<u32>", "injected": true}, {"name": "change", "type": "fn(u32) -> u32"}]});
        let js = constraints(serde_json::json!([apply, region("c::m::apply#add", &["atomic", "may run again", "on failure unchanged"])]));
        let externs = ["y_runtime".to_string()].into_iter().collect();
        let at = |s: &str, mir: &str, kind: crate::facts::Kind| {
            let offset = brs.find(s).unwrap();
            let mut callee = crate::facts::callee(mir, "c", &externs).unwrap();
            callee.kind = kind;
            crate::facts::Fact { offset, line: crate::trace::line_col(brs, offset).0, callee, def: None }
        };
        let u32s = || vec!["u32".to_string()];
        let mut facts = vec![
            at("atomic(", "y_runtime::shared::atomic::<u32, (), {closure@src/m.rs:4:41: 4:45}>", crate::facts::Kind::Call),
            at("change(", "x", crate::facts::Kind::Indirect(crate::facts::Sig::of(u32s()), None)),
            at("double)", "m::double", crate::facts::Kind::Value(crate::facts::Sig::of(u32s()))),
        ];
        let files = |facts: &Vec<crate::facts::Fact>| -> (Vec<(String, String, Value)>, crate::facts::Facts) {
            (vec![("src/m.brs".to_string(), brs.to_string(), js.clone())], [("src/m.brs".to_string(), facts.clone())].into_iter().collect())
        };
        // without what the crate passes, the call is refused; with it, `double` is all it may be, and it is kept
        assert!(check_body_with(brs, &js, "c::m::apply#add", Some(&facts), None).unwrap_err().contains("it calls `change`, a function passed in"));
        let (fs, m) = files(&facts);
        let levels = crate::level::analyse(&fs, Some(&m));
        assert_eq!(check_body_with(brs, &js, "c::m::apply#add", Some(&facts), Some(&levels)), Ok(()));
        // once `shout` may be passed too, the region may print
        facts.push(at("shout)", "m::shout", crate::facts::Kind::Value(crate::facts::Sig::of(u32s()))));
        let (fs, m) = files(&facts);
        let levels = crate::level::analyse(&fs, Some(&m));
        let err = check_body_with(brs, &js, "c::m::apply#add", Some(&facts), Some(&levels)).unwrap_err();
        assert!(err.contains("a call of a function pointer or an `Fn` bound (it may be `c::m::shout`"), "{err}");
    }

    #[test]
    fn rustc_sees_an_aliased_atomic_as_a_transaction() {
        // `run` is `atomic`, bound to a local: by its syntax, a call of `run`, which is no transaction
        let brs = "use y_runtime::shared::{atomic, Step};\nfn add(count) {\n    let run = atomic;\n    // y: region add is atomic\n    run(\"m::add#add\", None, |tx| Ok(Step::Done(tx.get(count))))\n}\n";
        let add = serde_json::json!({"kind": "function", "name": "c::m::add", "params": [{"name": "count", "type": "Shared<u32>", "injected": true}]});
        let js = constraints(serde_json::json!([add, region("c::m::add#add", &["atomic", "may run again", "on failure unchanged"])]));
        assert!(check_body(brs, &js, "c::m::add#add").unwrap_err().contains("runs in no transaction"));
        let externs = ["y_runtime".to_string()].into_iter().collect();
        let at = |s: &str, mir: &str| {
            let offset = brs.find(s).unwrap();
            crate::facts::Fact { offset, line: crate::trace::line_col(brs, offset).0, callee: crate::facts::callee(mir, "c", &externs).unwrap(), def: None }
        };
        // rustc names the function where `run` is bound and where it is called
        let facts = vec![
            at("atomic;", "y_runtime::shared::atomic::<u32, (), {closure@src/m.rs:5:30: 5:34}>"),
            at("run(", "y_runtime::shared::atomic::<u32, (), {closure@src/m.rs:5:30: 5:34}>"),
            at("get(", "y_runtime::shared::Tx::get::<u32>"),
        ];
        assert_eq!(check_body_with(brs, &js, "c::m::add#add", Some(&facts), None), Ok(()));
        // and a call in it that needs `effects`, by any name, is refused
        let mut store = facts.clone();
        store.push(at("get(", "y_runtime::store::Store::<m::Item>::read_all"));
        let err = check_body_with(brs, &js, "c::m::add#add", Some(&store), None).unwrap_err();
        assert!(err.contains("`read_all` (`y_runtime::store::Store::read_all`)"), "{err}");
    }
}

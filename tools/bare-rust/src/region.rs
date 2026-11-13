//! Regions (language 6.23, Y § 3.8): how a callable, or a named part of one, may execute.
//!
//! A region is a subject of kind `region`, named `<function>#<region>`: the stable name of the callable and the
//! region's name in it. Its clauses are of kind `region`, one constraint each: `within d`, `forever because "…"`,
//! `detach because "…"`, `uncancellable`, `atomic`, `isolation serializable` or `isolation snapshot`, `may run again`,
//! `waits until name : p` and `on failure unchanged`.
//!
//! Three things are checked:
//! - the clauses are well formed and agree with one another (`check_clauses`, from `libcheck::validate`);
//! - the function the region is in is a subject of the file, and bounds its effects when the region may run again
//!   (`libcheck::validate`);
//! - the bare body of that function marks the region where it starts, `// y: region name is …` or, for an alternative
//!   of an `orelse`, `// y: #name`, and a marker that states the region's constraints states the same ones as its
//!   clauses (`find_marker`, from weave, which refuses a file whose region is not in its body).
use crate::{parse_bare, text};
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
];

/// `crate::m::f#r` to (`crate::m::f`, `r`), if the name has that form.
pub fn split_name(name: &str) -> Option<(&str, &str)> {
    let (f, r) = name.split_once('#')?;
    let ident = |s: &str| !s.is_empty() && s.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (f.split("::").all(ident) && ident(r)).then_some((f, r))
}

/// The head a clause starts with.
pub fn head(text: &str) -> Option<&'static str> {
    HEADS.iter().copied().filter(|h| text == *h || text.starts_with(&format!("{h} "))).max_by_key(|h| h.len())
}

/// The first word of a constraint as a marker or a clause states it: what the two are compared by.
fn word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
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

/// A marker's constraints against the region's clauses: the same ones, by their first words. `supervise(…)` is how the
/// tasks of a region are run, not a constraint on it.
pub fn agree(name: &str, stated: &[String], clauses: &[&str]) -> Vec<String> {
    if stated.is_empty() {
        return vec![];
    }
    let a: std::collections::BTreeSet<&str> = stated.iter().map(|s| word(s)).filter(|w| !w.starts_with("supervise")).collect();
    let b: std::collections::BTreeSet<&str> = clauses.iter().map(|s| word(s)).collect();
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
    }
}

//! `bare-rust report`: the tables of results/e1.md, computed from the output of `bare-rust check` (one directory per
//! crate under its `--work` directory, holding `check.json` and `bare/`), so that the numbers can be regenerated.
use crate::check::test_count;
use crate::stats::stats;
use crate::R;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(crate) type Flat = BTreeMap<String, u64>;

pub(crate) struct Corpus {
    pub(crate) name: String,
    pub(crate) flat: Flat,
    pub(crate) check: Value,
}

/// Every number below a JSON object, keyed by its path joined with dots. Arrays, strings and booleans are skipped.
fn flatten(v: &Value, prefix: &str, out: &mut Flat) {
    match v {
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                out.insert(prefix.to_string(), n);
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                flatten(x, &if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn load(dir: &Path) -> R<Corpus> {
    let cj = dir.join("check.json");
    let text = fs::read_to_string(&cj).map_err(|e| format!("{}: {e} (run `bare-rust check` first)", cj.display()))?;
    let check: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", cj.display()))?;
    let mut flat = Flat::new();
    flatten(&stats(&dir.join("bare"))?, "", &mut flat);
    flatten(&check, "check", &mut flat);
    let name = check["crate"].as_str().unwrap_or("?").to_string();
    Ok(Corpus { name, flat, check })
}

pub(crate) fn table(out: &mut String, head: &[String], rows: &[Vec<String>]) {
    out.push_str(&format!("| {} |\n|{}\n", head.join(" | "), "---|".repeat(head.len())));
    for r in rows {
        out.push_str(&format!("| {} |\n", r.join(" | ")));
    }
    out.push('\n');
}

struct Report<'a> {
    cs: &'a [Corpus],
    out: String,
}

impl Report<'_> {
    fn n(&self, c: &Corpus, key: &str) -> u64 {
        c.flat.get(key).copied().unwrap_or(0)
    }

    fn total(&self, key: &str) -> u64 {
        self.cs.iter().map(|c| self.n(c, key)).sum()
    }

    fn head(&self, first: &str) -> Vec<String> {
        let mut h = vec![first.to_string()];
        h.extend(self.cs.iter().map(|c| c.name.clone()));
        h.push("all".into());
        h
    }

    fn row(&self, label: &str, key: &str) -> Vec<String> {
        let mut r = vec![label.to_string()];
        r.extend(self.cs.iter().map(|c| self.n(c, key).to_string()));
        r.push(self.total(key).to_string());
        r
    }

    /// One row per key below `prefix`, the larger totals first.
    fn rows_under(&self, prefix: &str, skip: &[&str]) -> Vec<Vec<String>> {
        let mut keys: Vec<String> = self.cs.iter().flat_map(|c| c.flat.keys()).filter(|k| k.starts_with(prefix)).cloned().collect();
        keys.sort();
        keys.dedup();
        keys.sort_by_key(|k| std::cmp::Reverse(self.total(k)));
        keys.iter().filter(|k| !skip.contains(&&k[prefix.len()..])).map(|k| self.row(&k[prefix.len()..], k)).collect()
    }

    fn section(&mut self, title: &str, first: &str, rows: Vec<Vec<String>>) {
        self.out.push_str(&format!("**{title}**\n\n"));
        let head = self.head(first);
        table(&mut self.out, &head, &rows);
    }
}

fn check_row(c: &Corpus) -> Vec<String> {
    let k = &c.check;
    let by = |p: &str| k.pointer(p).and_then(Value::as_u64).unwrap_or(0);
    let empty = |p: &str| k.pointer(p).and_then(Value::as_array).is_none_or(Vec::is_empty);
    let st = |ok: bool| if ok { "ok" } else { "FAIL" };
    let c1 = format!("{}, {} files", st(empty("/check1/failures")), by("/check1/files"));
    let c2 = format!(
        "{}: {} of {} exact, {} differ only in `mut`, {} other",
        st(empty("/check2/failures") && empty("/check2/mut_differs")),
        by("/check2/exact"),
        by("/check2/files"),
        k["check2"]["mut_differs"].as_array().map_or(0, Vec::len),
        k["check2"]["failures"].as_array().map_or(0, Vec::len)
    );
    let t = &k["check3"];
    let mut ran = vec!["cargo check".to_string()];
    let mut ok = empty("/check3/cargo_check_failures") && empty("/check3/new_check_warnings");
    if !t["clippy"].is_null() {
        ran.push(format!(
            "clippy --all-targets ({} warnings, original {})",
            by("/check3/clippy/woven"),
            by("/check3/clippy/original")
        ));
        ok &= empty("/check3/clippy/new");
    }
    if !t["test"].is_null() {
        ran.push(format!("cargo test ({} tests)", test_count(t["test"]["summary"].as_str().unwrap_or(""))));
        ok &= t["test"]["passed"] == true;
    }
    let c3 = format!("{}: {}", st(ok), ran.join("; "));
    let c4ok = by("/check4/silently_filled") == 0 && empty("/check4/other_errors") && empty("/check4/weave_errors");
    vec![
        c.name.clone(),
        format!("{} files, {} refused", by("/files"), k["refused"].as_array().map_or(0, Vec::len)),
        c1,
        c2,
        c3,
        st(c4ok).to_string(),
    ]
}

/// The markdown tables for the crates whose check directories are given. `provenance` is printed first.
pub fn report(dirs: &[&Path], provenance: &str) -> R<String> {
    let cs: Vec<Corpus> = dirs.iter().map(|d| load(d)).collect::<R<_>>()?;
    let mut r = Report { cs: &cs, out: String::new() };
    r.out.push_str(&format!("{provenance}\n\n"));

    let rows = [
        ("files accepted", "files.accepted"),
        ("files refused", "files.refused"),
        ("unbound entries: locals", "check.unbound.locals"),
        ("unbound entries: hints", "check.unbound.hints"),
        ("unbound entries: subjects", "check.unbound.subjects"),
    ];
    let rows = rows.iter().map(|(l, k)| r.row(l, k)).collect();
    r.section("Files", "", rows);
    let rows = r.rows_under("files.refused_by_reason.", &[]);
    if !rows.is_empty() {
        r.section("Refusals by reason", "reason", rows);
    }

    let rows = r.rows_under("subjects.", &[]);
    r.section("Subjects", "kind", rows);
    let rows = r.rows_under("slots_filled.", &[]);
    r.section("Slots filled, by production", "slot", rows);

    let mut rows: Vec<Vec<String>> = vec![r.row("type slots", "types.slots"), r.row("mapped to the type language", "types.mapped"), r.row("host text", "types.host_text")];
    let pct = |m: u64, s: u64| (100 * m + s / 2).checked_div(s).map_or("-".to_string(), |p| format!("{p}%"));
    let mut share = vec!["mapped share".to_string()];
    share.extend(r.cs.iter().map(|c| pct(r.n(c, "types.mapped"), r.n(c, "types.slots"))));
    share.push(pct(r.total("types.mapped"), r.total("types.slots")));
    rows.push(share);
    r.section("Types", "", rows);
    let rows = r.rows_under("types.mapped_by_form.", &[]);
    r.section("Mapped forms", "form", rows);
    let rows = r.rows_under("types.host_text_by_form.", &[]);
    r.section("Host text forms", "form", rows);

    let mut rows = vec![r.row("locals", "holes.locals"), r.row("hints", "holes.hints")];
    rows.extend(r.rows_under("holes.hints_by_kind.", &[]).into_iter().map(|mut x| {
        x[0] = format!("hints: {}", x[0]);
        x
    }));
    rows.push(r.row("functions with holes", "holes.functions_with_holes"));
    r.section("Holes, as stripped", "", rows);

    let rows = ["original", "derived", "inserted_by_rustc", "files_differing"].iter().map(|k| r.row(k, &format!("check.mut.{k}"))).collect();
    r.section("`mut` on bindings, `self` included", "", rows);

    let mut rows = r.rows_under("left_in_body.macros.", &[]);
    rows.extend(r.rows_under("left_in_body.lint.", &[]));
    rows.extend(r.rows_under("left_in_body.function_modifiers.", &[]).into_iter().map(|mut x| {
        x[0] = format!("fn {}", x[0]);
        x
    }));
    r.section("Left in bodies", "", rows);

    r.out.push_str("**Checks.**\n\n");
    let head: Vec<String> = ["corpus", "strip", "1 bare", "2 source", "3 check, clippy, test (what ran)", "4 holes"].map(String::from).to_vec();
    let rows: Vec<Vec<String>> = r.cs.iter().map(check_row).collect();
    table(&mut r.out, &head, &rows);

    let mut rows = vec![r.row("holes dropped", "check.check4.dropped_total"), r.row("functions with holes", "check.check4.functions_with_holes")];
    rows.push(r.row("undetermined", "check.check4.undetermined"));
    rows.extend(r.rows_under("check.check4.undetermined_by_kind.", &[]).into_iter().map(|mut x| {
        x[0] = format!("undetermined: {}", x[0]);
        x
    }));
    rows.push(r.row("functions with one", "check.check4.functions_with_undetermined"));
    rows.push(r.row("uninferred", "check.check4.undetermined_by_cause.uninferred"));
    rows.push(r.row("rejected", "check.check4.undetermined_by_cause.rejected"));
    rows.push(r.row("follow-on errors", "check.check4.follow_on_errors"));
    rows.push(r.row("at a site never dropped", "check.check4.undetermined_at_unfilled_sites_never_dropped"));
    rows.push(r.row("silently filled", "check.check4.silently_filled"));
    r.section("Check 4, all holes dropped", "", rows);
    Ok(r.out)
}

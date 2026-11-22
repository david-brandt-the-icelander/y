//! E2, bare in practice (proposal 10): weave every corpus with all `local` types and body hints dropped, keeping the
//! signatures, and count the holes `rustc` cannot infer, per function and by kind. Computed from the output of
//! `bare-rust check` (check 4 does the weave and the counting; this reads `check.json` and the stripped files).
use crate::report::{load, table, Corpus};
use crate::R;
use std::collections::BTreeSet;
use std::path::Path;

fn pct(a: u64, b: u64) -> String {
    if b == 0 {
        "-".into()
    } else {
        format!("{:.1}%", 100.0 * a as f64 / b as f64)
    }
}

pub fn e2(dirs: &[&Path], provenance: &str) -> R<String> {
    let cs: Vec<Corpus> = dirs.iter().map(|d| load(d)).collect::<R<_>>()?;
    let get = |c: &Corpus, k: &str| c.flat.get(k).copied().unwrap_or(0);
    let mut out = format!("{provenance}\n\n");
    let mut head = vec!["".to_string()];
    head.extend(cs.iter().map(|c| c.name.clone()));
    head.push("all".into());
    let row = |label: &str, f: &dyn Fn(&Corpus) -> u64| -> Vec<String> {
        let v: Vec<u64> = cs.iter().map(f).collect();
        let mut r = vec![label.to_string()];
        r.extend(v.iter().map(u64::to_string));
        r.push(v.iter().sum::<u64>().to_string());
        r
    };
    let kinds = |prefix: &str| -> BTreeSet<String> { cs.iter().flat_map(|c| c.flat.keys().filter_map(|k| k.strip_prefix(prefix).map(str::to_string))).collect() };

    let mut rows = vec![
        row("files", &|c| get(c, "files.accepted")),
        row("functions", &|c| get(c, "subjects.function")),
        row("functions with a hole in the file", &|c| get(c, "check.check4.functions_with_holes")),
    ];
    rows.push(row("holes dropped", &|c| get(c, "check.check4.dropped_total")));
    for k in kinds("check.check4.dropped.") {
        rows.push(row(&format!("dropped: {k}"), &|c| get(c, &format!("check.check4.dropped.{k}"))));
    }
    rows.push(row("holes rustc could not infer", &|c| get(c, "check.check4.undetermined")));
    for k in kinds("check.check4.undetermined_by_kind.") {
        rows.push(row(&format!("undetermined: {k}"), &|c| get(c, &format!("check.check4.undetermined_by_kind.{k}"))));
    }
    rows.push(row("  inferred to a type the program rejects", &|c| get(c, "check.check4.undetermined_by_cause.rejected")));
    rows.push(row("  not inferred", &|c| get(c, "check.check4.undetermined_by_cause.uninferred")));
    rows.push(row("functions needing at least one hint", &|c| get(c, "check.check4.functions_with_undetermined")));
    rows.push(row("errors that follow from those holes", &|c| get(c, "check.check4.follow_on_errors")));
    rows.push(row("other errors", &|c| c.check["check4"]["other_errors"].as_array().map_or(0, |a| a.len() as u64)));
    rows.push(row("holes silently filled by a guess", &|c| get(c, "check.check4.silently_filled")));
    table(&mut out, &head, &rows);

    let mut rows = vec![];
    let (mut nf, mut nh, mut nd) = (0, 0, 0);
    for c in &cs {
        let (f, h, d) = (get(c, "subjects.function"), get(c, "check.check4.functions_with_holes"), get(c, "check.check4.functions_with_undetermined"));
        let dropped = get(c, "check.check4.dropped_total");
        let u = get(c, "check.check4.undetermined");
        rows.push(vec![c.name.clone(), pct(h, f), pct(d, f), pct(d, h), pct(u, dropped)]);
        (nf, nh, nd) = (nf + f, nh + h, nd + d);
    }
    let (dr, un): (u64, u64) = cs.iter().fold((0, 0), |a, c| (a.0 + get(c, "check.check4.dropped_total"), a.1 + get(c, "check.check4.undetermined")));
    rows.push(vec!["all".into(), pct(nh, nf), pct(nd, nf), pct(nd, nh), pct(un, dr)]);
    let head2: Vec<String> = ["", "functions with a hole / functions", "functions needing a hint / functions", "functions needing a hint / functions with a hole", "undetermined / dropped holes"].map(String::from).to_vec();
    out.push_str("**Shares.**\n\n");
    table(&mut out, &head2, &rows);

    out.push_str("**Undetermined holes.** Each hole `rustc` could not determine from the signatures once the file's `locals` and `hints` were dropped, with the place in the bare body and the entry that would fill it.\n\n");
    for c in &cs {
        let list = c.check["check4"]["undetermined_holes"].as_array().cloned().unwrap_or_default();
        out.push_str(&format!("*{}* ({} holes)\n\n", c.name, list.len()));
        if list.is_empty() {
            out.push_str("None.\n\n");
            continue;
        }
        for h in &list {
            let what = match h["key"].as_str() {
                Some(k) => format!("local `{k}`"),
                None => format!("{} #{}", h["kind"].as_str().unwrap_or("?"), h["n"]),
            };
            out.push_str(&format!(
                "- `{}:{}:{}` `{}`: {what}, {} [{}] {}\n",
                h["file"].as_str().unwrap_or(""),
                h["line"],
                h["column"],
                h["subject"].as_str().unwrap_or(""),
                h["how"].as_str().unwrap_or(""),
                h["code"].as_str().unwrap_or(""),
                h["message"].as_str().unwrap_or("").replace('|', "\\|")
            ));
        }
        out.push('\n');
    }
    Ok(out)
}

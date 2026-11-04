//! The host profile of a library file (`libraries/host-<host>-<version>.json`): what the build and the host language
//! decide, and no subject can: the panic strategy, where a panic's location comes from, and the arms of each macro,
//! one signature per arm. It is a file of its own because none of it is a promise of an item of `std`.
use crate::json::canonical;
use crate::types::Ty;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The build settings a `given build` may name (language 6.12): the fixed list for Rust. A profile lists each with what it
/// is by default and where that is said; a name outside the list is not a setting a promise can depend on.
pub const BUILD_SETTINGS: [&str; 4] = ["overflow_checks", "debug_assertions", "panic=unwind", "panic=abort"];

/// The settings a `given build` may name: the profile's, or the fixed list when there is none.
pub fn builds(profile: Option<&Value>) -> BTreeSet<String> {
    match profile.and_then(|p| p["build"].as_array()) {
        Some(a) => a.iter().filter_map(|b| b["setting"].as_str().map(str::to_string)).collect(),
        None => BUILD_SETTINGS.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn host_path(libraries: &Path, host: &str, version: &str) -> PathBuf {
    libraries.join(format!("host-{host}-{version}.json"))
}

/// The number of arms of the profile, and everything wrong with it. `file` is the library it profiles.
pub fn validate(p: &Value, file: &Value, text: Option<&str>) -> (usize, Vec<String>) {
    let mut errs = vec![];
    if p["format"] != "y-host/0" {
        errs.push("host profile: format is `y-host/0`".to_string());
    }
    if p["host"] != file["host"] {
        errs.push(format!("host profile: host is {}, the library's is {}", p["host"], file["host"]));
    }
    if !matches!(p["panic"]["strategy"].as_str(), Some("unwind" | "abort")) {
        errs.push("host profile: panic.strategy is `unwind` or `abort`".into());
    }
    if p["panic"]["location"]["attribute"].as_str().is_none_or(|a| !a.starts_with("#[")) || p["panic"]["location"]["reports"].as_str().is_none() {
        errs.push("host profile: panic.location names the attribute (`#[track_caller]`) and whose location it reports".into());
    }
    let mut seen = BTreeSet::new();
    for b in p["build"].as_array().into_iter().flatten() {
        let name = b["setting"].as_str().unwrap_or("");
        if !BUILD_SETTINGS.contains(&name) {
            errs.push(format!("host profile: build setting `{name}` is not one of {}", BUILD_SETTINGS.join(", ")));
        }
        if !seen.insert(name) {
            errs.push(format!("host profile: build setting `{name}` is listed twice"));
        }
        if b["default"].as_str().is_none_or(str::is_empty) || b["source"].as_str().is_none_or(str::is_empty) {
            errs.push(format!("host profile: build setting `{name}` has its `default` and a `source`"));
        }
    }
    if p["build"].is_array() {
        for s in BUILD_SETTINGS.iter().filter(|s| !seen.contains(**s)) {
            errs.push(format!("host profile: build setting `{s}` is not listed"));
        }
    } else {
        errs.push("host profile: `build` lists the build settings a `given build` may name".into());
    }
    let macros: BTreeSet<&str> = file["subjects"].as_array().into_iter().flatten().filter(|s| s["kind"] == "macro").filter_map(|s| s["name"].as_str()).collect();
    let mut n = 0;
    for (name, m) in p["macros"].as_object().into_iter().flatten() {
        if !macros.contains(name.as_str()) {
            errs.push(format!("host profile: macro `{name}` is not a macro subject of the library"));
        }
        let arms = m["arms"].as_array().map_or(&[][..], Vec::as_slice);
        if arms.is_empty() {
            errs.push(format!("host profile: macro `{name}` has no arms"));
        }
        let bare = name.trim_end_matches('!').rsplit("::").next().unwrap_or("");
        for (i, a) in arms.iter().enumerate() {
            n += 1;
            let at = format!("host profile: `{name}` arm {i}");
            if a["pattern"].as_str().is_none_or(|s| !s.starts_with(&format!("{bare}!"))) {
                errs.push(format!("{at}: the pattern starts with `{bare}!`"));
            }
            if a["source"].as_str().is_none_or(str::is_empty) {
                errs.push(format!("{at}: no source"));
            }
            let types = a["params"].as_array().into_iter().flatten().map(|q| &q["type"]).chain(std::iter::once(&a["result"]));
            for t in types.filter_map(Value::as_str).filter(|t| *t != "never") {
                if let Err(e) = Ty::parse(t) {
                    errs.push(format!("{at}: {e}"));
                }
            }
        }
    }
    if text.is_some_and(|t| t != canonical(p)) {
        errs.push("host profile: not in canonical form (`bare-rust libfmt <file>` rewrites it)".into());
    }
    (n, errs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_profile_names_its_strategy_and_arms() {
        let lib = json!({"host":"rust","subjects":[{"kind":"macro","name":"std::vec!"}]});
        let p = json!({"format":"y-host/0","host":"rust","panic":{"strategy":"unwind","location":{"attribute":"#[track_caller]","reports":"caller"}},
            "build":BUILD_SETTINGS.iter().map(|s| json!({"setting":s,"default":"d","source":"s"})).collect::<Vec<_>>(),
            "macros":{"std::vec!":{"arms":[{"pattern":"vec![]","params":[],"result":"list<element type=u8>","source":"s"}]}}});
        assert_eq!(validate(&p, &lib, None), (1, vec![]));
        let mut bad = p.clone();
        bad["panic"]["strategy"] = json!("loud");
        bad["macros"]["std::nope!"] = json!({"arms":[]});
        bad["build"] = json!([{"setting":"opt_level","default":"d","source":"s"}]);
        let e = validate(&bad, &lib, None).1.join("\n");
        assert!(e.contains("strategy") && e.contains("not a macro subject") && e.contains("no arms") && e.contains("`opt_level` is not one of") && e.contains("`overflow_checks` is not listed"), "{e}");
        assert_eq!(builds(None).len(), 4);
        assert_eq!(builds(Some(&p)), BUILD_SETTINGS.iter().map(|s| s.to_string()).collect());
    }
}

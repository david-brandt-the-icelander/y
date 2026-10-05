//! Stage 2, part A: the externals lists, the library file format, and the mechanical half of check 5.
//! Needs the pinned crates: run `corpora/fetch.sh` first.
use bare_rust::check::{check_crate, CheckOpts};
use bare_rust::externals::{externals, Kind};
use bare_rust::json::canonical;
use bare_rust::libcheck::{check_signatures, libcheck, missing, validate};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn libraries() -> PathBuf {
    root().join("libraries")
}

fn corpus(name: &str) -> PathBuf {
    let dir = match name {
        "eve_cache" => root().join("tests/golden/crate"),
        n => root().join("corpora/src").join(n),
    };
    assert!(dir.join("Cargo.toml").exists(), "{} is not fetched: run tools/bare-rust/corpora/fetch.sh", dir.display());
    dir
}

fn std_file() -> (Value, String) {
    let text = fs::read_to_string(libraries().join("std-1.97.0.constraints.json")).unwrap();
    (serde_json::from_str(&text).unwrap(), text)
}

const EXEMPLARS: [&str; 4] = ["std::collections::HashMap::get", "std::primitive::str::chars", "std::str::Chars", "std::cmp::max"];

/// The library file with only part A's four exemplars: the synthetic subjects of a test go on top of them.
fn exemplars_only() -> Value {
    let (mut f, _) = std_file();
    f["subjects"].as_array_mut().unwrap().retain(|s| EXEMPLARS.contains(&s["name"].as_str().unwrap()));
    f
}

/// A library directory whose std file lacks the named subjects, with the externals lists copied: an incomplete library.
fn incomplete_libraries(name: &str, without: &[&str]) -> PathBuf {
    let dir = work(name);
    fs::create_dir_all(dir.join("externals")).unwrap();
    for c in ["strsim", "fnv", "eve_cache"] {
        fs::copy(libraries().join("externals").join(format!("{c}.json")), dir.join("externals").join(format!("{c}.json"))).unwrap();
    }
    let (mut f, _) = std_file();
    f["subjects"].as_array_mut().unwrap().retain(|s| !without.contains(&s["name"].as_str().unwrap()));
    fs::write(dir.join("std-1.97.0.constraints.json"), canonical(&f)).unwrap();
    dir
}

fn work(name: &str) -> PathBuf {
    let d = root().join("target/bare-rust-test").join(name);
    let _ = fs::remove_dir_all(&d);
    d
}

fn target() -> PathBuf {
    bare_rust::cargo::target_dir(&root())
}

fn has(e: &bare_rust::externals::Externals, k: Kind, p: &str) -> bool {
    e.items.contains_key(&(k, p.to_string()))
}

// ---------- externals ----------

#[test]
fn externals_of_the_golden_crate() {
    let (name, e) = externals(&corpus("eve_cache"), true).unwrap();
    assert_eq!(name, "eve_cache 0.1.0");
    assert!(has(&e, Kind::Function, "std::collections::HashMap::get"));
    assert!(has(&e, Kind::Function, "std::collections::HashMap::insert"));
    assert!(has(&e, Kind::Type, "std::collections::HashMap") && has(&e, Kind::Type, "std::option::Option"));
    // the derive is a macro and names the traits it implements
    assert!(has(&e, Kind::Macro, "std::hash::Hash!") && has(&e, Kind::Trait, "std::hash::Hash"));
    // the code a derive generates is not a call the author wrote
    assert!(!has(&e, Kind::Function, "std::hash::Hash::hash") && !has(&e, Kind::Function, "std::cmp::PartialEq::eq"));
    assert_eq!(e.items.len(), 10);
}

#[test]
fn externals_of_fnv_are_normalised_to_public_std_paths() {
    let (_, e) = externals(&corpus("fnv"), true).unwrap();
    // implemented trait methods, called trait methods, inherent methods on primitives, operators folded
    for (k, p) in [
        (Kind::Function, "std::hash::Hasher::write"),
        (Kind::Function, "std::hash::Hasher::finish"),
        (Kind::Function, "std::default::Default::default"),
        (Kind::Function, "std::primitive::u64::wrapping_mul"),
        (Kind::Function, "std::primitive::slice::len"),
        (Kind::Type, "std::hash::BuildHasherDefault"),
        (Kind::Type, "std::collections::HashMap"),
        (Kind::Trait, "std::ops::Deref"),
        (Kind::Macro, "std::assert_eq!"),
    ] {
        assert!(has(&e, k, p), "missing {p}");
    }
    assert!(e.paths().all(|(_, p)| p.starts_with("std::")), "every path is under std: {:?}", e.paths().collect::<Vec<_>>());
    assert!(!e.items.keys().any(|(_, p)| p.contains("::fmt::rt::")), "macro internals are left out");
    let (hw, _) = (&e.items[&(Kind::Function, "std::hash::Hasher::write".to_string())], ());
    assert_eq!(hw.trait_path.as_deref(), Some("std::hash::Hasher"));
    assert!(hw.roles.contains("implement") && hw.roles.contains("call"));
}

#[test]
fn externals_of_strsim_cover_what_the_source_calls() {
    let (_, e) = externals(&corpus("strsim"), true).unwrap();
    for (k, p) in [
        (Kind::Function, "std::primitive::str::chars"),
        (Kind::Function, "std::iter::Iterator::zip"),
        (Kind::Function, "std::collections::HashMap::with_capacity"),
        (Kind::Function, "std::collections::hash_map::Entry::or_insert"),
        (Kind::Function, "std::cmp::min"),
        (Kind::Function, "std::mem::swap"),
        (Kind::Function, "std::primitive::char::is_whitespace"),
        (Kind::Function, "std::fmt::Display::fmt"),
        (Kind::Const, "std::primitive::char::MAX"),
        (Kind::Type, "std::str::Chars"),
        (Kind::Type, "std::iter::Zip"),
        (Kind::Trait, "std::error::Error"),
        (Kind::Macro, "std::assert_eq!"),
        (Kind::Macro, "std::vec!"),
        (Kind::Macro, "std::fmt::Debug!"),
    ] {
        assert!(has(&e, k, p), "missing {p}");
    }
}

#[test]
fn committed_externals_lists_are_current() {
    for name in ["strsim", "fnv", "eve_cache"] {
        let (krate, e) = externals(&corpus(name), true).unwrap();
        let committed: Value = serde_json::from_str(&fs::read_to_string(libraries().join("externals").join(format!("{name}.json"))).unwrap()).unwrap();
        let mut now = e.to_json(&krate, true);
        // the toolchain may differ in patch level; the items may not
        now["rustc"] = committed["rustc"].clone();
        assert_eq!(now["items"], committed["items"], "{name}: run `bare-rust externals corpora/src/{name} --all-targets` again");
        assert_eq!(now["counts"], committed["counts"], "{name}");
    }
}

#[test]
fn without_all_targets_test_code_is_left_out() {
    let (_, lib_only) = externals(&corpus("strsim"), false).unwrap();
    let (_, all) = externals(&corpus("strsim"), true).unwrap();
    assert!(lib_only.items.len() < all.items.len());
    assert!(has(&all, Kind::Function, "std::primitive::f64::abs"), "from tests/lib.rs");
    assert!(!has(&lib_only, Kind::Function, "std::primitive::f64::abs"));
}

// ---------- the library file ----------

#[test]
fn the_library_file_is_valid_and_canonical() {
    let (file, text) = std_file();
    assert_eq!(validate(&file, Some(&text)), Vec::<String>::new());
    assert_eq!(canonical(&file), text);
    assert_eq!(file["crate"], "std 1.97.0");
    assert_eq!(file["status"], json!({"assumed": "hand-written"}));
    // the exemplars, and every external item of the corpora after them
    let names: Vec<&str> = file["subjects"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    for n in EXEMPLARS {
        assert!(names.contains(&n), "{n}");
    }
    assert_eq!(names.len(), 105, "49 functions, 25 types, 19 traits, 11 macros and 1 const");
}

#[test]
fn every_clause_cites_its_source_and_every_unexpressed_clause_its_reason() {
    use bare_rust::libstats::{reason_label, source_kind};
    let (file, _) = std_file();
    let mut labels = std::collections::BTreeSet::new();
    for s in file["subjects"].as_array().unwrap() {
        for c in s["clauses"].as_array().unwrap() {
            let at = format!("{} / {}", s["name"], c["text"]);
            let src = c["source"].as_str().unwrap();
            assert!(src.starts_with("1.97.0 ") || src.starts_with("hashbrown"), "{at}: source {src}");
            if c["kind"] == "unexpressed" {
                let r = c["reason"].as_str().unwrap();
                assert!(r.contains(": "), "{at}: a reason starts with what the language lacks, then a colon: {r}");
                labels.insert(reason_label(r).to_string());
            } else {
                // a stated clause says where it comes from, and a body or an attribute has a status of its own
                assert!(source_kind(src) != "other", "{at}: {src}");
                assert!(!c["status"].is_null() || src.contains(", body: "), "{at}: a documented clause has the status of its source");
            }
        }
    }
    assert!(labels.len() >= 20 && labels.len() <= 30, "the labels are a short list: {labels:?}");
}

#[test]
fn the_schema_accepts_both_forms_of_status_and_refuses_the_rest() {
    let (file, _) = std_file();
    let mut f = file.clone();
    f["status"] = json!("lifted");
    assert_eq!(validate(&f, None), Vec::<String>::new());
    f["status"] = json!({"assumed": ""});
    assert!(!validate(&f, None).is_empty());
    f["status"] = json!({"guessed": "x"});
    assert!(!validate(&f, None).is_empty());
    let mut f = file.clone();
    f["subjects"][0]["clauses"][0]["kind"] = json!("hope");
    assert!(validate(&f, None).iter().any(|e| e.contains("schema")));
    let mut f = file.clone();
    f["subjects"][0]["clauses"][0]["reason"] = json!("only an unexpressed clause has a reason");
    assert!(validate(&f, None).iter().any(|e| e.contains("schema")));
}

#[test]
fn stage_one_files_still_validate() {
    let j: Value = serde_json::from_str(&fs::read_to_string(root().join("tests/golden/cache.constraints.json")).unwrap()).unwrap();
    assert_eq!(validate(&j, None).iter().filter(|e| e.starts_with("schema")).count(), 0);
}

#[test]
fn coverage_names_the_missing_items_by_path() {
    let (_, e) = externals(&corpus("eve_cache"), true).unwrap();
    let items: Vec<(Kind, String)> = e.paths().map(|(k, p)| (k, p.to_string())).collect();
    // every item of the golden crate has an entry
    let (file, _) = std_file();
    assert_eq!(missing(&items, &file), vec![]);
    // with only the exemplars, HashMap::get is covered and the rest of the golden crate's items are not
    let m = missing(&items, &exemplars_only());
    assert!(!m.contains(&(Kind::Function, "std::collections::HashMap::get".to_string())));
    assert!(m.contains(&(Kind::Function, "std::collections::HashMap::insert".to_string())));
    assert_eq!(m.len(), 9);
}

// ---------- signature check ----------

fn subject_mut<'a>(f: &'a mut Value, name: &str) -> &'a mut Value {
    f["subjects"].as_array_mut().unwrap().iter_mut().find(|s| s["name"] == name).unwrap()
}

#[test]
fn the_whole_file_compiles_against_the_real_std() {
    let (file, _) = std_file();
    let r = check_signatures(&file, &work("sig-all"), &target()).unwrap();
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    for n in EXEMPLARS {
        assert!(r.checked.contains(&n.to_string()), "{n} was not checked: {:?}", r.unchecked);
    }
    // what the checker cannot do is the bang macros, and a trait with generics is only found by its path
    assert_eq!(r.unchecked.keys().collect::<Vec<_>>(), vec!["macro"]);
    assert_eq!(r.unchecked["macro"].len(), 4, "{:?}", r.unchecked);
    assert_eq!(r.resolved.len(), 5, "{:?}", r.resolved);
    assert_eq!(r.checked.len() + r.resolved.len() + r.unchecked_total(), 105);
}

#[test]
fn a_derive_of_a_trait_with_a_supertrait_is_checked_with_it() {
    let mut f = exemplars_only();
    let subjects = f["subjects"].as_array_mut().unwrap();
    for n in ["std::cmp::Eq!", "std::marker::Copy!"] {
        subjects.push(json!({ "kind": "macro", "name": n, "form": "derive" }));
    }
    let r = check_signatures(&f, &work("sig-derive"), &target()).unwrap();
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    assert!(r.checked.contains(&"std::cmp::Eq!".to_string()) && r.checked.contains(&"std::marker::Copy!".to_string()));
}

#[test]
fn a_wrong_signature_is_reported_against_its_subject() {
    let (file, _) = std_file();
    let mut f = file.clone();
    // the result borrows from the key, not the map
    subject_mut(&mut f, "std::collections::HashMap::get")["result"]["hold"]["from"] = json!("k");
    // a bound the real function needs, left out
    subject_mut(&mut f, "std::cmp::max")["generics"][0]["bounds"] = json!([]);
    // a type that is not the real one
    subject_mut(&mut f, "std::primitive::str::chars")["result"]["type"] = json!("std::str::CharIndices");
    // no such item
    f["subjects"].as_array_mut().unwrap().push(json!({
        "kind": "function", "name": "std::collections::HashMap::fetch",
        "params": [{"name": "self", "type": "ref<text>", "access": {"rights": "read", "guarantee": "readers"}}]
    }));
    let r = check_signatures(&f, &work("sig-wrong"), &target()).unwrap();
    let by: std::collections::BTreeMap<&str, &str> = r.mismatches.iter().map(|(n, m)| (n.as_str(), m.as_str())).collect();
    assert!(by["std::collections::HashMap::get"].starts_with("E0308") || by["std::collections::HashMap::get"].contains("lifetime"), "{by:?}");
    assert!(by["std::cmp::max"].contains("Ord"), "{by:?}");
    assert!(by["std::primitive::str::chars"].starts_with("E0308"), "{by:?}");
    assert!(by.contains_key("std::collections::HashMap::fetch"), "{by:?}");
    assert_eq!(r.mismatches.len(), 4, "{by:?}");
    assert!(r.checked.contains(&"std::str::Chars".to_string()), "the others are still checked");
}

#[test]
fn trait_methods_derives_consts_and_unfixed_generics() {
    let mut f = exemplars_only();
    let subjects = f["subjects"].as_array_mut().unwrap();
    subjects.push(json!({
        "kind": "function", "name": "std::iter::Iterator::zip",
        "generics": [{"name": "Self", "bounds": ["std::iter::Iterator"]}, {"name": "U", "bounds": ["std::iter::IntoIterator"]}],
        "params": [{"name": "self", "type": "Self"}, {"name": "other", "type": "U"}],
        "result": {"type": {"host": "std::iter::Zip<Self, U::IntoIter>"}}
    }));
    subjects.push(json!({
        "kind": "function", "name": "std::convert::From::from", "trait": "std::convert::From<T>",
        "generics": [{"name": "Self", "bounds": []}, {"name": "T", "bounds": []}],
        "params": [{"name": "value", "type": "T"}],
        "result": {"type": "Self"}
    }));
    subjects.push(json!({ "kind": "macro", "name": "std::hash::Hash!", "form": "derive" }));
    subjects.push(json!({ "kind": "macro", "name": "std::vec!", "form": "bang" }));
    subjects.push(json!({ "kind": "const", "name": "std::primitive::char::MAX", "type": "text where length(value) == 1 as 'char'" }));
    subjects.push(json!({ "kind": "trait", "name": "std::hash::Hasher" }));
    subjects.push(json!({ "kind": "type", "name": "std::collections::HashMap", "generics": [{"name": "K", "bounds": []}, {"name": "V", "bounds": []}, {"name": "S", "bounds": []}] }));
    subjects.push(json!({ "kind": "function", "name": "std::mem::size_of", "generics": [{"name": "T", "bounds": []}], "params": [], "result": {"type": "u64 as 'usize'"} }));
    let r = check_signatures(&f, &work("sig-more"), &target()).unwrap();
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    for n in ["std::iter::Iterator::zip", "std::convert::From::from", "std::hash::Hash!", "std::primitive::char::MAX", "std::hash::Hasher", "std::collections::HashMap"] {
        assert!(r.checked.contains(&n.to_string()), "{n}: checked {:?}, unchecked {:?}", r.checked, r.unchecked);
    }
    assert_eq!(r.unchecked["macro"], vec!["std::vec!".to_string()]);
    // `size_of::<T>` cannot be inferred from `fn() -> usize`: counted as unchecked, not as wrong
    assert!(r.unchecked["generics not fixed by the signature"].contains(&"std::mem::size_of".to_string()), "{:?}", r.unchecked);
}

#[test]
fn a_type_with_the_wrong_arity_is_a_mismatch() {
    let mut f = exemplars_only();
    f["subjects"].as_array_mut().unwrap().push(json!({ "kind": "type", "name": "std::vec::Vec" }));
    let r = check_signatures(&f, &work("sig-arity"), &target()).unwrap();
    assert!(r.mismatches.iter().any(|(n, m)| n == "std::vec::Vec" && m.starts_with("E0107")), "{:?}", r.mismatches);
}

// ---------- check 5 through the commands ----------

#[test]
fn libcheck_passes_for_all_three_corpora_with_no_item_missing() {
    for (name, items) in [("eve_cache", 10), ("fnv", 32), ("strsim", 94)] {
        let r = libcheck(&corpus(name), &libraries(), &work(&format!("libcheck-{name}")), &target()).unwrap();
        assert_eq!(r.items, items, "{name}");
        assert!(r.coverage_ok() && r.missing.is_empty(), "{name}: {:?}", r.missing);
        assert!(r.valid_ok() && r.sigs_ok(), "{name}: {:?} {:?}", r.invalid, r.sigs.as_ref().map(|s| &s.mismatches));
        assert!(r.ok(true), "{name}");
    }
}

#[test]
fn libcheck_reports_coverage_validity_and_signatures() {
    let libs = incomplete_libraries("libraries-no-insert", &["std::collections::HashMap::insert", "std::hash::Hash", "std::hash::Hash!"]);
    let r = libcheck(&corpus("eve_cache"), &libs, &work("libcheck-golden"), &target()).unwrap();
    assert_eq!(r.items, 10);
    assert!(r.valid_ok() && r.sigs_ok());
    assert!(!r.coverage_ok());
    assert_eq!(r.missing.len(), 3);
    assert!(r.ok(false) && !r.ok(true));
    assert!(r.sigs.as_ref().unwrap().checked.len() >= 3);
}

#[test]
fn libcheck_without_a_library_file_says_so() {
    let empty = work("no-library");
    fs::create_dir_all(empty.join("externals")).unwrap();
    fs::copy(libraries().join("externals/eve_cache.json"), empty.join("externals/eve_cache.json")).unwrap();
    let r = libcheck(&corpus("eve_cache"), &empty, &work("libcheck-none"), &target()).unwrap();
    assert!(r.skipped.is_some() && r.missing.len() == 10 && r.sigs.is_none());
}

fn check_opts(libraries: PathBuf, strict: bool) -> CheckOpts {
    CheckOpts { work: root().join("target/bare-rust-check"), target: target(), tests: true, full: false, libraries: Some(libraries), strict_libraries: strict }
}

#[test]
fn check_runs_check_5_for_a_crate_with_an_externals_list() {
    let r = check_crate(&corpus("eve_cache"), &check_opts(libraries(), true)).unwrap();
    let c5 = r.c5.as_ref().expect("check 5 ran");
    assert!(r.c5_ok() && r.ok(), "check 5: {:?} {:?}", c5.invalid, c5.missing);
    assert!(c5.missing.is_empty());
    // an incomplete library: reported, and a failure only when strict
    let libs = incomplete_libraries("check-libraries-incomplete", &["std::collections::HashMap::insert"]);
    let r = check_crate(&corpus("eve_cache"), &check_opts(libs.clone(), false)).unwrap();
    assert!(r.c5_ok() && r.ok() && r.c5.as_ref().unwrap().missing.len() == 1);
    let r = check_crate(&corpus("eve_cache"), &check_opts(libs, true)).unwrap();
    assert!(!r.c5_ok() && !r.ok());
    // and a crate without a list is not subject to it
    let r = check_crate(&corpus("eve_cache"), &check_opts(work("empty-libraries"), true)).unwrap();
    assert!(r.c5.is_none());
}

// ---------- the counts ----------

#[test]
fn libstats_counts_what_the_file_holds() {
    let (file, _) = std_file();
    let r = check_signatures(&file, &work("stats-sigs"), &target()).unwrap();
    let t = bare_rust::libstats::libstats(&file, Some(&r));
    for want in ["library std 1.97.0 (105 subjects)", "function (22 are trait methods)", "unexpressed", "no sequence value", "path-resolved only: std::cmp::PartialEq", "type slots (137); 118 map to the type language and 19 are host text"] {
        assert!(t.contains(want), "{want}: {t}");
    }
    assert!(!t.contains("reasons carry no label"), "{t}");
}

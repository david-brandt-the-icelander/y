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
fn externals_record_the_impl_each_call_resolves_to() {
    let (_, e) = externals(&corpus("strsim"), true).unwrap();
    let impls = |p: &str| e.items[&(Kind::Function, p.to_string())].impls.iter().cloned().collect::<Vec<_>>();
    // a trait method names the impl of the trait for the head of the self type, a method of the type its own impl
    assert!(impls("std::iter::Iterator::zip").contains(&"impl std::iter::Iterator for std::str::Chars".to_string()), "{:?}", impls("std::iter::Iterator::zip"));
    assert_eq!(impls("std::collections::HashMap::with_capacity"), vec!["impl std::collections::HashMap".to_string()]);
    assert_eq!(impls("std::primitive::char::is_whitespace"), vec!["impl char".to_string()]);
    // a free function has none, and the committed list carries them
    assert!(e.items[&(Kind::Function, "std::cmp::min".to_string())].impls.is_empty());
    let (_, fnv) = externals(&corpus("fnv"), true).unwrap();
    assert_eq!(fnv.items[&(Kind::Function, "std::convert::From::from".to_string())].impls.iter().next().map(String::as_str), Some("impl std::convert::From<u8> for u64"));
    let committed: Value = serde_json::from_str(&fs::read_to_string(libraries().join("externals/fnv.json")).unwrap()).unwrap();
    let from = committed["items"].as_array().unwrap().iter().find(|i| i["path"] == "std::convert::From::from").unwrap();
    assert_eq!(from["impls"], json!(["impl std::convert::From<u8> for u64"]));
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
    // the impls of B1, and the methods of those impls, are subjects of their own beside the entries for the external items
    let own = file["subjects"].as_array().unwrap().iter().filter(|s| s["kind"] != "impl" && s["impl"].is_null()).count();
    assert_eq!(own, 113, "49 functions, 27 types, 19 traits, 11 macros and 1 const: the 105 external items, Ordering for the laws, and OccupiedEntry for a view; and for B2's `given` items the trait methods Hash::hash, PartialEq::eq, PartialOrd::lt, Ord::cmp, Ord::min and the trait PartialOrd");
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
fn clause_text_is_checked_for_form_and_names_not_only_for_its_head() {
    let (file, _) = std_file();
    let at = |name: &str, f: &mut Value, prefix: &str, text: &str| {
        let s = subject_mut(f, name);
        let c = s["clauses"].as_array_mut().unwrap().iter_mut().find(|c| c["text"].as_str().unwrap().starts_with(prefix)).unwrap();
        c["text"] = json!(text);
    };
    // a host path is quoted, and balanced brackets and quotes are closed
    let mut f = file.clone();
    at("std::cmp::Eq", &mut f, "law reflexive", "law reflexive : all a : Self : std::cmp::PartialEq::eq(a, a)");
    at("std::cmp::max", &mut f, "ensure ge_both", "ensure ge_both : (result >= v1 and result >= v2");
    at("std::cmp::min", &mut f, "ensure le_both", "ensure le_both : result <= 'v1");
    at("std::mem::swap", &mut f, "ensure x_gets_y", "ensure x_gets_y : x == old(zzz)");
    at("std::cmp::Ord", &mut f, "law transitive", "law transitive : all a : Self : ('<Self as std::cmp::Ord>::cmp'(a, b) is Less)");
    let e = validate(&f, None).join("\n");
    for want in ["not quoted", "unclosed `(`", "unterminated `'`", "unknown name `zzz`", "unknown name `b`"] {
        assert!(e.contains(want), "{want}: {e}");
    }
    // a failure a clause names is declared in the file, and a variant it matches is a variant of a subject
    let mut f = file.clone();
    f.as_object_mut().unwrap().remove("failures");
    assert!(validate(&f, None).iter().any(|e| e.contains("failure `panic` is not declared")));
    let mut f = file.clone();
    subject_mut(&mut f, "std::cmp::Ordering").as_object_mut().unwrap().remove("variants");
    assert!(validate(&f, None).iter().any(|e| e.contains("unknown name `Less`")));
    assert_eq!(validate(&file, None), Vec::<String>::new());
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

// ---------- class A: views, payloads, budget and layout, derives, the host profile ----------

fn errors(f: &Value) -> String {
    validate(f, None).join("\n")
}

#[test]
fn a_view_is_declared_bound_to_a_host_function_and_usable_in_clauses() {
    let (file, _) = std_file();
    assert_eq!(errors(&file), "");
    // an undeclared view name is an unknown name
    let mut f = file.clone();
    f.as_object_mut().unwrap().remove("views");
    assert!(errors(&f).contains("unknown name `capacity`"), "{}", errors(&f));
    // the host path is quoted, under the subject the view reads, and the subject is in the file
    let mut f = file.clone();
    f["views"][0]["host"] = json!("'std::vec::Vec::capacity'");
    f["views"][1]["of"] = json!("'std::nowhere::T'");
    let e = errors(&f);
    assert!(e.contains("not a host path under `std::collections::HashMap`") && e.contains("not a subject of the file"), "{e}");
    f["views"][1] = f["views"][0].clone();
    assert!(errors(&f).contains("declared twice"));
    // the host path resolves in rustc: 3 views, none unchecked; a path that is not a method is a mismatch
    let (file, _) = std_file();
    let r = check_signatures(&file, &work("views-ok"), &target()).unwrap();
    assert_eq!((r.views_checked, r.views_unchecked.len(), r.mismatches.len()), (4, 0, 0), "{:?}", r.mismatches);
    let mut f = file.clone();
    f["views"][1]["host"] = json!("'std::vec::Vec::capacitee'");
    let r = check_signatures(&f, &work("views-bad"), &target()).unwrap();
    assert!(r.mismatches.iter().any(|(n, _)| n == "view capacity of std::vec::Vec"), "{:?}", r.mismatches);
}

#[test]
fn a_failure_carries_a_payload_that_a_clause_fills() {
    let (file, _) = std_file();
    let mut f = file.clone();
    let c = subject_mut(&mut f, "std::option::Option::expect")["clauses"][0].clone();
    assert_eq!(c["text"], "fails panic { message: msg, location: caller } when self is none");
    // a field the failure does not declare, or a payload that is not declared at all
    subject_mut(&mut f, "std::option::Option::expect")["clauses"][0]["text"] = json!("fails panic { detail: msg } when self is none");
    assert!(errors(&f).contains("`detail` is not a payload field of failure `panic`"), "{}", errors(&f));
    let mut f = file.clone();
    f["failures"]["panic"] = json!("runtime");
    assert!(errors(&f).contains("`message` is not a payload field"), "{}", errors(&f));
    let mut f = file.clone();
    f["failures"]["panic"]["payload"]["message"] = json!("list<");
    assert!(errors(&f).contains("payload field `message`"), "{}", errors(&f));
}

fn set_text(f: &mut Value, name: &str, kind: &str, text: &str) -> String {
    let c = subject_mut(f, name)["clauses"].as_array_mut().unwrap().iter_mut().find(|c| c["kind"] == kind || c["text"].as_str().is_some_and(|t| t.starts_with(kind))).unwrap();
    std::mem::replace(&mut c["text"], json!(text)).as_str().unwrap().to_string()
}

#[test]
fn budget_and_layout_are_clause_kinds() {
    let (file, _) = std_file();
    let mut f = file.clone();
    let was = set_text(&mut f, "std::collections::HashMap::new", "budget", "allocates at most 0 B");
    assert_eq!(was, "budget allocates at most 0 B per call");
    assert!(errors(&f).contains("a `budget` clause starts with `budget`"), "{}", errors(&f));
    set_text(&mut f, "std::collections::HashMap::new", "budget", "budget allocates at most 0 B per call and zzz");
    assert!(errors(&f).contains("unknown name `zzz`"));
    assert_eq!(set_text(&mut f, "std::hash::BuildHasherDefault", "layout", "layout round"), "size 0");
    assert!(errors(&f).contains("unknown name `round`"), "{}", errors(&f));
}

#[test]
fn named_functions_of_the_language_are_accepted_words() {
    let (file, _) = std_file();
    let mut f = file.clone();
    let was = set_text(&mut f, "std::primitive::f64::abs", "ensure unsigned", "ensure u : not sign_bit(result) and not is_nan(result) and total_order(self, result)");
    assert!(was.contains("sign_bit(result)"));
    assert_eq!(errors(&f), "");
    set_text(&mut f, "std::primitive::f64::abs", "ensure u ", "ensure u : not sign_bt(result)");
    assert!(errors(&f).contains("unknown name `sign_bt`"));
}

#[test]
fn a_derive_template_expands_per_field_into_clauses() {
    let (file, _) = std_file();
    // the derive macros name their template; a template that is not the macro's is refused
    let mut f = file.clone();
    assert_eq!(subject_mut(&mut f, "std::clone::Clone!")["clauses"][0]["text"], "derive Clone");
    subject_mut(&mut f, "std::clone::Clone!")["clauses"][0]["text"] = json!("derive Hash");
    assert!(errors(&f).contains("names a template that is not this derive's"), "{}", errors(&f));
    // a type that derives has fields, and its derives are templates
    let mut f = file.clone();
    let cache = json!({"kind": "type", "name": "std::demo::Cache", "visible": "public", "derive": ["Clone", "PartialEq"], "fields": [{"name": "entries", "type": "u64", "hold": {"kind": "owns"}}, {"name": "hits", "type": "u64", "hold": {"kind": "owns"}}]});
    f["subjects"].as_array_mut().unwrap().push(cache.clone());
    assert_eq!(errors(&f), "");
    let shown = bare_rust::derive::show(&f);
    assert!(shown.contains("ensure clone_entries : result.entries == self.entries") && shown.contains("ensure eq_hits : self.hits != other.hits implies not result"), "{shown}");
    // the expanded clauses are ordinary: they pass the clause lint
    for c in bare_rust::derive::expand(&cache) {
        let scope = bare_rust::clause::Scope { names: ["Self", "other"].iter().map(|s| s.to_string()).collect(), ..Default::default() };
        let k = c["kind"].as_str().unwrap();
        assert_eq!(bare_rust::clause::lint(k, c["text"].as_str().unwrap(), &[k], &scope), Vec::<String>::new());
    }
    f["subjects"][106]["derive"] = json!(["Serialize"]);
    assert!(errors(&f).contains("not a derive template"));
    f["subjects"][106]["derive"] = json!(["Clone"]);
    f["subjects"][106].as_object_mut().unwrap().remove("fields");
    assert!(errors(&f).contains("`derive` is for a type with `fields` or `variants`"));
}

#[test]
fn the_host_profile_holds_macro_arms_and_the_panic_strategy() {
    let lib = libraries();
    let text = fs::read_to_string(bare_rust::host::host_path(&lib, "rust", "1.97.0")).unwrap();
    let p: Value = serde_json::from_str(&text).unwrap();
    let (file, _) = std_file();
    assert_eq!(bare_rust::host::validate(&p, &file, Some(&text)), (10, vec![]));
    assert_eq!(p["panic"]["strategy"], "unwind");
    // the four macros whose clauses said "no macro arguments" have their arms here
    for m in ["std::assert_eq!", "std::panic!", "std::vec!", "std::write!"] {
        assert!(p["macros"][m]["arms"].as_array().is_some_and(|a| a.len() >= 2), "{m}");
    }
    let mut bad = p.clone();
    bad["panic"]["strategy"] = json!("loud");
    bad["macros"]["std::vec!"]["arms"][0]["pattern"] = json!("array![]");
    bad["macros"]["std::vec!"]["arms"][1]["params"][0]["type"] = json!("list<");
    let e = bare_rust::host::validate(&bad, &file, None).1.join("\n");
    assert!(e.contains("strategy") && e.contains("pattern starts with `vec!`") && e.contains("arm 1"), "{e}");
}

#[test]
fn what_a_function_computes_is_not_a_gap_of_the_language() {
    let (file, _) = std_file();
    let labels: Vec<&str> = file["subjects"].as_array().unwrap().iter().flat_map(|s| s["clauses"].as_array().into_iter().flatten()).filter(|c| c["kind"] == "unexpressed").map(|c| bare_rust::libstats::reason_label(c["reason"].as_str().unwrap())).collect();
    for gone in ["no formatted text", "no quantifier over fields", "no macro arguments", "no unwinding behaviour", "no budget or layout kind", "no sign of zero, no NaN payload"] {
        assert!(!labels.contains(&gone), "{gone}");
    }
    assert_eq!(labels.iter().filter(|l| **l == "what a function computes").count(), 8);
    assert_eq!(labels.len(), 65, "B3 took 121 to 114, B4 to 77, B5 to 65");
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
    assert_eq!(r.resolved.len(), 6, "{:?}", r.resolved);
    assert_eq!(r.checked.len() + r.resolved.len() + r.unchecked_total(), file["subjects"].as_array().unwrap().len());
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
    for want in ["library std 1.97.0 (260 subjects)", "function (27 are trait methods)", "unexpressed", "no callback as a function", "sequences of language 6.15 (13 clauses)", "path-resolved only: std::cmp::PartialEq", "type slots (188); 181 map to the type language and 7 are host text"] {
        assert!(t.contains(want), "{want}: {t}");
    }
    assert!(!t.contains("reasons carry no label"), "{t}");
}

// ---------- B1: implementations, implements, existential types, lifetimes and generics ----------

const CHAR: &str = "text where length(value) == 1 as 'char'";

/// `Chars<'a>` as the plan writes it: it holds a borrow for `'a` and implements `Iterator { Item = char }` and `Clone`; the
/// impl of `Iterator` is a subject of its own, with its method `next`.
fn chars_with_its_impl() -> Value {
    let mut f = exemplars_only();
    f.as_object_mut().unwrap().remove("views");
    let chars = subject_mut(&mut f, "std::str::Chars");
    chars["hold"] = json!({"kind": "borrows", "for": ["'a"]});
    chars["implements"] = json!([
        {"trait": "std::iter::Iterator", "assoc": {"Item": CHAR}, "impl": "std::str::Chars as std::iter::Iterator"},
        {"trait": "std::clone::Clone"}
    ]);
    chars["clauses"].as_array_mut().unwrap().push(json!({
        "kind": "implements", "text": "implements 'std::iter::Iterator' { Item = char }", "source": "1.97.0 core/src/str/iter.rs:35, impls: Iterator 35",
        "status": {"assumed": "documentation"}
    }));
    chars["clauses"].as_array_mut().unwrap().push(json!({
        "kind": "hold", "text": "hold : borrows for 'a", "source": "1.97.0 core/src/str/iter.rs:30, body: struct Chars<'a>", "status": {"observed": "from the declaration"}
    }));
    let subjects = f["subjects"].as_array_mut().unwrap();
    subjects.push(json!({
        "kind": "impl", "name": "std::str::Chars as std::iter::Iterator", "generics": [{"name": "'a", "bounds": []}],
        "self": "std::str::Chars<'a>", "trait": "std::iter::Iterator", "assoc": {"Item": CHAR},
        "clauses": [{"kind": "effects", "text": "no io", "source": "1.97.0 core/src/str/iter.rs:35, body: next", "status": {"observed": "body"}}]
    }));
    subjects.push(json!({
        "kind": "function", "name": "std::str::Chars as std::iter::Iterator::next", "impl": "std::str::Chars as std::iter::Iterator",
        "params": [{"name": "self", "type": "ref<std::str::Chars<'a>>", "access": {"rights": "write", "guarantee": "exclusive"}}],
        "result": {"type": format!("{CHAR}?")}
    }));
    f
}

#[test]
fn an_impl_is_a_subject_and_a_type_declares_what_it_implements() {
    let f = chars_with_its_impl();
    assert_eq!(errors(&f), "");
    // the clause forms are accepted: `implements`, `hold : borrows for 'a`, the associated type's name and type
    let mut bad = f.clone();
    // the clauses are found by kind, since the library file's own Chars entry carries them too
    let cl = |f: &mut Value, kind: &str, t: &str| {
        let cs = subject_mut(f, "std::str::Chars")["clauses"].as_array_mut().unwrap();
        for c in cs.iter_mut().filter(|c| c["kind"] == kind) {
            c["text"] = json!(t);
        }
    };
    cl(&mut bad, "implements", "implements 'std::iter::Iterator' { Item = nope }");
    cl(&mut bad, "hold", "hold : borrows for 'zz");
    let e = errors(&bad);
    assert!(e.contains("unknown name `nope`") && e.contains("unknown lifetime `'zz`"), "{e}");
    // an impl names its trait and its self type, and is named by them; an implements entry names a real impl of its trait
    let mut bad = f.clone();
    let imp = subject_mut(&mut bad, "std::str::Chars as std::iter::Iterator");
    imp.as_object_mut().unwrap().remove("self");
    imp["name"] = json!("std::str::Chars as std::clone::Clone");
    subject_mut(&mut bad, "std::str::Chars")["implements"][0]["impl"] = json!("std::str::Chars as std::nowhere");
    subject_mut(&mut bad, "std::str::Chars")["hold"]["for"] = json!(["'b"]);
    let e = errors(&bad);
    for want in ["names its `trait` and the `self` type", "ending in its trait `std::iter::Iterator`", "is not a subject of the file", "does not declare"] {
        assert!(e.contains(want), "{want}: {e}");
    }
    // an associated type is parsed like any other type, and `assoc` belongs to an impl
    let mut bad = f.clone();
    subject_mut(&mut bad, "std::str::Chars")["implements"][0]["assoc"]["Item"] = json!("list<");
    subject_mut(&mut bad, "std::str::Chars")["assoc"] = json!({"Item": "u8"});
    let e = errors(&bad);
    assert!(e.contains("implements 0: associated type `Item`") && e.contains("`assoc` is for an impl"), "{e}");
    // an implements clause is a clause kind, and the schema takes the old files unchanged
    let (file, text) = std_file();
    assert_eq!(validate(&file, Some(&text)), Vec::<String>::new());
}

#[test]
fn an_impl_and_its_methods_compile_against_the_real_std_impl() {
    let f = chars_with_its_impl();
    let r = check_signatures(&f, &work("sig-impl"), &target()).unwrap();
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    for n in ["std::str::Chars as std::iter::Iterator", "std::str::Chars as std::iter::Iterator::next", "std::str::Chars"] {
        assert!(r.checked.contains(&n.to_string()), "{n}: checked {:?}, unchecked {:?}", r.checked, r.unchecked);
    }
    assert_eq!(r.by_kind["impl"], (1, 0));
    // the wrong associated type, an impl the real type does not have, a method that is not the real one
    let mut f = chars_with_its_impl();
    subject_mut(&mut f, "std::str::Chars as std::iter::Iterator")["assoc"]["Item"] = json!("u8");
    subject_mut(&mut f, "std::str::Chars")["implements"][0]["assoc"]["Item"] = json!("u8");
    subject_mut(&mut f, "std::str::Chars")["implements"][1]["trait"] = json!("std::marker::Copy");
    subject_mut(&mut f, "std::str::Chars as std::iter::Iterator::next")["result"]["type"] = json!("u8?");
    let r = check_signatures(&f, &work("sig-impl-wrong"), &target()).unwrap();
    let by: std::collections::BTreeMap<&str, &str> = r.mismatches.iter().map(|(n, m)| (n.as_str(), m.as_str())).collect();
    assert!(by["std::str::Chars as std::iter::Iterator"].starts_with("E0271"), "{by:?}");
    assert!(by["std::str::Chars"].starts_with("E0271") || by["std::str::Chars"].starts_with("E0277"), "{by:?}");
    assert!(by["std::str::Chars as std::iter::Iterator::next"].starts_with("E0308"), "{by:?}");
    // an impl that is not there at all
    let mut f = chars_with_its_impl();
    f["subjects"].as_array_mut().unwrap().push(json!({
        "kind": "impl", "name": "std::str::Chars as std::marker::Copy", "generics": [{"name": "'a", "bounds": []}],
        "self": "std::str::Chars<'a>", "trait": "std::marker::Copy"
    }));
    let r = check_signatures(&f, &work("sig-impl-absent"), &target()).unwrap();
    assert_eq!(r.mismatches.len(), 1, "{:?}", r.mismatches);
    assert!(r.mismatches[0].0 == "std::str::Chars as std::marker::Copy" && r.mismatches[0].1.starts_with("E0277"), "{:?}", r.mismatches);
}

#[test]
fn some_and_any_are_checked_as_the_real_function_takes_and_returns_them() {
    let mut f = exemplars_only();
    f.as_object_mut().unwrap().remove("views");
    let subjects = f["subjects"].as_array_mut().unwrap();
    // `some I` as a parameter is a generic with the bound; `any I as 'Box'` is `Box<dyn I>`
    subjects.push(json!({
        "kind": "function", "name": "std::fs::write",
        "params": [{"name": "path", "type": "some std::convert::AsRef<std::path::Path>"},
                   {"name": "contents", "type": "some std::convert::AsRef<list<u8> as 'std::primitive::slice'>"}],
        "result": {"type": "()", "fails": "std::io::Error"}
    }));
    subjects.push(json!({
        "kind": "function", "name": "std::io::Error::new",
        "params": [{"name": "kind", "type": "std::io::ErrorKind"},
                   {"name": "error", "type": "some std::convert::Into<any std::error::Error + Send + Sync as 'std::boxed::Box'>"}],
        "result": {"type": "std::io::Error"}
    }));
    // `some I` as the result: the real result type implements `I`, with the associated types
    subjects.push(json!({
        "kind": "function", "name": "std::iter::repeat", "generics": [{"name": "T", "bounds": ["std::clone::Clone"]}],
        "params": [{"name": "elt", "type": "T"}],
        "result": {"type": "some std::iter::Iterator { Item = T }"}
    }));
    // a borrow inside a bound is shared and unnamed, and the result borrows from the one reference it is given
    subjects.push(json!({
        "kind": "function", "name": "std::primitive::str::split_whitespace",
        "params": [{"name": "self", "type": "ref<text>", "access": {"rights": "read", "guarantee": "readers"}}],
        "result": {"type": "some std::iter::Iterator { Item = ref<text> }", "hold": {"kind": "borrows", "from": "self"}}
    }));
    assert_eq!(errors(&f), "");
    let r = check_signatures(&f, &work("sig-exist"), &target()).unwrap();
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    for n in ["std::fs::write", "std::io::Error::new", "std::iter::repeat", "std::primitive::str::split_whitespace"] {
        assert!(r.checked.contains(&n.to_string()), "{n}: checked {:?}, unchecked {:?}", r.checked, r.unchecked);
    }
    // a bound the real function does not meet is a mismatch
    subject_mut(&mut f, "std::iter::repeat")["result"]["type"] = json!("some std::iter::Iterator { Item = u8 }");
    subject_mut(&mut f, "std::fs::write")["params"][1]["type"] = json!("some std::fmt::Display");
    let r = check_signatures(&f, &work("sig-exist-wrong"), &target()).unwrap();
    let by: std::collections::BTreeMap<&str, &str> = r.mismatches.iter().map(|(n, m)| (n.as_str(), m.as_str())).collect();
    assert!(by["std::iter::repeat"].starts_with("E0271"), "{by:?}");
    assert!(by["std::fs::write"].starts_with("E0277"), "{by:?}");
    assert_eq!(r.mismatches.len(), 2, "{by:?}");
    // what is not at the top of a result cannot be checked this way, and is said so
    let mut f = exemplars_only();
    f["subjects"].as_array_mut().unwrap().push(json!({
        "kind": "function", "name": "std::iter::repeat", "generics": [{"name": "T", "bounds": []}], "params": [{"name": "elt", "type": "T"}],
        "result": {"type": "some std::iter::Iterator { Item = T }", "fails": "std::io::Error"}
    }));
    let r = check_signatures(&f, &work("sig-exist-unchecked"), &target()).unwrap();
    assert!(r.unchecked.contains_key("a `some` result that fails"), "{:?}", r.unchecked);
}

#[test]
fn generics_and_lifetimes_on_host_subjects_are_checked() {
    // `for 'a`, `for T where T: Bound` on a type, a function and an impl, against the real items
    let mut f = exemplars_only();
    f.as_object_mut().unwrap().remove("views");
    let subjects = f["subjects"].as_array_mut().unwrap();
    subjects.push(json!({
        "kind": "type", "name": "std::slice::Iter", "generics": [{"name": "'a", "bounds": []}, {"name": "T", "bounds": ["'a"]}],
        "hold": {"kind": "borrows", "for": ["'a"]},
        "implements": [{"trait": "std::iter::Iterator", "assoc": {"Item": "ref<T>"}}]
    }));
    subjects.push(json!({
        "kind": "impl", "name": "std::vec::Vec as std::clone::Clone", "generics": [{"name": "T", "bounds": ["std::clone::Clone"]}],
        "self": "list<T> as 'std::vec::Vec'", "trait": "std::clone::Clone"
    }));
    subjects.push(json!({
        "kind": "impl", "name": "std::vec::Vec as std::iter::IntoIterator", "generics": [{"name": "T", "bounds": []}],
        "self": "list<T> as 'std::vec::Vec'", "trait": "std::iter::IntoIterator", "assoc": {"Item": "T"}
    }));
    assert_eq!(errors(&f), "");
    let r = check_signatures(&f, &work("sig-generics"), &target()).unwrap();
    // `Item = &'a T` needs a lifetime a bound can name, and the type has `'a`
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    for n in ["std::slice::Iter", "std::vec::Vec as std::clone::Clone", "std::vec::Vec as std::iter::IntoIterator"] {
        assert!(r.checked.contains(&n.to_string()), "{n}: {:?} {:?}", r.checked, r.unchecked);
    }
    // the bound of the impl is the real one's: without `T: Clone` the impl does not exist
    subject_mut(&mut f, "std::vec::Vec as std::clone::Clone")["generics"][0]["bounds"] = json!([]);
    let r = check_signatures(&f, &work("sig-generics-wrong"), &target()).unwrap();
    assert!(r.mismatches.iter().any(|(n, m)| n == "std::vec::Vec as std::clone::Clone" && m.starts_with("E0277")), "{:?}", r.mismatches);
}

// ---------- B2: `given`, promises that hold unless an implementation does otherwise ----------

/// `HashMap::get` as the plan writes it: it never panics given what `Q`'s `Hash` and `Eq` do, and counts only given the
/// build. The file has the bound's methods as subjects, so a method name can be checked.
fn get_with_given() -> Value {
    let mut f = exemplars_only();
    f.as_object_mut().unwrap().remove("views");
    let subjects = f["subjects"].as_array_mut().unwrap();
    subjects.push(json!({"kind": "trait", "name": "std::hash::Hash"}));
    subjects.push(json!({"kind": "function", "name": "std::hash::Hash::hash", "params": [], "trait": "std::hash::Hash"}));
    subjects.push(json!({"kind": "trait", "name": "std::cmp::Eq"}));
    subjects.push(json!({"kind": "function", "name": "std::cmp::Eq::eq", "params": [], "trait": "std::cmp::Eq"}));
    let get = subject_mut(&mut f, "std::collections::HashMap::get");
    let cl = get["clauses"].as_array_mut().unwrap();
    cl.clear();
    cl.push(json!({"kind": "never panics", "text": "never panics", "given": ["Q.hash, Q.eq never panic"], "source": "s", "status": {"assumed": "documentation"}}));
    cl.push(json!({"kind": "effects", "text": "effects none", "given": ["Q.hash, Q.eq effects none", "build panic=unwind"], "source": "s"}));
    cl.push(json!({"kind": "ensure", "text": "ensure same : result == self[k]", "given": ["build overflow_checks"], "source": "s"}));
    f
}

#[test]
fn a_given_names_generics_methods_defaults_and_build_settings() {
    let f = get_with_given();
    assert_eq!(errors(&f), "");
    fn get(f: &mut Value) -> &mut Value {
        subject_mut(f, "std::collections::HashMap::get")
    }
    // each way a `given` can be wrong is reported against the clause
    for (given, want) in [
        ("Z.hash never panic", "`Z` is not a generic type parameter"),
        ("Q.nope never panic", "`nope` is not a method of a bound of `Q`"),
        ("build opt_level", "not a build setting of the host profile"),
        ("default", "not a method of a trait"),
        ("Q.hash", "a `given` item is"),
    ] {
        let mut bad = f.clone();
        get(&mut bad)["clauses"][1]["given"] = json!([given]);
        let e = errors(&bad);
        assert!(e.contains(want) && e.contains("clause 1"), "{given}: {e}");
    }
    // `given` is the slot's, not the text's; it qualifies only the promises that depend on an implementation; an empty list is the schema's
    let mut bad = f.clone();
    get(&mut bad)["clauses"][0]["text"] = json!("never panics given Q.hash never panic");
    let e = errors(&bad);
    assert!(e.contains("the clause's `given` slot"), "{e}");
    let mut bad = f.clone();
    get(&mut bad)["clauses"][1]["given"] = json!([]);
    assert!(errors(&bad).contains("schema:"), "{}", errors(&bad));
    let mut bad = f.clone();
    get(&mut bad)["clauses"].as_array_mut().unwrap().push(json!({"kind": "unexpressed", "text": "t", "reason": "r", "given": ["default"], "source": "s"}));
    assert!(errors(&bad).contains("schema:"));
    let mut bad = f.clone();
    get(&mut bad)["clauses"].as_array_mut().unwrap().push(json!({"kind": "invariant", "text": "invariant i : true", "given": ["default"], "source": "s"}));
    assert!(errors(&bad).contains("`invariant` clause is not qualified by `given`"), "{}", errors(&bad));
}

#[test]
fn the_default_body_is_named_by_a_trait_method_and_a_checkable_method_is_known_to_its_bound() {
    let mut f = get_with_given();
    f["subjects"].as_array_mut().unwrap().push(json!({
        "kind": "function", "name": "std::cmp::Ord::max", "params": [], "trait": "std::cmp::Ord",
        "clauses": [{"kind": "never panics", "text": "never panics", "given": ["default", "Self.cmp never panic"], "source": "s"}]
    }));
    assert_eq!(errors(&f), "");
    // `K` has a bound the file has no methods of (`Borrow`): its methods cannot be checked, so any name passes; `Q`'s are checked
    let get = subject_mut(&mut f, "std::collections::HashMap::get");
    get["clauses"][1]["given"] = json!(["K.anything never panic"]);
    assert_eq!(errors(&f), "");
    // a file with `given` is canonical when libfmt prints it, with `given` after `text`; one without it is valid unchanged
    let f = get_with_given();
    let c = canonical(&f);
    assert!(c.contains("\"text\": \"never panics\",\n") && c.find("\"given\"").unwrap() > c.find("\"text\": \"never panics\"").unwrap());
    assert_eq!(canonical(&serde_json::from_str::<Value>(&c).unwrap()), c);
    assert_eq!(validate(&f, Some(&c)), Vec::<String>::new());
}

#[test]
fn libstats_counts_conditional_clauses_apart_and_the_profile_lists_the_build_settings() {
    let f = get_with_given();
    let t = bare_rust::libstats::libstats(&f, None);
    // the three of `get` above, and the two `given T.max` clauses of the exemplar `cmp::max`
    assert!(t.contains("conditional clauses, qualified by `given` (5 on 2 subjects)"), "{t}");
    assert!(t.contains("on a build setting") && t.contains("on a bound's implementations"), "{t}");
    let p: Value = serde_json::from_str(&fs::read_to_string(libraries().join("host-rust-1.97.0.json")).unwrap()).unwrap();
    let names: Vec<&str> = p["build"].as_array().unwrap().iter().map(|b| b["setting"].as_str().unwrap()).collect();
    assert_eq!(names, ["overflow_checks", "debug_assertions", "panic=unwind", "panic=abort"]);
    assert_eq!(bare_rust::host::builds(Some(&p)).len(), 4);
}

// ---------- B3: callbacks, their arguments, results and conditions, and protocols over calls ----------

/// `Option::map_or_else` and an `Iterator::count` as the plan writes them (language 6.13).
fn with_callbacks() -> Value {
    let mut f = exemplars_only();
    f.as_object_mut().unwrap().remove("views");
    let subjects = f["subjects"].as_array_mut().unwrap();
    subjects.push(json!({"kind": "trait", "name": "std::iter::Iterator"}));
    subjects.push(json!({"kind": "function", "name": "std::iter::Iterator::next", "params": [], "trait": "std::iter::Iterator"}));
    subjects.push(json!({
        "kind": "function", "name": "std::option::Option::map_or_else",
        "generics": [{"name": "T", "bounds": []}, {"name": "U", "bounds": []}, {"name": "D", "bounds": ["FnOnce() -> U"]}, {"name": "F", "bounds": ["FnOnce(T) -> U"]}],
        "params": [{"name": "self", "type": "T?"}, {"name": "default", "type": "D"}, {"name": "f", "type": "F"}], "result": {"type": "U"},
        "clauses": [
            {"kind": "callback", "text": "callback default : called exactly once when self is none, returning d", "source": "s"},
            {"kind": "callback", "text": "callback f : called exactly once when exists self, with self, returning r", "source": "s"},
            {"kind": "ensure", "text": "ensure from_callback : result == (if exists self then r else d)", "source": "s"}
        ]
    }));
    subjects.push(json!({
        "kind": "function", "name": "std::iter::Iterator::count", "generics": [{"name": "Self", "bounds": ["std::iter::Iterator"]}], "trait": "std::iter::Iterator",
        "params": [{"name": "self", "type": "Self"}], "result": {"type": "u64 as 'usize'"},
        "clauses": [{"kind": "protocol", "text": "protocol self.next+", "source": "s"}]
    }));
    f
}

#[test]
fn a_callback_names_its_arguments_and_result_for_the_clauses_of_its_subject() {
    let f = with_callbacks();
    assert_eq!(errors(&f), "");
    let clauses = |f: &mut Value| subject_mut(f, "std::option::Option::map_or_else")["clauses"].as_array_mut().unwrap().clone();
    // the names `returning` binds are known to the ensure that follows, and to nothing else
    let mut bad = f.clone();
    subject_mut(&mut bad, "std::option::Option::map_or_else")["clauses"][2]["text"] = json!("ensure from_callback : result == (if exists self then r else e)");
    assert!(errors(&bad).contains("unknown name `e`"), "{}", errors(&bad));
    let mut bad = f.clone();
    subject_mut(&mut bad, "std::option::Option::map_or_else")["clauses"].as_array_mut().unwrap().remove(1);
    assert!(errors(&bad).contains("unknown name `r`"), "{}", errors(&bad));
    // a `when` uses only the names the subject has, not what a callback returns
    for (text, want) in [
        ("callback default : called exactly once when nope is none, returning d", "unknown name `nope`"),
        ("callback default : called exactly once when d is none, returning d", "unknown name `d`"),
        ("callback default : called exactly once when self is none, with q, returning d", ""),
        ("callback default : called exactly once when self is none, with nope(self), returning d", "unknown name `nope`"),
        ("callback default : called at least once when self is none, returning d", "name the values of one call"),
        ("callback default : called exactly once, returning d, with self", "`returning` names one result"),
        ("callback default : called exactly once, returning", "`returning` names one result"),
        ("callback default : called exactly once, returning d e", "`returning` names one result"),
        ("callback default : called exactly once, with", "`with` names the call's arguments"),
    ] {
        let mut bad = f.clone();
        subject_mut(&mut bad, "std::option::Option::map_or_else")["clauses"][0]["text"] = json!(text);
        let e = errors(&bad);
        if want.is_empty() {
            assert_eq!(e, "", "{text}");
        } else {
            assert!(e.contains(want), "{text}: {e}");
        }
    }
    // old callback clauses stay valid
    let mut old = f.clone();
    subject_mut(&mut old, "std::option::Option::map_or_else")["clauses"][0]["text"] = json!("callback default : called at most once before return");
    subject_mut(&mut old, "std::option::Option::map_or_else")["clauses"][2]["text"] = json!("ensure x : result == result");
    assert_eq!(errors(&old), "");
    assert_eq!(clauses(&mut old).len(), 3);
}

#[test]
fn a_protocol_names_callbacks_and_parameters() {
    let f = with_callbacks();
    for (text, want) in [
        ("protocol (self.next)* self.next", ""),
        ("protocol begin self.next+ end", ""),
        ("protocol (self.nope)*", "`nope` is not a method of `self`'s bounds"),
        ("protocol (other.next)*", "`other` is not a parameter of the subject"),
        ("protocol (nope)*", "`nope` is not a callback or a parameter"),
        ("protocol (self.next", "unclosed"),
        ("protocol", "names at least one call"),
        ("protocol self.next ; self.next", "calls joined by"),
    ] {
        let mut bad = f.clone();
        subject_mut(&mut bad, "std::iter::Iterator::count")["clauses"][0]["text"] = json!(text);
        let e = errors(&bad);
        if want.is_empty() {
            assert_eq!(e, "", "{text}");
        } else {
            assert!(e.contains(want), "{text}: {e}");
        }
    }
    // a callback of the subject is an event too
    let mut g = f.clone();
    let m = subject_mut(&mut g, "std::option::Option::map_or_else");
    m["clauses"].as_array_mut().unwrap().push(json!({"kind": "protocol", "text": "protocol (default | f)", "source": "s"}));
    assert_eq!(errors(&g), "");
}

#[test]
fn libstats_counts_callback_clauses_by_what_they_name_and_the_file_is_canonical() {
    let f = with_callbacks();
    let t = bare_rust::libstats::libstats(&f, None);
    assert!(t.contains("callback clauses (2 on 1 subjects), and protocols over calls (1)"), "{t}");
    let c = canonical(&f);
    assert_eq!(canonical(&serde_json::from_str::<Value>(&c).unwrap()), c);
    assert_eq!(validate(&f, Some(&c)), Vec::<String>::new());
}

#[test]
fn b3_closed_the_callback_promises_and_relabelled_what_stays() {
    let (file, _) = std_file();
    let clauses: Vec<(&str, &Value)> = file["subjects"].as_array().unwrap().iter().flat_map(|s| s["clauses"].as_array().into_iter().flatten().map(move |c| (s["name"].as_str().unwrap(), c))).collect();
    let labels: Vec<&str> = clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed").map(|(_, c)| bare_rust::libstats::reason_label(c["reason"].as_str().unwrap())).collect();
    assert_eq!(labels.len(), 65, "121 before B3: 7 closed; B4 closed 37 more, B5 12 more");
    for gone in ["no value of a callback result", "no order of calls", "callback clause has no condition"] {
        assert!(!labels.contains(&gone), "{gone}");
    }
    let n = |l: &str| labels.iter().filter(|x| **x == l).count();
    assert_eq!((n("no sequence value"), n("no promise about a result's later calls"), n("no promise")), (0, 4, 6));
    let with = |w: &str| clauses.iter().filter(|(_, c)| c["kind"] == "callback" && c["text"].as_str().unwrap().contains(w)).count();
    assert_eq!((with(" when "), with(", with "), with(", returning ")), (3, 2, 2));
    let mut protocols: Vec<&str> = clauses.iter().filter(|(_, c)| c["kind"] == "protocol").map(|(s, _)| *s).collect();
    protocols.sort();
    assert_eq!(protocols, ["std::assert_eq!", "std::iter::Iterator::count"]);
}

// ---------- B4: eight small additions (language 6.14) ----------

/// The subjects the small additions are written on: `slice::len`, `mem::replace`, `slice::split_at_mut`, and the exemplar `Chars`.
fn small_additions() -> Value {
    let mut f = exemplars_only();
    f.as_object_mut().unwrap().remove("views");
    let slice = "ref<list<T> as 'std::primitive::slice'>";
    let subjects = f["subjects"].as_array_mut().unwrap();
    subjects.push(json!({
        "kind": "function", "name": "std::primitive::slice::len", "generics": [{"name": "T", "bounds": []}],
        "params": [{"name": "self", "type": slice, "access": {"rights": "read", "guarantee": "readers"}}], "result": {"type": "u64 as 'usize'"},
        "clauses": [{"kind": "effects", "text": "evaluable at compile time", "source": "s", "status": {"lifted": "s"}}]
    }));
    subjects.push(json!({
        "kind": "function", "name": "std::mem::replace", "generics": [{"name": "T", "bounds": []}],
        "params": [{"name": "dest", "type": "ref<T>", "access": {"rights": "write", "guarantee": "exclusive"}}, {"name": "src", "type": "T"}], "result": {"type": "T"},
        "clauses": [
            {"kind": "hold", "text": "lifetime dest, src survive call", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "hold", "text": "hold dest : moved from src", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "hold", "text": "hold result : moved from dest", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "ensure", "text": "ensure moved : dest == src implies result moved from src", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "never panics", "text": "never panics except exhausted", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "budget", "text": "budget work expected amortised O(1)", "source": "s", "status": {"assumed": "documentation"}}
        ]
    }));
    subjects.push(json!({
        "kind": "function", "name": "std::primitive::slice::split_at_mut", "generics": [{"name": "T", "bounds": []}],
        "params": [{"name": "self", "type": slice, "access": {"rights": "write", "guarantee": "exclusive"}}, {"name": "mid", "type": "u64 as 'usize'"}],
        "result": {"type": format!("({slice}, {slice})"), "access": {"rights": "write", "guarantee": "exclusive"}, "hold": {"kind": "borrows", "from": "self"}},
        "clauses": [
            {"kind": "ensure", "text": "ensure part_sizes : size(result.0) == mid and size(result.1) == old(size(self)) - mid", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "ensure", "text": "ensure pair : result is (head, tail) implies size(head) + size(tail) == old(size(self))", "source": "s", "status": {"assumed": "documentation"}},
            {"kind": "access", "text": "disjoint result.0, result.1", "source": "s", "status": {"assumed": "documentation"}}
        ]
    }));
    subject_mut(&mut f, "std::str::Chars")["clauses"] = json!([{"kind": "result must be used", "text": "must be used", "source": "s", "status": {"lifted": "s"}}]);
    f
}

/// The errors after the clause `n` of a subject becomes `kind` and `text` (or is added, past the end).
fn with_clause(f: &Value, subject: &str, n: usize, kind: &str, text: &str) -> String {
    let mut g = f.clone();
    let cl = subject_mut(&mut g, subject)["clauses"].as_array_mut().unwrap();
    let c = json!({"kind": kind, "text": text, "source": "s", "status": {"assumed": "documentation"}});
    if n < cl.len() {
        cl[n] = c;
    } else {
        cl.push(c);
    }
    errors(&g)
}

#[test]
fn the_small_additions_validate_and_old_clauses_stay_valid() {
    let f = small_additions();
    assert_eq!(errors(&f), "");
    let c = canonical(&f);
    assert_eq!(validate(&f, Some(&c)), Vec::<String>::new());
    // the older forms of the same kinds are still the same clauses
    let r = "std::mem::replace";
    for (n, kind, text) in [(4, "never panics", "never panics"), (5, "budget", "budget work O(size(dest))"), (1, "hold", "hold dest : owns")] {
        assert_eq!(with_clause(&f, r, n, kind, text), "", "{text}");
    }
}

#[test]
fn evaluable_at_compile_time_is_a_bound_on_a_function() {
    let f = small_additions();
    assert_eq!(with_clause(&f, "std::mem::replace", 9, "effects", "evaluable at compile time"), "");
    let e = with_clause(&f, "std::str::Chars", 9, "effects", "evaluable at compile time");
    assert!(e.contains("is for a function"), "{e}");
    let e = with_clause(&f, "std::mem::replace", 9, "effects", "evaluable at compile time now");
    assert!(e.contains("unknown name `now`"), "{e}");
    let e = with_clause(&f, "std::mem::replace", 9, "effects", "evaluable at run time");
    assert!(e.contains("starts with"), "{e}");
}

#[test]
fn must_be_used_is_for_a_type_and_result_must_be_used_for_a_function() {
    let f = small_additions();
    let e = with_clause(&f, "std::str::Chars", 0, "result must be used", "result must be used");
    assert!(e.contains("`must be used` is for a type or a trait"), "{e}");
    let e = with_clause(&f, "std::mem::replace", 9, "result must be used", "must be used");
    assert!(e.contains("`must be used` is for a type or a trait"), "{e}");
    assert_eq!(with_clause(&f, "std::mem::replace", 9, "result must be used", "result must be used"), "");
    // a trait can be must-use too (Iterator)
    let mut g = f.clone();
    g["subjects"].as_array_mut().unwrap().push(json!({"kind": "trait", "name": "std::iter::Iterator", "clauses": [{"kind": "result must be used", "text": "must be used", "source": "s", "status": {"lifted": "s"}}]}));
    assert_eq!(errors(&g), "");
}

#[test]
fn a_move_names_one_path_and_an_end_names_the_call() {
    let f = small_additions();
    let r = "std::mem::replace";
    for (kind, text, want) in [
        ("hold", "hold result : moved from dest", ""),
        ("hold", "hold dest : moved from src", ""),
        ("hold", "hold nope : moved from src", "`nope` is not a parameter of the subject"),
        ("hold", "hold result : moved from nope", "unknown name `nope`"),
        ("hold", "hold result : moved from", "`moved from` names one path"),
        ("hold", "hold result : moved from dest src", "`moved from` names one path"),
        ("hold", "hold result : moved from dest.0", ""),
        ("hold", "lifetime src ends in call", ""),
        ("hold", "lifetime src ends in call when src == dest", ""),
        ("hold", "lifetime src ends in call when", "`lifetime v ends in call`"),
        ("hold", "lifetime src, dest ends in call", "`lifetime v ends in call`"),
        ("hold", "lifetime src survive call", ""),
        ("hold", "lifetime dest, src survive call", ""),
        ("hold", "lifetime dest, survive call", "`lifetime a, b survive call`"),
        ("hold", "lifetime dest src survive call", "`lifetime a, b survive call`"),
        ("hold", "lifetime dest survive call now", "`lifetime a, b survive call`"),
        ("hold", "lifetime nope survive call", "unknown name `nope`"),
        ("ensure", "ensure m : result moved from src", ""),
        ("ensure", "ensure m : result moved from nope", "unknown name `nope`"),
    ] {
        let e = with_clause(&f, r, 1, kind, text);
        if want.is_empty() {
            assert_eq!(e, "", "{text}");
        } else {
            assert!(e.contains(want), "{text}: {e}");
        }
    }
}

#[test]
fn exhaustion_cost_and_obligations_have_forms_of_their_own() {
    let f = small_additions();
    let r = "std::mem::replace";
    for (n, kind, text, want) in [
        (4, "never panics", "never panics except exhausted", ""),
        (4, "never panics", "never panics except broke", "unknown name `broke`"),
        (4, "never panics", "never panics except", "unknown name `except`"),
        (5, "budget", "budget work expected O(1)", ""),
        (5, "budget", "budget work amortised O(size(dest))", ""),
        (5, "budget", "budget work sometimes O(1)", "unknown name `sometimes`"),
        (9, "obliges", "obliges caller : src == dest because \"the two must be told apart\"", ""),
        (9, "obliges", "obliges caller : src == dest", "states a condition and the reason"),
        (9, "obliges", "obliges caller : src == dest because src", "states a condition and the reason"),
        (9, "obliges", "obliges caller : because \"x\"", "states a condition and the reason"),
        (9, "obliges", "obliges caller : nope because \"x\"", "unknown name `nope`"),
        (9, "obliges", "assume caller : src because \"x\"", "starts with `obliges caller`"),
    ] {
        let e = with_clause(&f, r, n, kind, text);
        if want.is_empty() {
            assert_eq!(e, "", "{text}");
        } else {
            assert!(e.contains(want), "{text}: {e}");
        }
    }
    let e = with_clause(&f, "std::str::Chars", 9, "obliges", "obliges caller : true because \"x\"");
    assert!(e.contains("`obliges caller` is for a function"), "{e}");
}

#[test]
fn tuple_parts_and_element_frames_name_what_the_subject_has() {
    let f = small_additions();
    let s = "std::primitive::slice::split_at_mut";
    for (n, kind, text, want) in [
        (0, "ensure", "ensure p : size(result.0) == mid and size(result.1) == mid", ""),
        (0, "ensure", "ensure p : result.1 == result.0", ""),
        (1, "ensure", "ensure p : result is (a, _) implies size(a) == mid", ""),
        (1, "ensure", "ensure p : result is (a, b) implies size(c) == 1", "unknown name `c`"),
        (1, "ensure", "ensure p : size(a) == 1", "unknown name `a`"),
        (2, "access", "disjoint result.0, result.1", ""),
        (2, "access", "disjoint result.0, other", "unknown name `other`"),
        (2, "access", "mutates only self[mid]", ""),
        (2, "access", "mutates only self[mid], result.0", ""),
        (2, "access", "mutates only self[kee]", "unknown name `kee`"),
    ] {
        let e = with_clause(&f, s, n, kind, text);
        if want.is_empty() {
            assert_eq!(e, "", "{text}");
        } else {
            assert!(e.contains(want), "{text}: {e}");
        }
    }
}

#[test]
fn a_const_claim_is_checked_by_calling_the_item_in_a_const_context() {
    let mut f = small_additions();
    let r = check_signatures(&f, &work("sig-const"), &target()).unwrap();
    assert_eq!(r.mismatches, vec![], "{:?}", r.mismatches);
    assert!(r.checked.contains(&"std::primitive::slice::len".to_string()));
    // `cmp::max` is a `const fn` whose const-ness is unstable: calling it where a constant is needed does not compile
    subject_mut(&mut f, "std::cmp::max")["clauses"] = json!([{"kind": "effects", "text": "evaluable at compile time", "source": "s", "status": {"lifted": "s"}}]);
    // so is a function that is not const at all
    subject_mut(&mut f, "std::mem::replace")["clauses"].as_array_mut().unwrap().push(json!({"kind": "effects", "text": "evaluable at compile time", "source": "s", "status": {"lifted": "s"}}));
    subject_mut(&mut f, "std::primitive::str::chars")["clauses"] = json!([{"kind": "effects", "text": "evaluable at compile time", "source": "s", "status": {"lifted": "s"}}]);
    let r = check_signatures(&f, &work("sig-const-bad"), &target()).unwrap();
    let by: std::collections::BTreeMap<&str, &str> = r.mismatches.iter().map(|(n, m)| (n.as_str(), m.as_str())).collect();
    assert!(by["std::cmp::max"].starts_with("E0658"), "unstable const-ness: {by:?}");
    assert!(by["std::primitive::str::chars"].starts_with("E0015"), "{by:?}");
    assert!(!by.contains_key("std::mem::replace") && !by.contains_key("std::primitive::slice::len"), "a const fn passes: {by:?}");
    assert_eq!(r.mismatches.len(), 2, "{by:?}");
}

#[test]
fn libstats_counts_the_small_additions() {
    // the exemplars carry `cmp::max`'s tie and `HashMap::get`'s expected cost
    let f = small_additions();
    let t = bare_rust::libstats::libstats(&f, None);
    assert!(t.contains("small additions of language 6.14 (13 clauses)"), "{t}");
    let line = |l: &str| t.lines().find(|x| x.contains(l)).unwrap_or("").split_whitespace().rev().nth(1).unwrap_or("").to_string();
    assert_eq!(
        [line("(6.14.1)"), line("(6.14.2)"), line("(6.14.3)"), line("(6.14.4)"), line("(6.14.5)"), line("(6.14.6)"), line("(6.14.7)"), line("(6.14.8)"), line("(6.14.9)")],
        ["1", "1", "4", "1", "1", "2", "3", "0", "0"].map(String::from),
        "{t}"
    );
}

#[test]
fn b4_closed_the_small_additions_and_relabelled_what_stays() {
    let (file, _) = std_file();
    let clauses: Vec<(&str, &Value)> = file["subjects"].as_array().unwrap().iter().flat_map(|s| s["clauses"].as_array().into_iter().flatten().map(move |c| (s["name"].as_str().unwrap(), c))).collect();
    let labels: Vec<&str> = clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed").map(|(_, c)| bare_rust::libstats::reason_label(c["reason"].as_str().unwrap())).collect();
    assert_eq!(labels.len(), 65, "114 before B4: 37 closed, then B5: 12 closed");
    for gone in ["no const-ness", "must-use on a type", "no drop", "no identity of moved values", "no resource exhaustion", "no expected or amortised cost", "no tuple parts", "no frame condition on a collection", "no sufficient condition"] {
        assert!(!labels.contains(&gone), "{gone}");
    }
    let n = |l: &str| labels.iter().filter(|x| **x == l).count();
    assert_eq!(
        (n("no stable const-ness"), n("no identity of cloned values"), n("no identity of a stored key"), n("no effects of a destructor"), n("no arm of a macro in a clause"), n("no condition on an associated type's implementation"), n("no host state")),
        (4, 2, 1, 1, 1, 3, 7)
    );
    assert_eq!(n("no sequence value"), 0, "B5 gave every one a sequence or a label of its own");
    let with = |k: &str, t: &str| clauses.iter().filter(|(_, c)| c["kind"] == k && c["text"].as_str().unwrap().starts_with(t)).count();
    assert_eq!((with("effects", "evaluable at compile time"), with("result must be used", "must be used"), with("never panics", "never panics except exhausted"), with("budget", "budget work expected")), (14, 14, 2, 2));
    // every const claim of the file is an item the host promises to keep const
    for (s, c) in clauses.iter().filter(|(_, c)| c["text"] == "evaluable at compile time") {
        assert!(c["source"].as_str().unwrap().contains("rustc_const_stable"), "{s}");
    }
    // each type that carries `#[must_use]` says so, and the iterator adapters cite their attribute
    for (s, c) in clauses.iter().filter(|(_, c)| c["kind"] == "result must be used" && c["text"] == "must be used") {
        assert!(c["source"].as_str().unwrap().contains("attribute #[must_use"), "{s}");
    }
}

// ---------- B5: sequences (language 6.15) ----------

/// The exemplars of the small additions, with a view for `map` to name.
fn sequences() -> Value {
    let mut f = small_additions();
    subject_mut(&mut f, "std::str::Chars")["views"] = json!([{"name": "widen", "host": "'std::str::Chars::as_str'"}]);
    f
}

#[test]
fn sequences_take_the_operators_of_language_6_15_and_old_clauses_stay_valid() {
    let f = sequences();
    assert_eq!(errors(&f), "");
    let s = "std::primitive::slice::split_at_mut";
    for (kind, text, want) in [
        ("ensure", "ensure e : result.0 == old(self)[..mid] and result.1 == old(self)[mid..]", ""),
        ("ensure", "ensure e : old(self)[mid] == none or old(self)[0 .. mid] == old(self)[..mid]", ""),
        ("ensure", "ensure e : yields(result.0) ++ yields(result.1) == old(yields(self))", ""),
        ("ensure", "ensure e : reverse(reverse(old(self))) == old(self) and take(mid, skip(1, old(self))) == old(self)[1..mid]", ""),
        ("ensure", "ensure e : zip(old(self), enumerate(old(self))) == [] and map(deref, old(self)) == filter(widen, old(self))", ""),
        ("ensure", "ensure e : map(scalars, old(self)) == old(self)", ""),
        ("ensure", "ensure e : map(f, old(self)) == old(self)", "unknown name `f`"),
        ("ensure", "ensure e : map(mid, old(self)) == old(self)", "is a parameter or callback, not a named pure function"),
        ("ensure", "ensure e : filter('std::primitive::slice::is_empty', old(self)) == old(self)", "takes the name of a pure function first"),
        ("ensure", "ensure e : map(deref) == old(self)", "`map` takes 2 arguments"),
        ("ensure", "ensure e : yields() == old(self)", "`yields` takes 1 argument"),
        ("ensure", "ensure e : take(1, 2, 3) == old(self)", "`take` takes 2 arguments"),
        ("ensure", "ensure e : old(self)[mid == 1", "unclosed `[`"),
        ("ensure", "ensure e : old(self)[zz..] == []", "unknown name `zz`"),
        ("ensure", "ensure e : old(self)[1..2] == yields(zz)", "unknown name `zz`"),
        // the older forms of the same clauses
        ("ensure", "ensure e : size(result.0) == mid and (all x in result.1 : x == x)", ""),
    ] {
        let e = with_clause(&f, s, 0, kind, text);
        if want.is_empty() {
            assert_eq!(e, "", "{text}");
        } else {
            assert!(e.contains(want), "{text}: {e}");
        }
    }
    let c = canonical(&f);
    assert_eq!(validate(&f, Some(&c)), Vec::<String>::new());
}

#[test]
fn libstats_counts_the_sequences() {
    let mut f = sequences();
    let line = |t: &str, l: &str| t.lines().find(|x| x.contains(l)).unwrap_or("").split_whitespace().rev().nth(1).unwrap_or("0").parse::<usize>().unwrap_or(0);
    let rows = ["`yields(e)`", "indexing `xs[i]`", "slices `xs[i..j]`", "`++`", "`reverse`, `take`, `skip`", "`zip`, `enumerate`", "`map`, `filter`", "`scalars`, `deref`"];
    let before_t = bare_rust::libstats::libstats(&f, None);
    let before: Vec<usize> = rows.iter().map(|r| line(&before_t, r)).collect();
    let s = "std::primitive::slice::split_at_mut";
    let cl = subject_mut(&mut f, s)["clauses"].as_array_mut().unwrap();
    for t in [
        "ensure a : yields(result.0) ++ yields(result.1) == old(yields(self))",
        "ensure b : result.0 == old(self)[..mid]",
        "ensure c : old(self)[0] == none",
        "ensure d : reverse(take(mid, skip(1, old(self)))) == []",
        "ensure e : zip(old(self), enumerate(old(self))) == []",
        "ensure f : map(deref, old(self)) == old(self)",
    ] {
        cl.push(json!({"kind": "ensure", "text": t, "source": "s", "status": {"assumed": "documentation"}}));
    }
    let t = bare_rust::libstats::libstats(&f, None);
    let total = |t: &str| t.lines().find(|x| x.starts_with("sequences of language 6.15")).unwrap().to_string();
    assert_eq!(total(&t), format!("sequences of language 6.15 ({} clauses)", total(&before_t).split('(').nth(1).unwrap().split(' ').next().unwrap().parse::<usize>().unwrap() + 6), "{t}");
    let after: Vec<usize> = rows.iter().map(|r| line(&t, r)).collect();
    // a: yields and `++`; b: a slice; c: an index; d: reverse, take, skip; e: zip, enumerate; f: map and deref
    let delta: Vec<usize> = after.iter().zip(&before).map(|(a, b)| a - b).collect();
    assert_eq!(delta, [1, 1, 1, 1, 1, 1, 1, 1], "{t}");
}

#[test]
fn b5_stated_what_the_sequences_state_and_relabelled_what_stays() {
    let (file, _) = std_file();
    let clauses: Vec<(&str, &Value)> = file["subjects"].as_array().unwrap().iter().flat_map(|s| s["clauses"].as_array().into_iter().flatten().map(move |c| (s["name"].as_str().unwrap(), c))).collect();
    let labels: Vec<&str> = clauses.iter().filter(|(_, c)| c["kind"] == "unexpressed").map(|(_, c)| bare_rust::libstats::reason_label(c["reason"].as_str().unwrap())).collect();
    assert_eq!(labels.len(), 65, "77 before B5: 12 closed");
    assert!(!labels.contains(&"no sequence value"));
    let n = |l: &str| labels.iter().filter(|x| **x == l).count();
    assert_eq!(
        (n("no callback as a function"), n("no promise about a result's later calls"), n("no promise of disorder"), n("no lifetime for the elements of a sequence"), n("no successor of a type parameter"), n("no from_iter subject")),
        (4, 4, 1, 1, 1, 1)
    );
    // the twelve that now state a sequence, by subject
    let says = |subject: &str, text: &str| clauses.iter().any(|(s, c)| *s == subject && c["kind"] == "ensure" && c["text"].as_str().unwrap() == text);
    for (s, t) in [
        ("std::primitive::str::chars", "ensure scalars : yields(result) == scalars(self)"),
        ("std::primitive::slice::iter", "ensure in_order : map(deref, yields(result)) == self"),
        ("std::primitive::slice::split_at_mut", "ensure part_elements : result.0 == old(self)[..mid] and result.1 == old(self)[mid..]"),
        ("std::iter::Iterator::next", "ensure head : result == old(yields(self))[0]"),
        ("std::iter::Iterator::next", "ensure advances : exists result implies yields(self) == old(yields(self))[1..]"),
        ("std::iter::Iterator::enumerate", "ensure pairs : yields(result) == enumerate(old(yields(self)))"),
        ("std::iter::Iterator::skip", "ensure after_n : yields(result) == skip(n, old(yields(self)))"),
        ("std::iter::Iterator::take", "ensure first_items : yields(result) == take(n, old(yields(self)))"),
        ("std::iter::Iterator::copied", "ensure copies : yields(result) == map(deref, old(yields(self)))"),
        ("std::iter::IntoIterator::into_iter", "ensure same_items : yields(result) == yields(self)"),
        ("std::vec::Vec as std::iter::IntoIterator::into_iter", "ensure in_order : yields(result) == self"),
    ] {
        assert!(says(s, t), "{s}: {t}");
    }
    // an adapter's callback is not a named function, so `map` and its kin stay under a label of their own
    for s in ["map", "filter", "take_while", "flat_map"] {
        let name = format!("std::iter::Iterator::{s}");
        assert!(clauses.iter().any(|(n, c)| *n == name && c["kind"] == "unexpressed" && c["reason"].as_str().unwrap().starts_with("no callback as a function")), "{s}");
        assert!(!clauses.iter().any(|(n, c)| *n == name && c["text"].as_str().unwrap().contains("yields(")), "{s}: a closure is not a named function");
    }
}

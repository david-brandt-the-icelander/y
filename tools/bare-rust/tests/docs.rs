//! `docs/bare-format.md` shows each kind of subject as Rust, bare body and constraint JSON. The blocks are the real
//! output of `strip` on `docs/bare-format-src/lib.rs`; this test strips it again and checks every block against it.
use serde_json::Value;
use std::fs;

fn read(p: &str) -> String {
    fs::read_to_string(format!("{}/{p}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// The fenced blocks of a Markdown text: (language, text).
fn blocks(md: &str) -> Vec<(String, String)> {
    let (mut out, mut cur): (Vec<(String, String)>, Option<(String, String)>) = (vec![], None);
    for line in md.lines() {
        match (&mut cur, line.strip_prefix("```")) {
            (None, Some(lang)) => cur = Some((lang.trim().to_string(), String::new())),
            (Some(_), Some(_)) => out.push(cur.take().unwrap()),
            (Some((_, t)), None) => {
                t.push_str(line);
                t.push('\n');
            }
            _ => {}
        }
    }
    out
}

#[test]
fn the_format_reference_matches_what_strip_writes() {
    let rs = read("docs/bare-format-src/lib.rs");
    let (brs, js) = bare_rust::strip(&rs, "format_demo 0.1.0", "src/lib.rs").unwrap();
    let subjects = js["subjects"].as_array().unwrap();
    let md = read("docs/bare-format.md");
    let bs: Vec<(String, String)> = blocks(&md).into_iter().filter(|(l, _)| matches!(l.as_str(), "rust" | "brs" | "json")).collect();
    assert_eq!(bs.len() % 3, 0, "every example is a rust, a brs and a json block");
    let mut kinds = std::collections::BTreeSet::new();
    for (i, triple) in bs.chunks(3).enumerate() {
        let langs: Vec<&str> = triple.iter().map(|b| b.0.as_str()).collect();
        assert_eq!(langs, ["rust", "brs", "json"], "example {i}");
        assert!(rs.contains(triple[0].1.trim_end()), "example {i}: the Rust block is not in docs/bare-format-src/lib.rs:\n{}", triple[0].1);
        assert!(brs.contains(triple[1].1.trim_end()), "example {i}: the bare block is not what strip writes:\n{}\n--- strip wrote:\n{brs}", triple[1].1);
        let stream = serde_json::Deserializer::from_str(&triple[2].1).into_iter::<Value>();
        let mut n = 0;
        for v in stream {
            let v = v.unwrap_or_else(|e| panic!("example {i}: json block: {e}"));
            n += 1;
            if let Some(h) = v.get("holes") {
                for (k, x) in h.as_object().unwrap() {
                    assert_eq!(&js["holes"][k.as_str()], x, "example {i}: holes of {k}");
                }
            } else {
                assert!(subjects.contains(&v), "example {i}: the subject is not what strip writes: {v}");
                kinds.insert(format!("{}{}", v["kind"].as_str().unwrap(), if v["variants"].is_array() { ":enum" } else { "" }));
            }
        }
        assert!(n > 0, "example {i}: empty json block");
    }
    assert!(bs.len() / 3 >= 9, "an example for each kind of subject");
    for k in ["function", "impl", "alias", "type", "type:enum"] {
        assert!(kinds.contains(k), "no example of {k}: {kinds:?}");
    }
    for word in ["scaffold", "holes", "TODO", "no subject"] {
        assert!(md.contains(word), "the reference does not mention {word}");
    }
}

#[test]
fn the_example_source_is_a_crate_that_compiles() {
    let rs = read("docs/bare-format-src/lib.rs");
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/bare-rust-test/docs-src");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let f = dir.join("lib.rs");
    fs::write(&f, rs).unwrap();
    let out = std::process::Command::new("rustc").args(["--crate-type", "lib", "--edition", "2021", "--out-dir"]).arg(&dir).arg(&f).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

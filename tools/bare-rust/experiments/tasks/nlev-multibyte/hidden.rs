use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn umlaut() { close(normalized_levenshtein("löwen", "lowen"), 0.8); close(normalized_levenshtein("lowen", "löwen"), 0.8); }
#[test] fn cjk() { close(normalized_levenshtein("日本語", "日本人"), 2.0 / 3.0); close(normalized_levenshtein("日本語", ""), 0.0); }
#[test] fn ascii() { close(normalized_levenshtein("kitten", "sitting"), 0.57142); close(normalized_levenshtein("", ""), 1.0); }

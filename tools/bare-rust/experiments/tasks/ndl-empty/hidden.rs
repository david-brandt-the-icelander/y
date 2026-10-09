use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn both_empty() { let v = normalized_damerau_levenshtein("", ""); assert!(!v.is_nan()); close(v, 1.0); }
#[test] fn one_empty() { close(normalized_damerau_levenshtein("", "flower"), 0.0); close(normalized_damerau_levenshtein("tree", ""), 0.0); }
#[test] fn regular() { close(normalized_damerau_levenshtein("levenshtein", "löwenbräu"), 0.27272); close(normalized_damerau_levenshtein("sunglasses", "sunglasses"), 1.0); }

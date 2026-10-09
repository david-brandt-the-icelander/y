use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn same() { close(jaro("a", "a"), 1.0); close(jaro_winkler("a", "a"), 1.0); }
#[test] fn different() { close(jaro("a", "b"), 0.0); close(jaro_winkler("a", "b"), 0.0); }
#[test] fn one_and_two() { close(jaro("a", "ab"), (1.0 + 0.5 + 1.0) / 3.0); close(jaro("ab", "a"), (0.5 + 1.0 + 1.0) / 3.0); }
#[test] fn generic() { close(generic_jaro(&vec![7], &vec![7]), 1.0); close(generic_jaro_winkler(&vec![7], &vec![8]), 0.0); }
#[test] fn empty() { close(jaro("", ""), 1.0); close(jaro("", "a"), 0.0); }

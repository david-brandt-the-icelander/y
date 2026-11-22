use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn martha() { close(jaro("MARTHA", "MARHTA"), 0.9444); close(jaro_winkler("MARTHA", "MARHTA"), 0.9611); }
#[test] fn dixon() { close(jaro("DIXON", "DICKSONX"), 0.7667); }
#[test] fn generic() { close(generic_jaro(&vec![1, 2, 3, 4, 5, 6], &vec![1, 2, 3, 5, 4, 6]), (1.0 + 1.0 + 5.0 / 6.0) / 3.0); }
#[test] fn no_transposition() { close(jaro("abc", "abc"), 1.0); }

use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn values() {
    close(normalized_damerau_levenshtein("levenshtein", "löwenbräu"), 0.27272);
    close(normalized_damerau_levenshtein("", ""), 1.0);
    close(normalized_damerau_levenshtein("", "flower"), 0.0);
    close(normalized_damerau_levenshtein("tree", ""), 0.0);
    close(normalized_damerau_levenshtein("sunglasses", "sunglasses"), 1.0);
}
#[test] fn transposition_is_one_edit() { close(normalized_damerau_levenshtein("abcd", "acbd"), 0.75); }

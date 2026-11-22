use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn prefix_is_capped_at_four() {
    close(jaro_winkler("abcdefghij", "abcdefghik"), 0.96);
}
#[test] fn generic_prefix_is_capped_at_four() {
    let a: Vec<u32> = (0..10).collect();
    let mut b = a.clone();
    b[9] = 99;
    close(generic_jaro_winkler(&a, &b), 0.96);
}
#[test] fn never_above_one() {
    assert!(jaro_winkler("thequickbrownfoxjumpedoverthelazydog", "thequickbrownfoxjumpedoverthelazydogs") <= 1.0);
    close(jaro_winkler("same", "same"), 1.0);
}
#[test] fn short_prefix_unchanged() {
    close(jaro_winkler("MARTHA", "MARHTA"), 0.9611);
    close(jaro_winkler("DIXON", "DICKSONX"), 0.8133);
}

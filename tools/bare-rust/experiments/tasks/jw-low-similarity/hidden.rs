use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn low_similarity_is_not_boosted() {
    let j = jaro("abxxxxxxxxx", "abyyyyyyyyy");
    close(jaro_winkler("abxxxxxxxxx", "abyyyyyyyyy"), j);
    close(j, 0.4545);
}
#[test] fn generic_low_similarity_is_not_boosted() {
    let a = vec![1, 2, 7, 7, 7, 7, 7, 7, 7, 7, 7];
    let b = vec![1, 2, 8, 8, 8, 8, 8, 8, 8, 8, 8];
    close(generic_jaro_winkler(&a, &b), generic_jaro(&a, &b));
}
#[test] fn high_similarity_is_boosted() {
    close(jaro_winkler("MARTHA", "MARHTA"), 0.9611);
    close(jaro_winkler("cheeseburger", "cheese fries"), 0.8667);
}

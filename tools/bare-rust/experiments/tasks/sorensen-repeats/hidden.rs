use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn aa_aaaa() { close(sorensen_dice("aa", "aaaa"), 0.5); close(sorensen_dice("aaaa", "aa"), 0.5); }
#[test] fn never_above_one() { for (a, b) in [("abababab", "ab"), ("night", "nacht"), ("aaaaaa", "aaa")] { let v = sorensen_dice(a, b); assert!(v >= 0.0 && v <= 1.0, "{} {} -> {}", a, b, v); } }
#[test] fn multiset() { close(sorensen_dice("abab", "ababab"), 0.75); }
#[test] fn plain() { close(sorensen_dice("feris", "ferris"), 0.8889); close(sorensen_dice("french", "quebec"), 0.0); }

use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn basics() {
    close(sorensen_dice("", ""), 1.0);
    close(sorensen_dice("a", "a"), 1.0);
    close(sorensen_dice("a", "b"), 0.0);
    close(sorensen_dice("", "a"), 0.0);
    close(sorensen_dice("french", "quebec"), 0.0);
    close(sorensen_dice("france", "france"), 1.0);
}
#[test] fn values() {
    close(sorensen_dice("feris", "ferris"), 0.88889);
    close(sorensen_dice("iphone", "iphone x"), 0.90909);
    close(sorensen_dice("healed", "sealed"), 0.8);
    close(sorensen_dice("fRaNce", "france"), 0.2);
}
#[test] fn whitespace_and_repeats() { close(sorensen_dice("apple event", "apple    event"), 1.0); close(sorensen_dice("aa", "aaaa"), 0.5); }

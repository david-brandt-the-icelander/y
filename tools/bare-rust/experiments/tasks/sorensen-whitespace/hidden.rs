use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn spaces() { close(sorensen_dice("apple event", "apple    event"), 1.0); close(sorensen_dice("fer ris", "ferris"), 1.0); }
#[test] fn other_whitespace() { close(sorensen_dice("a\tb\nc", "abc"), 1.0); close(sorensen_dice("ab\u{a0}cd", "abcd"), 1.0); }
#[test] fn still_compares() { close(sorensen_dice("iphone", "iphone x"), 0.90909); close(sorensen_dice("healed", "sealed"), 0.8); }
#[test] fn blank() { close(sorensen_dice("   ", ""), 1.0); close(sorensen_dice(" a ", "b"), 0.0); }

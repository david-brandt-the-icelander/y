use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn ints() { close(generic_normalized_levenshtein(&[1, 2, 3], &[1, 2, 4]), 2.0 / 3.0); close(generic_normalized_levenshtein(&vec![1, 2, 3], &vec![1, 2, 3, 4, 5, 6]), 0.5); }
#[test] fn empties() { close(generic_normalized_levenshtein(&Vec::<u8>::new(), &Vec::<u8>::new()), 1.0); close(generic_normalized_levenshtein(&Vec::<u8>::new(), &vec![1u8]), 0.0); close(generic_normalized_levenshtein(&vec![1u8, 2], &Vec::<u8>::new()), 0.0); }
#[test] fn strings_agree() { close(normalized_levenshtein("kitten", "sitting"), 0.57142); close(normalized_levenshtein("", ""), 1.0); }
#[test] fn mixed() { close(generic_normalized_levenshtein(&vec![String::from("a"), String::from("b")], &vec!["a", "c"]), 0.5); }

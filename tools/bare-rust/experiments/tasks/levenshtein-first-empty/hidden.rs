use strsim::*;
#[test] fn first_empty() { assert_eq!(levenshtein("", "abc"), 3); assert_eq!(levenshtein("", "ä"), 1); }
#[test] fn generic_first_empty() { assert_eq!(generic_levenshtein(&Vec::<u8>::new(), &vec![1, 2, 3, 4]), 4); }
#[test] fn second_empty() { assert_eq!(levenshtein("abc", ""), 3); }
#[test] fn both_empty() { assert_eq!(levenshtein("", ""), 0); }
#[test] fn regular() { assert_eq!(levenshtein("kitten", "sitting"), 3); assert_eq!(generic_levenshtein(&[1, 2, 3], &[1, 2, 3, 4, 5, 6]), 3); }
#[test] fn normalized_first_empty() { assert!(normalized_levenshtein("", "abc").abs() < 1e-9); }

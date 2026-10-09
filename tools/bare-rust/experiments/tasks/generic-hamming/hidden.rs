use strsim::*;
#[test] fn ints() { assert_eq!(generic_hamming(&[1, 2, 4], &[1, 2, 3]), Ok(1)); assert_eq!(generic_hamming(vec![1, 2], vec![3, 4]), Ok(2)); }
#[test] fn chars() { assert_eq!(generic_hamming("hamming".chars(), "hammers".chars()), Ok(3)); }
#[test] fn mixed_item_types() { assert_eq!(generic_hamming(vec![String::from("a"), String::from("b")], vec!["a", "c"]), Ok(1)); }
#[test] fn lengths() { assert_eq!(generic_hamming(&[1], &[1, 2]), Err(StrSimError::DifferentLengthArgs)); assert_eq!(generic_hamming(Vec::<u8>::new(), vec![]), Ok(0)); }
#[test] fn str_version_still_works() { assert_eq!(hamming("hamming", "hammers"), Ok(3)); assert_eq!(hamming("ham", "hamming"), Err(StrSimError::DifferentLengthArgs)); }

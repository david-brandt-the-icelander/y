use strsim::*;
#[test] fn str_longer_second() { assert_eq!(hamming("ham", "hamming"), Err(StrSimError::DifferentLengthArgs)); }
#[test] fn str_longer_first() { assert_eq!(hamming("hamming", "ham"), Err(StrSimError::DifferentLengthArgs)); }
#[test] fn generic_lengths() {
    assert_eq!(generic_hamming(&[1, 2], &[1]), Err(StrSimError::DifferentLengthArgs));
    assert_eq!(generic_hamming(vec![1], vec![1, 2]), Err(StrSimError::DifferentLengthArgs));
    assert_eq!(generic_hamming("ham".chars(), "hamming".chars()), Err(StrSimError::DifferentLengthArgs));
}
#[test] fn equal_lengths() { assert_eq!(hamming("hamming", "hammers"), Ok(3)); assert_eq!(hamming("", ""), Ok(0)); assert_eq!(generic_hamming(&[1, 2, 4], &[1, 2, 3]), Ok(1)); }
#[test] fn multibyte() { assert_eq!(hamming("häm", "ham"), Ok(1)); }

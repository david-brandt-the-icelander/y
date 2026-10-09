use strsim::*;
#[test] fn swap() { assert_eq!(osa_distance("ab", "ba"), 1); }
#[test] fn swap_inside() { assert_eq!(osa_distance("abcd", "acbd"), 1); assert_eq!(osa_distance("abcdef", "abdcfe"), 2); }
#[test] fn restricted() { assert_eq!(osa_distance("ca", "abc"), 3); assert_eq!(osa_distance("ac", "cba"), 3); }
#[test] fn plain() { assert_eq!(osa_distance("kitten", "sitting"), 3); assert_eq!(osa_distance("", "abc"), 3); }
#[test] fn same() { assert_eq!(osa_distance("same", "same"), 0); }

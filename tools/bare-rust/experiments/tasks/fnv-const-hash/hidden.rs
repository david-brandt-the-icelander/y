use fnv::*;
use std::hash::Hasher;
const FOOBAR: u64 = fnv_hash(b"foobar");
const EMPTY: u64 = fnv_hash(b"");
fn h(b: &[u8]) -> u64 { let mut x = FnvHasher::default(); x.write(b); x.finish() }
#[test] fn consts() { assert_eq!(FOOBAR, 0x85944171f73967e8); assert_eq!(EMPTY, 0xcbf29ce484222325); }
#[test] fn agrees_with_hasher() { for s in [&b"a"[..], b"hello world", b"\0\x01\xff", &[7u8; 300][..]] { assert_eq!(fnv_hash(s), h(s)); } }

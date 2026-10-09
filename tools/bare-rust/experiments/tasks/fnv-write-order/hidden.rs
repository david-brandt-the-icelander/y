use fnv::*;
use std::hash::Hasher;
fn h(b: &[u8]) -> u64 { let mut x = FnvHasher::default(); x.write(b); x.finish() }
#[test] fn vectors() {
    assert_eq!(h(b""), 0xcbf29ce484222325);
    assert_eq!(h(b"a"), 0xaf63dc4c8601ec8c);
    assert_eq!(h(b"b"), 0xaf63df4c8601f1a5);
    assert_eq!(h(b"foo"), 0xdcb27518fed9d577);
    assert_eq!(h(b"foobar"), 0x85944171f73967e8);
    assert_eq!(h(b"a\0"), 0x089be207b544f1e4);
}
#[test] fn pieces() { let mut x = FnvHasher::default(); x.write(b"foo"); x.write(b"bar"); assert_eq!(x.finish(), h(b"foobar")); }
#[test] fn with_key() { let mut x = FnvHasher::with_key(0xcbf29ce484222325); x.write(b"a"); assert_eq!(x.finish(), 0xaf63dc4c8601ec8c); }
#[test] fn in_map() { let mut m: std::collections::HashMap<&str, i32, std::hash::BuildHasherDefault<FnvHasher>> = Default::default(); m.insert("k", 1); assert_eq!(m["k"], 1); }

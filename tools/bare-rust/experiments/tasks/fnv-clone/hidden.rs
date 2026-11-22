use fnv::*;
use std::hash::Hasher;
#[test] fn clone_is_independent() {
    let mut h = FnvHasher::default();
    h.write(b"foo");
    let mut g = h.clone();
    g.write(b"bar");
    let mut whole = FnvHasher::default();
    whole.write(b"foobar");
    assert_eq!(g.finish(), whole.finish());
    let mut foo = FnvHasher::default();
    foo.write(b"foo");
    assert_eq!(h.finish(), foo.finish());
}
#[test] fn clone_of_keyed() { let h = FnvHasher::with_key(7); assert_eq!(h.clone().finish(), 7); }
fn needs_clone<T: Clone>(_: &T) {}
#[test] fn bound() { needs_clone(&FnvHasher::default()); }

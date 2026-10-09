use fnv::*;
use std::hash::BuildHasher;
#[test] fn map() {
    let mut m: FnvHashMap<&str, u32> = FnvHashMap::default();
    m.insert("one", 1);
    m.insert("two", 2);
    assert_eq!(m.get("two"), Some(&2));
    let m2: FnvHashMap<u8, u8> = FnvHashMap::with_capacity_and_hasher(10, Default::default());
    assert!(m2.is_empty());
}
#[test] fn set() { let mut s = FnvHashSet::default(); s.insert(1); s.insert(1); s.insert(2); assert_eq!(s.len(), 2); }
#[test] fn builder() { let b = FnvBuildHasher::default(); let _: &FnvBuildHasher = &b; assert_eq!(b.hash_one(1u8), b.hash_one(1u8)); }

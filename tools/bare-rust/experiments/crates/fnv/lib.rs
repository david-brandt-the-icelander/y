//! An implementation of the [Fowler–Noll–Vo hash function][chongo].
//!
//! ## About
//!
//! The FNV hash function is a custom `Hasher` implementation that is more
//! efficient for smaller hash keys.
//!
//! [The Rust Standard Library documentation][docs] states that while the default
//! `Hasher` implementation, SipHash, is good in many cases, it is notably slower
//! than other algorithms with short keys, such as when you have a map of integers
//! to other values.  In cases like these, [FNV is demonstrably faster][graphs].
//!
//! Its disadvantages are that it performs badly on larger inputs, and
//! provides no protection against collision attacks, where a malicious user
//! can craft specific keys designed to slow a hasher down. Thus, it is
//! important to profile your program to ensure that you are using small hash
//! keys, and be certain that your program could not be exposed to malicious
//! inputs (including being a networked server).
//!
//! The Rust compiler itself uses FNV, as it is not worried about
//! denial-of-service attacks, and can assume that its inputs are going to be
//! small—a perfect use case for FNV.
//!
#![cfg_attr(
    feature = "std",
    doc = r#"

## Using FNV in a `HashMap`

The `FnvHashMap` type alias is the easiest way to use the standard library’s
`HashMap` with FNV.

```rust
use fnv::FnvHashMap;

let mut map = FnvHashMap::default();
map.insert(1, "one");
map.insert(2, "two");

map = FnvHashMap::with_capacity_and_hasher(10, Default::default());
map.insert(1, "one");
map.insert(2, "two");
```

Note, the standard library’s `HashMap::new` and `HashMap::with_capacity`
are only implemented for the `RandomState` hasher, so using `Default` to
get the hasher is the next best option.

## Using FNV in a `HashSet`

Similarly, `FnvHashSet` is a type alias for the standard library’s `HashSet`
with FNV.

```rust
use fnv::FnvHashSet;

let mut set = FnvHashSet::default();
set.insert(1);
set.insert(2);

set = FnvHashSet::with_capacity_and_hasher(10, Default::default());
set.insert(1);
set.insert(2);
```
"#
)]
//!
//! [chongo]: http://www.isthe.com/chongo/tech/comp/fnv/index.html
//! [docs]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
//! [graphs]: https://cglab.ca/~abeinges/blah/hash-rs/

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(not(feature = "std"), test))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use core::default::Default;
#[cfg(not(feature = "std"))]
use core::hash::{BuildHasherDefault, Hasher};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "std")]
use std::default::Default;
#[cfg(feature = "std")]
use std::hash::{BuildHasherDefault, Hasher};

const INITIAL_STATE: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0100_0000_01b3;

/// An implementation of the Fowler–Noll–Vo hash function.
///
/// See the [crate documentation](index.html) for more details.
#[allow(missing_copy_implementations)]
#[derive(Clone)]
pub struct FnvHasher(u64);

impl Default for FnvHasher {
    #[inline]
    fn default() -> FnvHasher {
        FnvHasher(INITIAL_STATE)
    }
}

impl FnvHasher {
    /// Create an FNV hasher starting with a state corresponding
    /// to the hash `key`.
    #[inline]
    #[must_use]
    pub fn with_key(key: u64) -> FnvHasher {
        FnvHasher(key)
    }
}

impl Hasher for FnvHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let FnvHasher(mut hash) = *self;

        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }

        *self = FnvHasher(hash);
    }
}

/// A builder for default FNV hashers.
pub type FnvBuildHasher = BuildHasherDefault<FnvHasher>;

/// A `HashMap` using a default FNV hasher.
#[cfg(feature = "std")]
pub type FnvHashMap<K, V> = HashMap<K, V, FnvBuildHasher>;

/// A `HashSet` using a default FNV hasher.
#[cfg(feature = "std")]
pub type FnvHashSet<T> = HashSet<T, FnvBuildHasher>;

/// Const version of FNV hash.
#[inline]
#[must_use]
pub const fn fnv_hash(bytes: &[u8]) -> u64 {
    let mut hash = INITIAL_STATE;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(PRIME);
        i += 1;
    }
    hash
}

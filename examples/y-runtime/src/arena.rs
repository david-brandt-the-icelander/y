//! Y's arenas and resources (Y § 3.6).
//!
//! - An [`Arena`] is `arena name:`: what is allocated in it is released all at once when the block ends, and nothing
//!   allocated there escapes. Values in it refer to each other by typed [`Index`], which is how Y keeps graphs.
//! - A [`Resource`] is a type that declares release behaviour; [`Held`] releases it exactly once, at the end of the
//!   scope of its last holder, on normal exit and on failure alike.
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ARENA: AtomicU64 = AtomicU64::new(1);

/// Y's `index<unit_type=t>` into an arena: it indexes only what it counts, in the arena that made it.
pub struct Index<T> {
    arena: u64,
    at: usize,
    _of: PhantomData<fn() -> T>,
}

impl<T> Clone for Index<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Index<T> {}

impl<T> PartialEq for Index<T> {
    fn eq(&self, other: &Self) -> bool {
        self.arena == other.arena && self.at == other.at
    }
}

impl<T> Eq for Index<T> {}

impl<T> std::fmt::Debug for Index<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "index {} of arena {}", self.at, self.arena)
    }
}

/// Y's `arena name:`: its values live until it ends, and then go all at once.
pub struct Arena<T> {
    id: u64,
    name: &'static str,
    values: Vec<T>,
}

impl<T> Arena<T> {
    /// The arena a block `arena name:` opens.
    pub fn new(name: &'static str) -> Self {
        Arena { id: NEXT_ARENA.fetch_add(1, Ordering::Relaxed), name, values: Vec::new() }
    }

    /// Allocates a value in the arena, and gives its index.
    pub fn allocate(&mut self, value: T) -> Index<T> {
        self.values.push(value);
        Index { arena: self.id, at: self.values.len() - 1, _of: PhantomData }
    }

    fn checked(&self, index: Index<T>) -> usize {
        if index.arena != self.id {
            super::fault(self.name, "an index from another arena");
        }
        index.at
    }

    /// The value at an index of this arena. An index of another arena is a fault.
    pub fn get(&self, index: Index<T>) -> &T {
        &self.values[self.checked(index)]
    }

    pub fn get_mut(&mut self, index: Index<T>) -> &mut T {
        let at = self.checked(index);
        &mut self.values[at]
    }

    /// How many values it holds.
    pub fn count(&self) -> usize {
        self.values.len()
    }

    /// Every value, in the order allocated.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.values.iter()
    }
}

/// A type that declares release behaviour: Y's resource (Y § 3.6). Release may fail, with the failure the type names.
pub trait Resource {
    type ReleaseFailure;
    /// Releases what the value holds outside memory. Called exactly once, by [`Held`].
    fn release(&mut self) -> Result<(), Self::ReleaseFailure>;
}

/// A resource in the scope of its holder, released exactly once: by [`Held::end`] on a normal exit, which returns the
/// release's failure, or when the scope is left early by a failure, when the scope's own failure is the one reported
/// and a failing release is written to the error stream rather than lost.
pub struct Held<R: Resource> {
    resource: Option<R>,
}

impl<R: Resource> Held<R> {
    pub fn new(resource: R) -> Self {
        Held { resource: Some(resource) }
    }

    /// The resource, while it is held.
    pub fn get(&mut self) -> &mut R {
        self.resource.as_mut().unwrap_or_else(|| super::fault("y_runtime::arena::Held", "a resource used after release"))
    }

    /// The end of the scope: releases the resource, and fails as its release does.
    pub fn end(mut self) -> Result<(), R::ReleaseFailure> {
        match self.resource.take() {
            Some(mut r) => r.release(),
            None => Ok(()),
        }
    }
}

impl<R: Resource> Drop for Held<R> {
    fn drop(&mut self) {
        if let Some(mut r) = self.resource.take() {
            if r.release().is_err() {
                eprintln!("a resource failed to release while its scope was failing");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn an_arena_keeps_a_graph_by_index() {
        struct Node {
            name: &'static str,
            parent: Option<Index<Node>>,
        }
        let mut scratch = Arena::new("parse_scratch");
        let root = scratch.allocate(Node { name: "root", parent: None });
        let leaf = scratch.allocate(Node { name: "leaf", parent: Some(root) });
        let up = scratch.get(leaf).parent.map(|p| scratch.get(p).name);
        assert_eq!((up, scratch.count()), (Some("root"), 2));
        let other: Arena<Node> = Arena::new("other");
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| other.get(root).name)).is_err());
    }

    struct Log {
        released: Rc<RefCell<u32>>,
        fails: bool,
    }

    impl Resource for Log {
        type ReleaseFailure = &'static str;
        fn release(&mut self) -> Result<(), &'static str> {
            *self.released.borrow_mut() += 1;
            if self.fails {
                Err("disk full")
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn a_resource_is_released_exactly_once() {
        let released = Rc::new(RefCell::new(0));
        let held = Held::new(Log { released: Rc::clone(&released), fails: true });
        assert_eq!(held.end(), Err("disk full"));
        assert_eq!(*released.borrow(), 1);
        {
            let _early = Held::new(Log { released: Rc::clone(&released), fails: false });
        }
        assert_eq!(*released.borrow(), 2, "a scope left early still releases");
    }
}

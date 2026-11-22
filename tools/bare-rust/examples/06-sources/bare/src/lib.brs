//! Iterators that are sources (produce elements from parameters,
//! not from another iterator).
use std::fmt;
use std::mem;

/// Creates a new unfold source with the specified closure as the "iterator
/// function" and an initial state to eventually pass to the closure.
fn unfold(initial_state, f)
{
    Unfold {
        f,
        state: initial_state,
    }
}

impl fmt::Debug for Unfold
{
    fn fmt(self, f) {
        f.debug_struct("Unfold").field("state", &self.state).finish()
    }
}

/// See [`unfold`](crate::unfold) for more information.
#[derive(Clone)]
struct Unfold {
    f,
    /// Internal state that will be passed to the closure on the next iteration
    state,
}

impl Iterator for Unfold
{
    type Item;

    #[inline]
    fn next(self) {
        (self.f)(&mut self.state)
    }
}

/// An iterator that infinitely applies function to value and yields results.
///
/// This `struct` is created by the [`iterate()`](crate::iterate) function.
/// See its documentation for more.
#[derive(Clone)]
struct Iterate {
    state,
    f,
}

impl fmt::Debug for Iterate
{
    fn fmt(self, f) {
        f.debug_struct("Iterate").field("state", &self.state).finish()
    }
}

impl Iterator for Iterate
{
    type Item;

    #[inline]
    fn next(self) {
        let next_state = (self.f)(&self.state);
        Some(mem::replace(&mut self.state, next_state))
    }

    #[inline]
    fn size_hint(self) {
        (usize::MAX, None)
    }
}

/// Creates a new iterator that infinitely applies function to value and yields results.
fn iterate(initial_value, f)
{
    Iterate {
        state: initial_value,
        f,
    }
}

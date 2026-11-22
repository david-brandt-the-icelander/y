//! Iterators that are sources (produce elements from parameters,
//! not from another iterator).
use std::fmt;
use std::mem;

/// Creates a new unfold source with the specified closure as the "iterator
/// function" and an initial state to eventually pass to the closure.
pub fn unfold<A, St, F>(initial_state: St, f: F) -> Unfold<St, F>
where
    F: FnMut(&mut St) -> Option<A>,
{
    Unfold {
        f,
        state: initial_state,
    }
}

impl<St, F> fmt::Debug for Unfold<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unfold").field("state", &self.state).finish()
    }
}

/// See [`unfold`](crate::unfold) for more information.
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Unfold<St, F> {
    f: F,
    /// Internal state that will be passed to the closure on the next iteration
    pub state: St,
}

impl<A, St, F> Iterator for Unfold<St, F>
where
    F: FnMut(&mut St) -> Option<A>,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (self.f)(&mut self.state)
    }
}

/// An iterator that infinitely applies function to value and yields results.
///
/// This `struct` is created by the [`iterate()`](crate::iterate) function.
/// See its documentation for more.
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Iterate<St, F> {
    state: St,
    f: F,
}

impl<St, F> fmt::Debug for Iterate<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Iterate").field("state", &self.state).finish()
    }
}

impl<St, F> Iterator for Iterate<St, F>
where
    F: FnMut(&St) -> St,
{
    type Item = St;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next_state = (self.f)(&self.state);
        Some(mem::replace(&mut self.state, next_state))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

/// Creates a new iterator that infinitely applies function to value and yields results.
pub fn iterate<St, F>(initial_value: St, f: F) -> Iterate<St, F>
where
    F: FnMut(&St) -> St,
{
    Iterate {
        state: initial_value,
        f,
    }
}

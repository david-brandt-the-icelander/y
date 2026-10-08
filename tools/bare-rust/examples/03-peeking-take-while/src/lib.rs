use std::iter::Peekable;

/// An iterator that allows peeking at an element before deciding to accept it.
///
/// This is implemented by peeking adaptors like peekable and put back,
/// but also by a few iterators that can be peeked natively, like the slice’s
/// by reference iterator ([`std::slice::Iter`]).
pub trait PeekingNext: Iterator {
    /// Pass a reference to the next iterator element to the closure `accept`;
    /// if `accept` returns `true`, return it as the next element,
    /// else `None`.
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnOnce(&Self::Item) -> bool;
}

impl<I> PeekingNext for &mut I
where
    I: PeekingNext,
{
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        F: FnOnce(&Self::Item) -> bool,
    {
        (*self).peeking_next(accept)
    }
}

impl<I> PeekingNext for Peekable<I>
where
    I: Iterator,
{
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        F: FnOnce(&Self::Item) -> bool,
    {
        if let Some(r) = self.peek() {
            if !accept(r) {
                return None;
            }
        }
        self.next()
    }
}

impl<T> PeekingNext for ::std::vec::IntoIter<T> {
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        F: FnOnce(&Self::Item) -> bool,
    {
        match accept(self.as_slice().first()?) {
            true => self.next(),
            false => None,
        }
    }
}

/// An iterator adaptor that takes items while a closure returns `true`.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct PeekingTakeWhile<'a, I, F>
where
    I: Iterator + 'a,
{
    iter: &'a mut I,
    f: F,
}

/// Create a `PeekingTakeWhile`
pub fn peeking_take_while<I, F>(iter: &mut I, f: F) -> PeekingTakeWhile<'_, I, F>
where
    I: Iterator,
{
    PeekingTakeWhile { iter, f }
}

impl<I, F> Iterator for PeekingTakeWhile<'_, I, F>
where
    I: PeekingNext,
    F: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.peeking_next(&mut self.f)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<I, F> PeekingNext for PeekingTakeWhile<'_, I, F>
where
    I: PeekingNext,
    F: FnMut(&I::Item) -> bool,
{
    fn peeking_next<G>(&mut self, g: G) -> Option<Self::Item>
    where
        G: FnOnce(&Self::Item) -> bool,
    {
        let f = &mut self.f;
        self.iter.peeking_next(|r| f(r) && g(r))
    }
}

// Some iterators are so lightweight we can simply clone them to save their
// state and use that for peeking.
impl<'a, T> PeekingNext for ::std::slice::Iter<'a, T> {
    fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
    where
        F: FnOnce(&Self::Item) -> bool,
    {
        let saved_state = self.clone();
        if let Some(r) = self.next() {
            if !accept(&r) {
                *self = saved_state;
            } else {
                return Some(r);
            }
        }
        None
    }
}

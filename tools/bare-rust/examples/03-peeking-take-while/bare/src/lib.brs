use std::iter::Peekable;

/// An iterator that allows peeking at an element before deciding to accept it.
///
/// This is implemented by peeking adaptors like peekable and put back,
/// but also by a few iterators that can be peeked natively, like the slice’s
/// by reference iterator ([`std::slice::Iter`]).
trait PeekingNext {
    /// Pass a reference to the next iterator element to the closure `accept`;
    /// if `accept` returns `true`, return it as the next element,
    /// else `None`.
    fn peeking_next(self, accept);
}

impl PeekingNext for I
{
    fn peeking_next(self, accept)
    {
        (*self).peeking_next(accept)
    }
}

impl PeekingNext for Peekable
{
    fn peeking_next(self, accept)
    {
        if let Some(r) = self.peek() {
            if !accept(r) {
                return None;
            }
        }
        self.next()
    }
}

impl PeekingNext for ::std::vec::IntoIter {
    fn peeking_next(self, accept)
    {
        match accept(self.as_slice().first()?) {
            true => self.next(),
            false => None,
        }
    }
}

/// An iterator adaptor that takes items while a closure returns `true`.
struct PeekingTakeWhile
{
    iter,
    f,
}

/// Create a `PeekingTakeWhile`
fn peeking_take_while(iter, f)
{
    PeekingTakeWhile { iter, f }
}

impl Iterator for PeekingTakeWhile
{
    type Item;
    fn next(self) {
        self.iter.peeking_next(&mut self.f)
    }

    fn size_hint(self) {
        (0, self.iter.size_hint().1)
    }
}

impl PeekingNext for PeekingTakeWhile
{
    fn peeking_next(self, g)
    {
        let f = &mut self.f;
        self.iter.peeking_next(|r| f(r) && g(r))
    }
}

// Some iterators are so lightweight we can simply clone them to save their
// state and use that for peeking.
impl PeekingNext for ::std::slice::Iter {
    fn peeking_next(self, accept)
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

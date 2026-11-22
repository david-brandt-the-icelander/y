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
macro_rules! peeking_next_by_clone {
    ([$($typarm:tt)*] $type_:ty) => {
        impl<$($typarm)*> PeekingNext for $type_ {
            fn peeking_next<F>(&mut self, accept: F) -> Option<Self::Item>
                where F: FnOnce(&Self::Item) -> bool
            {
                let saved_state = self.clone();
                if let Some(r) = self.next() {
                    if !accept(&r) {
                        *self = saved_state;
                    } else {
                        return Some(r)
                    }
                }
                None
            }
        }
    }
}


peeking_next_by_clone! { ['a, T] ::std::slice::Iter<'a, T> }
peeking_next_by_clone! { ['a] ::std::str::Chars<'a> }
peeking_next_by_clone! { ['a] ::std::str::CharIndices<'a> }
peeking_next_by_clone! { ['a] ::std::str::Bytes<'a> }
peeking_next_by_clone! { ['a, T] ::std::option::Iter<'a, T> }
peeking_next_by_clone! { ['a, T] ::std::result::Iter<'a, T> }
peeking_next_by_clone! { [T] ::std::iter::Empty<T> }

// cloning a Rev has no extra overhead; peekable and put backs are never DEI.
peeking_next_by_clone! { [I: Clone + PeekingNext + DoubleEndedIterator]
::std::iter::Rev<I> }

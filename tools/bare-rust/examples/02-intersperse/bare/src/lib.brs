use std::iter::{Fuse, FusedIterator};

trait IntersperseElement {
    fn generate(self);
}

#[derive(Debug, Clone)]
struct IntersperseElementSimple(_);

impl IntersperseElement for IntersperseElementSimple {
    fn generate(self) {
        self.0.clone()
    }
}

/// An iterator adaptor to insert a particular value
/// between each element of the adapted iterator.
///
/// Iterator element type is `I::Item`
///
/// This iterator is *fused*.
///
/// See [`.intersperse()`](crate::Itertools::intersperse) for more information.
type Intersperse;

/// Create a new Intersperse iterator
fn intersperse(iter, elt)
{
    intersperse_with(iter, IntersperseElementSimple(elt))
}

impl IntersperseElement for F {
    fn generate(self) {
        self()
    }
}

/// An iterator adaptor to insert a particular value created by a function
/// between each element of the adapted iterator.
///
/// Iterator element type is `I::Item`
///
/// This iterator is *fused*.
///
/// See [`.intersperse_with()`](crate::Itertools::intersperse_with) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
struct IntersperseWith
{
    element,
    iter,
    /// `peek` is None while no item have been taken out of `iter` (at definition).
    /// Then `peek` will alternatively be `Some(None)` and `Some(Some(item))`,
    /// where `None` indicates it's time to generate from `element` (unless `iter` is empty).
    peek,
}

/// Create a new `IntersperseWith` iterator
fn intersperse_with(iter, elt)
{
    IntersperseWith {
        peek: None,
        iter: iter.fuse(),
        element: elt,
    }
}

impl Iterator for IntersperseWith
{
    type Item;
    #[inline]
    fn next(self) {
        let Self {
            element,
            iter,
            peek,
        } = self;
        match peek {
            Some(item @ Some(_)) => item.take(),
            Some(None) => match iter.next() {
                new @ Some(_) => {
                    *peek = Some(new);
                    Some(element.generate())
                }
                None => None,
            },
            None => {
                *peek = Some(None);
                iter.next()
            }
        }
    }

    fn size_hint(self) {
        let sh = self.iter.size_hint();
        sh = size_hint::add(sh, sh);
        match self.peek {
            Some(Some(_)) => size_hint::add_scalar(sh, 1),
            Some(None) => sh,
            None => size_hint::sub_scalar(sh, 1),
        }
    }

    fn fold(self, init, f)
    {
        let Self {
            element,
            iter,
            peek,
        } = self;
        let accum = init;

        if let Some(x) = peek.unwrap_or_else(|| iter.next()) {
            accum = f(accum, x);
        }

        iter.fold(accum, |accum, x| {
            let accum = f(accum, element.generate());
            f(accum, x)
        })
    }
}

impl FusedIterator for IntersperseWith
{
}

mod size_hint {
    type SizeHint;

    fn add(a, b) {
        let min = a.0.saturating_add(b.0);
        let max = match (a.1, b.1) {
            (Some(x), Some(y)) => x.checked_add(y),
            _ => None,
        };
        (min, max)
    }

    fn add_scalar(sh, x) {
        let (low, hi) = sh;
        low = low.saturating_add(x);
        hi = hi.and_then(|elt| elt.checked_add(x));
        (low, hi)
    }

    fn sub_scalar(sh, x) {
        let (low, hi) = sh;
        low = low.saturating_sub(x);
        hi = hi.map(|elt| elt.saturating_sub(x));
        (low, hi)
    }
}

use std::iter::Fuse;
use std::ops::Index;

use self::size_hint::SizeHint;

pub struct LazyBuffer<I: Iterator> {
    it: Fuse<I>,
    buffer: Vec<I::Item>,
}

impl<I> LazyBuffer<I>
where
    I: Iterator,
{
    pub fn new(it: I) -> Self {
        Self {
            it: it.fuse(),
            buffer: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn size_hint(&self) -> SizeHint {
        size_hint::add_scalar(self.it.size_hint(), self.len())
    }

    pub fn count(self) -> usize {
        self.len() + self.it.count()
    }

    pub fn get_next(&mut self) -> bool {
        if let Some(x) = self.it.next() {
            self.buffer.push(x);
            true
        } else {
            false
        }
    }

    pub fn prefill(&mut self, len: usize) {
        let buffer_len = self.buffer.len();
        if len > buffer_len {
            let delta = len - buffer_len;
            self.buffer.extend(self.it.by_ref().take(delta));
        }
    }
}

impl<I> LazyBuffer<I>
where
    I: Iterator,
    I::Item: Clone,
{
    pub fn get_at(&self, indices: &[usize]) -> Vec<I::Item> {
        indices.iter().map(|i| self.buffer[*i].clone()).collect()
    }

    pub fn get_array<const K: usize>(&self, indices: [usize; K]) -> [I::Item; K] {
        indices.map(|i| self.buffer[i].clone())
    }
}

impl<I, J> Index<J> for LazyBuffer<I>
where
    I: Iterator,
    I::Item: Sized,
    Vec<I::Item>: Index<J>,
{
    type Output = <Vec<I::Item> as Index<J>>::Output;

    fn index(&self, index: J) -> &Self::Output {
        self.buffer.index(index)
    }
}

mod size_hint {
    /// `SizeHint` is the return type of `Iterator::size_hint()`.
    pub type SizeHint = (usize, Option<usize>);

    /// Add `x` correctly to a `SizeHint`.
    #[inline]
    pub fn add_scalar(sh: SizeHint, x: usize) -> SizeHint {
        let (mut low, mut hi) = sh;
        low = low.saturating_add(x);
        hi = hi.and_then(|elt| elt.checked_add(x));
        (low, hi)
    }
}

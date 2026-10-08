use std::iter::Fuse;
use std::ops::Index;

use self::size_hint::SizeHint;

struct LazyBuffer {
    it,
    buffer,
}

impl LazyBuffer
{
    fn new(it) {
        Self {
            it: it.fuse(),
            buffer: Vec::new(),
        }
    }

    fn len(self) {
        self.buffer.len()
    }

    fn size_hint(self) {
        size_hint::add_scalar(self.it.size_hint(), self.len())
    }

    fn count(self) {
        self.len() + self.it.count()
    }

    fn get_next(self) {
        if let Some(x) = self.it.next() {
            self.buffer.push(x);
            true
        } else {
            false
        }
    }

    fn prefill(self, len) {
        let buffer_len = self.buffer.len();
        if len > buffer_len {
            let delta = len - buffer_len;
            self.buffer.extend(self.it.by_ref().take(delta));
        }
    }
}

impl LazyBuffer
{
    fn get_at(self, indices) {
        indices.iter().map(|i| self.buffer[*i].clone()).collect()
    }
}

impl Index for LazyBuffer
{
    type Output;

    fn index(self, index) {
        self.buffer.index(index)
    }
}

mod size_hint {
    /// `SizeHint` is the return type of `Iterator::size_hint()`.
    type SizeHint;

    /// Add `x` correctly to a `SizeHint`.
    #[inline]
    fn add_scalar(sh, x) {
        let (low, hi) = sh;
        low = low.saturating_add(x);
        hi = hi.and_then(|elt| elt.checked_add(x));
        (low, hi)
    }
}

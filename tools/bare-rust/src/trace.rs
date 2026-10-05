//! The span map of a woven file: every byte weave or the `mut` pass inserted, and where it came from, so that a
//! compiler error in the woven text can be reported against the bare file or the constraint slot (proposal 6, step 4).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Printed from a slot of the constraint file.
    Slot,
    /// Printed from a hole's `locals` or `hints` entry.
    Hole,
    /// A `mut` the compiler asked for.
    Mut,
}

/// What an inserted span came from: a subject of the constraint file and a slot path in it.
#[derive(Clone, Debug)]
pub struct Origin {
    pub kind: Kind,
    pub subject: String,
    pub slot: String,
}

impl Origin {
    pub fn describe(&self) -> String {
        match self.kind {
            Kind::Mut => "mut derived through rustc".into(),
            Kind::Slot => format!("slot {} of `{}`", self.slot, self.subject),
            Kind::Hole => format!("hole {} of `{}`", self.slot, self.subject),
        }
    }
}

/// A woven range that is not a copy of the bare text: it replaces `bare` (empty for an insertion).
#[derive(Clone, Debug)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub bare_start: usize,
    pub bare_end: usize,
    pub origin: Origin,
}

#[derive(Clone, Debug, Default)]
pub struct SpanMap {
    pub spans: Vec<Span>,
}

/// A position in the bare file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarePos {
    pub offset: usize,
    pub line: usize,
    pub col: usize,
}

pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let before = &src.as_bytes()[..offset];
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let start = before.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    (line, String::from_utf8_lossy(&before[start..]).chars().count() + 1)
}

impl SpanMap {
    /// The bare offset of a woven offset. A position inside an inserted span maps to the span's bare start
    /// (or its bare end, for an end position).
    pub fn to_bare(&self, pos: usize, end: bool) -> usize {
        let mut delta: isize = 0;
        for s in &self.spans {
            if s.end <= pos {
                delta += (s.end - s.start) as isize - (s.bare_end - s.bare_start) as isize;
            } else if s.start < pos {
                return if end { s.bare_end } else { s.bare_start };
            } else {
                break;
            }
        }
        (pos as isize - delta).max(0) as usize
    }

    /// The inserted spans a woven range is at: its start, or its end, lies inside the span. A range that merely
    /// wraps an inserted span is not at it. Narrowest first.
    pub fn touching(&self, start: usize, end: usize) -> Vec<&Span> {
        let mut v: Vec<&Span> =
            self.spans.iter().filter(|s| (s.start <= start && start < s.end) || (s.start < end && end <= s.end)).collect();
        v.sort_by_key(|s| s.end - s.start);
        v
    }

    /// Record a `mut ` insertion at a woven offset: later spans move, and the new span is kept in order.
    pub fn insert_mut(&mut self, pos: usize, len: usize, subject: &str) {
        let bare = self.to_bare(pos, false);
        let (mut at, n) = (self.spans.len(), self.spans.len());
        for (i, s) in self.spans.iter_mut().enumerate() {
            if s.start >= pos {
                if at == n {
                    at = i;
                }
                s.start += len;
                s.end += len;
            } else if s.end > pos {
                s.end += len; // inside an inserted span: it grows
            }
        }
        let origin = Origin { kind: Kind::Mut, subject: subject.to_string(), slot: "mut".into() };
        self.spans.insert(at, Span { start: pos, end: pos + len, bare_start: bare, bare_end: bare, origin });
    }
}

//! `sensitive<value_type=t>` and `reveal(x) because "…"` (Y § 3.3, § 3.15): a value that is not shown, and the
//! capability that shows it. A stand-in: a sensitive value is masked wherever it is written, `Debug` and `Display` both
//! giving `<sensitive>`, so a `log` that interpolates one shows the mask (Y § 3.7); [`Reveal`] gives the value, and the
//! lowering marks each use with its claim.
use std::fmt;

/// A value that is not shown: `Debug` and `Display` mask it.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Sensitive<T>(T);

impl<T> Sensitive<T> {
    pub fn new(value: T) -> Self {
        Sensitive(value)
    }
}

impl<T> fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("<sensitive>")
    }
}

impl<T> fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("<sensitive>")
    }
}

/// The capability `reveal`: the right to see a sensitive value, which the entry point has from the root.
pub struct Reveal {
    _private: (),
}

impl Reveal {
    /// The capability, for a test.
    pub fn fake() -> Reveal {
        Reveal { _private: () }
    }

    /// `reveal(value) because "…"`: the value.
    pub fn reveal<'a, T>(&self, value: &'a Sensitive<T>) -> &'a T {
        &value.0
    }
}

impl crate::System {
    /// The `reveal` capability.
    pub fn reveal(&self) -> Reveal {
        Reveal { _private: () }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sensitive_value_is_masked_and_revealed_only_by_the_capability() {
        let s = Sensitive::new("hunter2");
        assert_eq!((format!("{s}"), format!("{s:?}")), ("<sensitive>".to_string(), "<sensitive>".to_string()));
        assert_eq!(*Reveal::fake().reveal(&s), "hunter2");
    }
}

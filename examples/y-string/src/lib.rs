//! Y's `standard.string`, y-string (Y § 3.3): the native implementation of Y's string abstraction.
//!
//! | Y | Here |
//! |---|---|
//! | `string` | [`YString`]: a sequence of runes, normalised to NFC when it is made |
//! | `rune` | [`Rune`]: one extended grapheme cluster |
//! | `scalar` | `char`: one Unicode scalar value |
//! | `string<character set=ascii>` | [`AsciiString`]: one byte per rune, counted and indexed in constant time |
//! | `bytes` | `Vec<u8>` |
//!
//! Counting and indexing a [`YString`] walk its grapheme boundaries; an [`AsciiString`] needs no walk, which is the
//! optimisation the `character set` constraint allows. A position is from 0, as the lowering's `index<of=rune>` is.
use std::fmt;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use y_runtime::ConstraintViolation;

/// One rune: one extended grapheme cluster, such as `e`, `é` or `👍🏽`, which may be several scalars.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rune(String);

impl Rune {
    /// Its scalars, in order.
    pub fn scalars(&self) -> std::str::Chars<'_> {
        self.0.chars()
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Rune {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An encoding of scalars as code units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Encoding {
    Utf8,
    Utf16,
}

/// What to do with input that is not valid in its encoding: `invalid_input.reject` or `invalid_input.replace`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InvalidInput {
    Reject,
    /// Each invalid sequence becomes U+FFFD.
    Replace,
}

/// The failure of a decoding that rejects invalid input: where the first invalid byte is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidEncoding {
    pub at_byte: usize,
}

/// Y's `string`: a sequence of runes. It is normalised to NFC when it is made, so canonically equivalent strings are
/// equal, hash alike and order alike.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct YString(String);

impl YString {
    /// The string a Y literal or a constructor makes from host text: normalised to NFC.
    pub fn new(text: &str) -> Self {
        YString(text.nfc().collect())
    }

    /// Y's `string.decode_utf8(bytes, on_invalid: …)`.
    pub fn decode_utf8(bytes: &[u8], on_invalid: InvalidInput) -> Result<Self, InvalidEncoding> {
        match (std::str::from_utf8(bytes), on_invalid) {
            (Ok(text), _) => Ok(YString::new(text)),
            (Err(e), InvalidInput::Reject) => Err(InvalidEncoding { at_byte: e.valid_up_to() }),
            (Err(_), InvalidInput::Replace) => Ok(YString::new(&String::from_utf8_lossy(bytes))),
        }
    }

    /// Its runes, in order.
    pub fn runes(&self) -> impl Iterator<Item = Rune> + '_ {
        self.0.graphemes(true).map(|g| Rune(g.to_string()))
    }

    /// Y's `count()`: how many runes, which walks the grapheme boundaries.
    pub fn count(&self) -> usize {
        self.0.graphemes(true).count()
    }

    /// The rune at a position, if there is one.
    pub fn rune_at(&self, index: usize) -> Option<Rune> {
        self.0.graphemes(true).nth(index).map(|g| Rune(g.to_string()))
    }

    /// Y's `scalars()`.
    pub fn scalars(&self) -> std::str::Chars<'_> {
        self.0.chars()
    }

    /// Y's `code_units(encoding).count()`: what a host limit such as `NVARCHAR(n)` counts.
    pub fn code_unit_count(&self, encoding: Encoding) -> usize {
        match encoding {
            Encoding::Utf8 => self.0.len(),
            Encoding::Utf16 => self.0.encode_utf16().count(),
        }
    }

    /// Y's `encode(encoding.utf8)`, as bytes.
    pub fn encode_utf8(&self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    /// The string where it meets `character set=ascii`, or the constraint it breaks.
    pub fn to_ascii(&self) -> Result<AsciiString, ConstraintViolation> {
        AsciiString::new(&self.0)
    }

    /// The host text, for a native function that needs it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for YString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Y's `string<character set=ascii>`: every rune is one ASCII scalar and one byte, so counting and indexing take
/// constant time. ASCII text is already in NFC.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AsciiString(String);

impl AsciiString {
    /// The text where it is ASCII; otherwise the constraint it breaks.
    pub fn new(text: &str) -> Result<Self, ConstraintViolation> {
        if text.is_ascii() {
            Ok(AsciiString(text.to_string()))
        } else {
            Err(ConstraintViolation { constraint: "character set=ascii" })
        }
    }

    /// Y's `count()`, in constant time: one rune per byte.
    pub fn count(&self) -> usize {
        self.0.len()
    }

    /// The rune at a position, in constant time.
    pub fn rune_at(&self, index: usize) -> Option<Rune> {
        self.0.get(index..index + 1).map(|r| Rune(r.to_string()))
    }

    pub fn runes(&self) -> impl Iterator<Item = Rune> + '_ {
        self.0.chars().map(|c| Rune(c.to_string()))
    }

    /// The same value as a string of any runes: an ASCII string is a `string`, with no conversion in Y.
    pub fn to_string_of_runes(&self) -> YString {
        YString(self.0.clone())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonically_equivalent_strings_are_equal() {
        let composed = YString::new("caf\u{e9}");
        let decomposed = YString::new("cafe\u{301}");
        assert_eq!(composed, decomposed);
        assert_eq!((decomposed.count(), decomposed.scalars().count()), (4, 4));
    }

    #[test]
    fn counting_names_what_it_counts() {
        let thumbs = YString::new("ok 👍🏽 🇮🇸");
        assert_eq!(thumbs.count(), 6, "o, k, space, the thumb, space, the flag");
        assert_eq!(thumbs.scalars().count(), 8, "the thumb and the flag are two scalars each");
        assert_eq!(thumbs.code_unit_count(Encoding::Utf16), 12);
        assert_eq!(thumbs.code_unit_count(Encoding::Utf8), 20);
        assert_eq!(thumbs.rune_at(3).map(|r| r.scalars().count()), Some(2));
        assert_eq!(thumbs.rune_at(5).map(|r| r.to_string()), Some("🇮🇸".to_string()));
        assert_eq!(thumbs.rune_at(6), None);
    }

    #[test]
    fn an_ascii_string_is_a_constraint_met() {
        let word = YString::new("hangman").to_ascii().expect("ASCII");
        assert_eq!((word.count(), word.rune_at(4).map(|r| r.to_string())), (7, Some("m".to_string())));
        assert_eq!(word.to_string_of_runes(), YString::new("hangman"));
        assert_eq!(YString::new("café").to_ascii(), Err(ConstraintViolation { constraint: "character set=ascii" }));
    }

    #[test]
    fn decoding_rejects_or_replaces_as_asked() {
        let bad = [b'a', 0xff, b'b'];
        assert_eq!(YString::decode_utf8(&bad, InvalidInput::Reject), Err(InvalidEncoding { at_byte: 1 }));
        assert_eq!(YString::decode_utf8(&bad, InvalidInput::Replace).map(|s| s.count()), Ok(3));
        assert_eq!(YString::decode_utf8("e\u{301}".as_bytes(), InvalidInput::Reject), Ok(YString::new("\u{e9}")));
    }
}

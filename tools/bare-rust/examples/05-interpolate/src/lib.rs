
/// Interpolate capture references in `replacement` and write the interpolation
/// result to `dst`. References in `replacement` take the form of $N or $name,
/// where `N` is a capture group index and `name` is a capture group name. The
/// function provided, `name_to_index`, maps capture group names to indices.
///
/// The `append` function given is responsible for writing the replacement
/// to the `dst` buffer. That is, it is called with the capture group index
/// of a capture group reference and is expected to resolve the index to its
/// corresponding matched text. If no such match exists, then `append` should
/// not write anything to its given buffer.
#[inline]
pub fn interpolate<A, N>(
    mut replacement: &[u8],
    mut append: A,
    mut name_to_index: N,
    dst: &mut Vec<u8>,
) where
    A: FnMut(usize, &mut Vec<u8>),
    N: FnMut(&str) -> Option<usize>,
{
    while !replacement.is_empty() {
        match replacement.iter().position(|&b| b == b'$') {
            None => break,
            Some(i) => {
                dst.extend(&replacement[..i]);
                replacement = &replacement[i..];
            }
        }
        if replacement.get(1).map_or(false, |&b| b == b'$') {
            dst.push(b'$');
            replacement = &replacement[2..];
            continue;
        }
        debug_assert!(!replacement.is_empty());
        let cap_ref = match find_cap_ref(replacement) {
            Some(cap_ref) => cap_ref,
            None => {
                dst.push(b'$');
                replacement = &replacement[1..];
                continue;
            }
        };
        replacement = &replacement[cap_ref.end..];
        match cap_ref.cap {
            Ref::Number(i) => append(i, dst),
            Ref::Named(name) => {
                if let Some(i) = name_to_index(name) {
                    append(i, dst);
                }
            }
        }
    }
    dst.extend(replacement);
}

/// `CaptureRef` represents a reference to a capture group inside some text.
/// The reference is either a capture group name or a number.
///
/// It is also tagged with the position in the text immediately proceeding the
/// capture reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaptureRef<'a> {
    cap: Ref<'a>,
    end: usize,
}

/// A reference to a capture group in some text.
///
/// e.g., `$2`, `$foo`, `${foo}`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ref<'a> {
    Named(&'a str),
    Number(usize),
}

/// Parses a possible reference to a capture group name in the given text,
/// starting at the beginning of `replacement`.
///
/// If no such valid reference could be found, None is returned.
#[inline]
fn find_cap_ref(replacement: &[u8]) -> Option<CaptureRef<'_>> {
    let mut i = 0;
    if replacement.len() <= 1 || replacement[0] != b'$' {
        return None;
    }
    let mut brace = false;
    i += 1;
    if replacement[i] == b'{' {
        brace = true;
        i += 1;
    }
    let mut cap_end = i;
    while replacement.get(cap_end).map_or(false, is_valid_cap_letter) {
        cap_end += 1;
    }
    if cap_end == i {
        return None;
    }
    // We just verified that the range 0..cap_end is valid ASCII, so it must
    // therefore be valid UTF-8. If we really cared, we could avoid this UTF-8
    // check with an unchecked conversion or by parsing the number straight
    // from &[u8].
    let cap = std::str::from_utf8(&replacement[i..cap_end])
        .expect("valid UTF-8 capture name");
    if brace {
        if !replacement.get(cap_end).map_or(false, |&b| b == b'}') {
            return None;
        }
        cap_end += 1;
    }
    Some(CaptureRef {
        cap: match cap.parse::<u32>() {
            Ok(i) => Ref::Number(i as usize),
            Err(_) => Ref::Named(cap),
        },
        end: cap_end,
    })
}

/// Returns true if and only if the given byte is allowed in a capture name.
#[inline]
fn is_valid_cap_letter(b: &u8) -> bool {
    match *b {
        b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' => true,
        _ => false,
    }
}

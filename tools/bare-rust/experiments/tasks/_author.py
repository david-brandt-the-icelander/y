#!/usr/bin/env python3
"""Generates tasks/<id>/{task.json,STATEMENT.md,hidden.rs,base.patch,ref.patch} from the specs below.

Snapshots live in experiments/crates/<crate>/ (corpus HEAD, in-source unit tests removed). Each task's base is that
snapshot with base.patch applied; the reference is the snapshot with ref.patch applied (empty patch = the snapshot).
Re-run:  python3 experiments/tasks/_author.py   (needs `diff`; rewrites every task directory's patches).
"""
import json, os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATES = os.path.join(HERE, "..", "crates")
LIB = {"strsim": "src/lib.rs", "fnv": "lib.rs"}


def cut(text, start, end):
    i = text.index(start)
    j = text.index(end, i)
    return text[:i] + text[j:]


def rep(old, new):
    return (old, new)


T = []  # task specs


def task(id, crate, kind, e5, statement, hidden, base=None, ref=None, files=None):
    T.append(dict(id=id, crate=crate, kind=kind, e5=e5, statement=statement, hidden=hidden, base=base or [],
                  ref=ref or []))


# edits are (file, old, new) or (file, ("cut", start, end))
def L(crate, old, new):
    return (LIB[crate], old, new)


# ---------------------------------------------------------------------------------------------------- body-only
task("jw-low-similarity", "strsim", "body", False,
     "`jaro_winkler` gives a score that is too high for strings that are not very similar but start with the same letters. "
     "For example `jaro_winkler(\"abxxxxxxxxx\", \"abyyyyyyyyy\")` is much larger than `jaro` of the same pair, but the "
     "Jaro-Winkler definition only boosts a pair whose Jaro similarity is already high (above 0.7). Fix it for both "
     "`jaro_winkler` and `generic_jaro_winkler`.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn low_similarity_is_not_boosted() {
    let j = jaro("abxxxxxxxxx", "abyyyyyyyyy");
    close(jaro_winkler("abxxxxxxxxx", "abyyyyyyyyy"), j);
    close(j, 0.4545);
}
#[test] fn generic_low_similarity_is_not_boosted() {
    let a = vec![1, 2, 7, 7, 7, 7, 7, 7, 7, 7, 7];
    let b = vec![1, 2, 8, 8, 8, 8, 8, 8, 8, 8, 8];
    close(generic_jaro_winkler(&a, &b), generic_jaro(&a, &b));
}
#[test] fn high_similarity_is_boosted() {
    close(jaro_winkler("MARTHA", "MARHTA"), 0.9611);
    close(jaro_winkler("cheeseburger", "cheese fries"), 0.8667);
}
''',
     base=[L("strsim", """    if sim > 0.7 {
        let prefix_length = a
            .into_iter()
            .take(4)
            .zip(b)
            .take_while(|(a_elem, b_elem)| a_elem == b_elem)
            .count();

        sim + 0.1 * prefix_length as f64 * (1.0 - sim)
    } else {
        sim
    }
}""", """    let prefix_length = a
        .into_iter()
        .take(4)
        .zip(b)
        .take_while(|(a_elem, b_elem)| a_elem == b_elem)
        .count();

    sim + 0.1 * prefix_length as f64 * (1.0 - sim)
}""")])

task("jw-prefix-cap", "strsim", "body", False,
     "`jaro_winkler` can return values above what it should for strings that share a long prefix: the prefix bonus is "
     "supposed to look at no more than the first four characters (the standard Winkler scaling), but "
     "`jaro_winkler(\"abcdefghij\", \"abcdefghik\")` is about 0.99 where the standard answer is 0.96. Same for "
     "`generic_jaro_winkler`.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn prefix_is_capped_at_four() {
    close(jaro_winkler("abcdefghij", "abcdefghik"), 0.96);
}
#[test] fn generic_prefix_is_capped_at_four() {
    let a: Vec<u32> = (0..10).collect();
    let mut b = a.clone();
    b[9] = 99;
    close(generic_jaro_winkler(&a, &b), 0.96);
}
#[test] fn never_above_one() {
    assert!(jaro_winkler("thequickbrownfoxjumpedoverthelazydog", "thequickbrownfoxjumpedoverthelazydogs") <= 1.0);
    close(jaro_winkler("same", "same"), 1.0);
}
#[test] fn short_prefix_unchanged() {
    close(jaro_winkler("MARTHA", "MARHTA"), 0.9611);
    close(jaro_winkler("DIXON", "DICKSONX"), 0.8133);
}
''',
     base=[L("strsim", "            .take(4)\n", "")])

task("jaro-transpositions", "strsim", "body", False,
     "`jaro` overcounts transpositions: `jaro(\"MARTHA\", \"MARHTA\")` should be 0.9444 (the textbook value) but we "
     "return 0.8889, and `jaro_winkler` inherits the problem (textbook 0.9611). `generic_jaro` has the same fault.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn martha() { close(jaro("MARTHA", "MARHTA"), 0.9444); close(jaro_winkler("MARTHA", "MARHTA"), 0.9611); }
#[test] fn dixon() { close(jaro("DIXON", "DICKSONX"), 0.7667); }
#[test] fn generic() { close(generic_jaro(&vec![1, 2, 3, 4, 5, 6], &vec![1, 2, 3, 5, 4, 6]), (1.0 + 1.0 + 5.0 / 6.0) / 3.0); }
#[test] fn no_transposition() { close(jaro("abc", "abc"), 1.0); }
''',
     base=[L("strsim", "    transpositions /= 2;\n", "")])

task("jaro-single-char", "strsim", "body", True,
     "`jaro(\"a\", \"a\")` panics with an arithmetic overflow in debug builds. Strings of length one (on either side, "
     "against anything) must give ordinary results: 1.0 for equal one-character strings and 0.0 for different ones. "
     "`jaro_winkler` and the generic versions are affected too.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn same() { close(jaro("a", "a"), 1.0); close(jaro_winkler("a", "a"), 1.0); }
#[test] fn different() { close(jaro("a", "b"), 0.0); close(jaro_winkler("a", "b"), 0.0); }
#[test] fn one_and_two() { close(jaro("a", "ab"), (1.0 + 0.5 + 1.0) / 3.0); close(jaro("ab", "a"), (0.5 + 1.0 + 1.0) / 3.0); }
#[test] fn generic() { close(generic_jaro(&vec![7], &vec![7]), 1.0); close(generic_jaro_winkler(&vec![7], &vec![8]), 0.0); }
#[test] fn empty() { close(jaro("", ""), 1.0); close(jaro("", "a"), 0.0); }
''',
     base=[L("strsim", """    let mut search_range = max(a_len, b_len) / 2;
    search_range = search_range.saturating_sub(1);""", """    let mut search_range = max(a_len, b_len) / 2;
    search_range -= 1;""")])

task("osa-transposition", "strsim", "body", False,
     "`osa_distance` is wrong when strings differ by an adjacent swap: `osa_distance(\"ab\", \"ba\")` returns 0, "
     "although the strings are different (a transposition counts as one edit). The other operations are fine.",
     r'''
use strsim::*;
#[test] fn swap() { assert_eq!(osa_distance("ab", "ba"), 1); }
#[test] fn swap_inside() { assert_eq!(osa_distance("abcd", "acbd"), 1); assert_eq!(osa_distance("abcdef", "abdcfe"), 2); }
#[test] fn restricted() { assert_eq!(osa_distance("ca", "abc"), 3); assert_eq!(osa_distance("ac", "cba"), 3); }
#[test] fn plain() { assert_eq!(osa_distance("kitten", "sitting"), 3); assert_eq!(osa_distance("", "abc"), 3); }
#[test] fn same() { assert_eq!(osa_distance("same", "same"), 0); }
''',
     base=[L("strsim", "prev_two_distances[j - 1] + 1);", "prev_two_distances[j - 1]);")])

task("ndl-empty", "strsim", "body", False,
     "`normalized_damerau_levenshtein(\"\", \"\")` returns NaN. It should be 1.0 (two empty strings are identical), "
     "like `normalized_levenshtein` does. Every other input is fine.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn both_empty() { let v = normalized_damerau_levenshtein("", ""); assert!(!v.is_nan()); close(v, 1.0); }
#[test] fn one_empty() { close(normalized_damerau_levenshtein("", "flower"), 0.0); close(normalized_damerau_levenshtein("tree", ""), 0.0); }
#[test] fn regular() { close(normalized_damerau_levenshtein("levenshtein", "löwenbräu"), 0.27272); close(normalized_damerau_levenshtein("sunglasses", "sunglasses"), 1.0); }
''',
     base=[L("strsim", """pub fn normalized_damerau_levenshtein(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }

""", """pub fn normalized_damerau_levenshtein(a: &str, b: &str) -> f64 {
""")])

task("sorensen-whitespace", "strsim", "body", True,
     "`sorensen_dice` should ignore whitespace: `sorensen_dice(\"apple event\", \"apple    event\")` should be 1.0 and "
     "`sorensen_dice(\"fer ris\", \"ferris\")` too, but today the spaces take part in the bigrams and the scores are "
     "lower. All kinds of whitespace (tabs, newlines, non-breaking space) count.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn spaces() { close(sorensen_dice("apple event", "apple    event"), 1.0); close(sorensen_dice("fer ris", "ferris"), 1.0); }
#[test] fn other_whitespace() { close(sorensen_dice("a\tb\nc", "abc"), 1.0); close(sorensen_dice("ab\u{a0}cd", "abcd"), 1.0); }
#[test] fn still_compares() { close(sorensen_dice("iphone", "iphone x"), 0.90909); close(sorensen_dice("healed", "sealed"), 0.8); }
#[test] fn blank() { close(sorensen_dice("   ", ""), 1.0); close(sorensen_dice(" a ", "b"), 0.0); }
''',
     base=[L("strsim", """    let a: String = a.chars().filter(|&x| !char::is_whitespace(x)).collect();
    let b: String = b.chars().filter(|&x| !char::is_whitespace(x)).collect();
""", """    let a: String = a.to_string();
    let b: String = b.to_string();
""")])

task("sorensen-repeats", "strsim", "body", True,
     "`sorensen_dice` counts repeated bigrams wrongly. A bigram that occurs once in the first string should match at "
     "most one occurrence in the second: `sorensen_dice(\"aa\", \"aaaa\")` must be 0.5 but returns 1.5, a score above "
     "1.0. The coefficient is 2 * |shared bigrams (as multisets)| / (total bigrams of both).",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn aa_aaaa() { close(sorensen_dice("aa", "aaaa"), 0.5); close(sorensen_dice("aaaa", "aa"), 0.5); }
#[test] fn never_above_one() { for (a, b) in [("abababab", "ab"), ("night", "nacht"), ("aaaaaa", "aaa")] { let v = sorensen_dice(a, b); assert!(v >= 0.0 && v <= 1.0, "{} {} -> {}", a, b, v); } }
#[test] fn multiset() { close(sorensen_dice("abab", "ababab"), 0.75); }
#[test] fn plain() { close(sorensen_dice("feris", "ferris"), 0.8889); close(sorensen_dice("french", "quebec"), 0.0); }
''',
     base=[L("strsim", """            if *bi > 0 {
                *bi -= 1;
                intersection_size += 1;
            }""", """            if *bi > 0 {
                intersection_size += 1;
            }""")])

task("hamming-lengths", "strsim", "body", True,
     "`hamming` and `generic_hamming` silently accept sequences of different lengths: `hamming(\"ham\", \"hamming\")` "
     "returns `Ok(0)`. It should return `Err(StrSimError::DifferentLengthArgs)` when the lengths differ, whichever of "
     "the two is longer.",
     r'''
use strsim::*;
#[test] fn str_longer_second() { assert_eq!(hamming("ham", "hamming"), Err(StrSimError::DifferentLengthArgs)); }
#[test] fn str_longer_first() { assert_eq!(hamming("hamming", "ham"), Err(StrSimError::DifferentLengthArgs)); }
#[test] fn generic_lengths() {
    assert_eq!(generic_hamming(&[1, 2], &[1]), Err(StrSimError::DifferentLengthArgs));
    assert_eq!(generic_hamming(vec![1], vec![1, 2]), Err(StrSimError::DifferentLengthArgs));
    assert_eq!(generic_hamming("ham".chars(), "hamming".chars()), Err(StrSimError::DifferentLengthArgs));
}
#[test] fn equal_lengths() { assert_eq!(hamming("hamming", "hammers"), Ok(3)); assert_eq!(hamming("", ""), Ok(0)); assert_eq!(generic_hamming(&[1, 2, 4], &[1, 2, 3]), Ok(1)); }
#[test] fn multibyte() { assert_eq!(hamming("häm", "ham"), Ok(1)); }
''',
     base=[L("strsim", "            _ => return Err(StrSimError::DifferentLengthArgs),", "            _ => return Ok(count),")])

task("levenshtein-first-empty", "strsim", "body", True,
     "`levenshtein(\"\", \"abc\")` returns 0 but must be 3 (the number of insertions), and `generic_levenshtein` "
     "does the same with empty sequences. `levenshtein(\"abc\", \"\")` and the non-empty cases are right.",
     r'''
use strsim::*;
#[test] fn first_empty() { assert_eq!(levenshtein("", "abc"), 3); assert_eq!(levenshtein("", "ä"), 1); }
#[test] fn generic_first_empty() { assert_eq!(generic_levenshtein(&Vec::<u8>::new(), &vec![1, 2, 3, 4]), 4); }
#[test] fn second_empty() { assert_eq!(levenshtein("abc", ""), 3); }
#[test] fn both_empty() { assert_eq!(levenshtein("", ""), 0); }
#[test] fn regular() { assert_eq!(levenshtein("kitten", "sitting"), 3); assert_eq!(generic_levenshtein(&[1, 2, 3], &[1, 2, 3, 4, 5, 6]), 3); }
#[test] fn normalized_first_empty() { assert!(normalized_levenshtein("", "abc").abs() < 1e-9); }
''',
     base=[L("strsim", "    let mut result = b_len;\n", "    let mut result = 0;\n")])

task("nlev-multibyte", "strsim", "body", True,
     "`normalized_levenshtein` gives wrong scores for non-ASCII text: `normalized_levenshtein(\"löwen\", \"lowen\")` "
     "should be 0.8 (one substitution out of five characters) but we return about 0.83. The edit distance itself is "
     "right; the normalisation is not.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn umlaut() { close(normalized_levenshtein("löwen", "lowen"), 0.8); close(normalized_levenshtein("lowen", "löwen"), 0.8); }
#[test] fn cjk() { close(normalized_levenshtein("日本語", "日本人"), 2.0 / 3.0); close(normalized_levenshtein("日本語", ""), 0.0); }
#[test] fn ascii() { close(normalized_levenshtein("kitten", "sitting"), 0.57142); close(normalized_levenshtein("", ""), 1.0); }
''',
     base=[L("strsim", "(a.chars().count().max(b.chars().count()) as f64)", "(a.len().max(b.len()) as f64)")])

task("fnv-write-order", "fnv", "body", True,
     "`FnvHasher` produces the wrong hash values: `hash(b\"a\")` should be 0xaf63dc4c8601ec8c (the published FNV-1a "
     "test vector) and `b\"foobar\"` 0x85944171f73967e8. Hashes agree with the reference implementation for the empty "
     "input only. Writing in several pieces must give the same result as writing the whole input at once.",
     r'''
use fnv::*;
use std::hash::Hasher;
fn h(b: &[u8]) -> u64 { let mut x = FnvHasher::default(); x.write(b); x.finish() }
#[test] fn vectors() {
    assert_eq!(h(b""), 0xcbf29ce484222325);
    assert_eq!(h(b"a"), 0xaf63dc4c8601ec8c);
    assert_eq!(h(b"b"), 0xaf63df4c8601f1a5);
    assert_eq!(h(b"foo"), 0xdcb27518fed9d577);
    assert_eq!(h(b"foobar"), 0x85944171f73967e8);
    assert_eq!(h(b"a\0"), 0x089be207b544f1e4);
}
#[test] fn pieces() { let mut x = FnvHasher::default(); x.write(b"foo"); x.write(b"bar"); assert_eq!(x.finish(), h(b"foobar")); }
#[test] fn with_key() { let mut x = FnvHasher::with_key(0xcbf29ce484222325); x.write(b"a"); assert_eq!(x.finish(), 0xaf63dc4c8601ec8c); }
#[test] fn in_map() { let mut m: std::collections::HashMap<&str, i32, std::hash::BuildHasherDefault<FnvHasher>> = Default::default(); m.insert("k", 1); assert_eq!(m["k"], 1); }
''',
     base=[L("fnv", """            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }

        *self""", """            hash = hash.wrapping_mul(PRIME);
            hash ^= u64::from(*byte);
        }

        *self""")])

# ------------------------------------------------------------------------------------------------ signature-changing
task("fnv-clone", "fnv", "signature", True,
     "I need to snapshot a `FnvHasher` part-way through a stream, keep feeding the original, and later continue from "
     "the snapshot, but `FnvHasher` is not `Clone`. Please make it cloneable (the clone must be independent of the "
     "original).",
     r'''
use fnv::*;
use std::hash::Hasher;
#[test] fn clone_is_independent() {
    let mut h = FnvHasher::default();
    h.write(b"foo");
    let mut g = h.clone();
    g.write(b"bar");
    let mut whole = FnvHasher::default();
    whole.write(b"foobar");
    assert_eq!(g.finish(), whole.finish());
    let mut foo = FnvHasher::default();
    foo.write(b"foo");
    assert_eq!(h.finish(), foo.finish());
}
#[test] fn clone_of_keyed() { let h = FnvHasher::with_key(7); assert_eq!(h.clone().finish(), 7); }
fn needs_clone<T: Clone>(_: &T) {}
#[test] fn bound() { needs_clone(&FnvHasher::default()); }
''',
     base=[L("fnv", "#[derive(Clone)]\npub struct FnvHasher", "pub struct FnvHasher")])

task("fnv-const-hash", "fnv", "signature", False,
     "We need to compute FNV hashes of byte strings at compile time, for example `const KEY: u64 = fnv_hash(b\"name\");`. "
     "Add a free function `fnv_hash(bytes: &[u8]) -> u64` usable in `const` contexts that returns the same value as "
     "writing `bytes` into a default `FnvHasher`.",
     r'''
use fnv::*;
use std::hash::Hasher;
const FOOBAR: u64 = fnv_hash(b"foobar");
const EMPTY: u64 = fnv_hash(b"");
fn h(b: &[u8]) -> u64 { let mut x = FnvHasher::default(); x.write(b); x.finish() }
#[test] fn consts() { assert_eq!(FOOBAR, 0x85944171f73967e8); assert_eq!(EMPTY, 0xcbf29ce484222325); }
#[test] fn agrees_with_hasher() { for s in [&b"a"[..], b"hello world", b"\0\x01\xff", &[7u8; 300][..]] { assert_eq!(fnv_hash(s), h(s)); } }
''',
     base=[])

task("fnv-aliases", "fnv", "signature", True,
     "Using `FnvHasher` with the standard collections is clumsy: one has to spell out `HashMap<K, V, BuildHasherDefault<FnvHasher>>`. "
     "Please add the type aliases `FnvHashMap<K, V>` and `FnvHashSet<T>` (available when the `std` feature is on) "
     "and `FnvBuildHasher` for the builder.",
     r'''
use fnv::*;
use std::hash::BuildHasher;
#[test] fn map() {
    let mut m: FnvHashMap<&str, u32> = FnvHashMap::default();
    m.insert("one", 1);
    m.insert("two", 2);
    assert_eq!(m.get("two"), Some(&2));
    let m2: FnvHashMap<u8, u8> = FnvHashMap::with_capacity_and_hasher(10, Default::default());
    assert!(m2.is_empty());
}
#[test] fn set() { let mut s = FnvHashSet::default(); s.insert(1); s.insert(1); s.insert(2); assert_eq!(s.len(), 2); }
#[test] fn builder() { let b = FnvBuildHasher::default(); let _: &FnvBuildHasher = &b; assert_eq!(b.hash_one(1u8), b.hash_one(1u8)); }
''',
     base=[])  # filled below

task("sorensen-dice", "strsim", "signature", True,
     "Please add `sorensen_dice(a: &str, b: &str) -> f64`, the Sorensen-Dice similarity of two strings based on "
     "their bigrams (adjacent character pairs), between 0.0 and 1.0. Whitespace is ignored, equal strings score 1.0, a "
     "string with fewer than two characters scores 0.0 against anything different, and repeated bigrams are counted "
     "as a multiset.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn basics() {
    close(sorensen_dice("", ""), 1.0);
    close(sorensen_dice("a", "a"), 1.0);
    close(sorensen_dice("a", "b"), 0.0);
    close(sorensen_dice("", "a"), 0.0);
    close(sorensen_dice("french", "quebec"), 0.0);
    close(sorensen_dice("france", "france"), 1.0);
}
#[test] fn values() {
    close(sorensen_dice("feris", "ferris"), 0.88889);
    close(sorensen_dice("iphone", "iphone x"), 0.90909);
    close(sorensen_dice("healed", "sealed"), 0.8);
    close(sorensen_dice("fRaNce", "france"), 0.2);
}
#[test] fn whitespace_and_repeats() { close(sorensen_dice("apple event", "apple    event"), 1.0); close(sorensen_dice("aa", "aaaa"), 0.5); }
''',
     base=[])

task("ndl-add", "strsim", "signature", False,
     "We have `normalized_levenshtein` but no normalized version of the Damerau-Levenshtein distance. Please add "
     "`normalized_damerau_levenshtein(a: &str, b: &str) -> f64`: 1.0 for identical strings (including two empty "
     "ones), 0.0 when one is empty and the other not, otherwise 1 minus the distance over the length in characters "
     "of the longer string.",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn values() {
    close(normalized_damerau_levenshtein("levenshtein", "löwenbräu"), 0.27272);
    close(normalized_damerau_levenshtein("", ""), 1.0);
    close(normalized_damerau_levenshtein("", "flower"), 0.0);
    close(normalized_damerau_levenshtein("tree", ""), 0.0);
    close(normalized_damerau_levenshtein("sunglasses", "sunglasses"), 1.0);
}
#[test] fn transposition_is_one_edit() { close(normalized_damerau_levenshtein("abcd", "acbd"), 0.75); }
''',
     base=[])

task("generic-hamming", "strsim", "signature", False,
     "`hamming` only takes `&str`. Please add `generic_hamming(a, b)` that takes any two values implementing "
     "`IntoIterator` (by value) whose item types can be compared with `PartialEq` (the two item types may differ) "
     "and returns the same `HammingResult`: the number of differing positions, or "
     "`Err(StrSimError::DifferentLengthArgs)` if the lengths differ. `hamming` should keep working as before.",
     r'''
use strsim::*;
#[test] fn ints() { assert_eq!(generic_hamming(&[1, 2, 4], &[1, 2, 3]), Ok(1)); assert_eq!(generic_hamming(vec![1, 2], vec![3, 4]), Ok(2)); }
#[test] fn chars() { assert_eq!(generic_hamming("hamming".chars(), "hammers".chars()), Ok(3)); }
#[test] fn mixed_item_types() { assert_eq!(generic_hamming(vec![String::from("a"), String::from("b")], vec!["a", "c"]), Ok(1)); }
#[test] fn lengths() { assert_eq!(generic_hamming(&[1], &[1, 2]), Err(StrSimError::DifferentLengthArgs)); assert_eq!(generic_hamming(Vec::<u8>::new(), vec![]), Ok(0)); }
#[test] fn str_version_still_works() { assert_eq!(hamming("hamming", "hammers"), Ok(3)); assert_eq!(hamming("ham", "hamming"), Err(StrSimError::DifferentLengthArgs)); }
''',
     base=[])

task("error-traits", "strsim", "signature", False,
     "`StrSimError` cannot be used with `?` in functions returning `Box<dyn std::error::Error>` and cannot be printed with `{}`. "
     "Please make it a proper error type: `Display` with the message \"Differing length arguments provided\" and `std::error::Error`.",
     r'''
use strsim::*;
fn dist(a: &str, b: &str) -> Result<usize, Box<dyn std::error::Error>> { Ok(hamming(a, b)?) }
#[test] fn display() { assert_eq!(StrSimError::DifferentLengthArgs.to_string(), "Differing length arguments provided"); assert_eq!(format!("{}", StrSimError::DifferentLengthArgs), "Differing length arguments provided"); }
#[test] fn question_mark() { assert_eq!(dist("ham", "hum").unwrap(), 1); let e = dist("ham", "hamming").unwrap_err(); assert_eq!(e.to_string(), "Differing length arguments provided"); }
#[test] fn is_error() { fn takes<E: std::error::Error>(_: &E) {} takes(&StrSimError::DifferentLengthArgs); }
''',
     base=[])

task("generic-normalized-levenshtein", "strsim", "signature", False,
     "`normalized_levenshtein` is only available for strings. Please add `generic_normalized_levenshtein(a, b)`, "
     "mirroring `generic_levenshtein`: it takes two references to collections whose references are `IntoIterator`, "
     "with items comparable by `PartialEq` (item types may differ), and returns the score as an `f64` between 0.0 and "
     "1.0 (1.0 for two empty sequences, otherwise 1 minus the distance over the length of the longer sequence).",
     r'''
use strsim::*;
fn close(a: f64, b: f64) { assert!((a - b).abs() < 1e-4, "{} vs {}", a, b); }
#[test] fn ints() { close(generic_normalized_levenshtein(&[1, 2, 3], &[1, 2, 4]), 2.0 / 3.0); close(generic_normalized_levenshtein(&vec![1, 2, 3], &vec![1, 2, 3, 4, 5, 6]), 0.5); }
#[test] fn empties() { close(generic_normalized_levenshtein(&Vec::<u8>::new(), &Vec::<u8>::new()), 1.0); close(generic_normalized_levenshtein(&Vec::<u8>::new(), &vec![1u8]), 0.0); close(generic_normalized_levenshtein(&vec![1u8, 2], &Vec::<u8>::new()), 0.0); }
#[test] fn strings_agree() { close(normalized_levenshtein("kitten", "sitting"), 0.57142); close(normalized_levenshtein("", ""), 1.0); }
#[test] fn mixed() { close(generic_normalized_levenshtein(&vec![String::from("a"), String::from("b")], &vec!["a", "c"]), 0.5); }
''',
     base=[])

# ---- edits that need the HEAD text to cut (filled in as callables)
def fill():
    snap = {c: open(os.path.join(CRATES, c, LIB[c])).read() for c in LIB}
    by = {t["id"]: t for t in T}

    s = snap["fnv"]
    cutf = cut(s, "/// Const version of FNV hash.", "\n").replace("", "")
    # fnv_hash: remove the whole function
    i = s.index("/// Const version of FNV hash.")
    by["fnv-const-hash"]["base"] = [(LIB["fnv"], s[i:], "")]
    by["fnv-const-hash"]["_trim"] = True
    # aliases
    a0 = s.index("/// A builder for default FNV hashers.")
    a1 = s.index("/// Const version of FNV hash.")
    by["fnv-aliases"]["base"] = [(LIB["fnv"], s[a0:a1], ""),
                                 (LIB["fnv"], "#[cfg(feature = \"std\")]\nuse std::collections::{HashMap, HashSet};\n", "")]

    s = snap["strsim"]
    i = s.index("/// Returns an Iterator of char tuples.")
    by["sorensen-dice"]["base"] = [(LIB["strsim"], s[i:], "")]
    i0 = s.index("/// Calculates a normalized score of the Damerau")
    i1 = s.index("/// Returns an Iterator of char tuples.")
    by["ndl-add"]["base"] = [(LIB["strsim"], s[i0:i1], ""),
                             ("tests/lib.rs", "damerau_levenshtein, hamming, jaro, jaro_winkler, levenshtein, normalized_damerau_levenshtein,\n    normalized_levenshtein, osa_distance,", "damerau_levenshtein, hamming, jaro, jaro_winkler, levenshtein,\n    normalized_levenshtein, osa_distance,"),
                             ]
    t = open(os.path.join(CRATES, "strsim", "tests/lib.rs")).read()
    j0 = t.index("#[test]\nfn normalized_damerau_levenshtein_works()")
    j1 = t.index("#[test]\nfn jaro_works()")
    by["ndl-add"]["base"].append(("tests/lib.rs", t[j0:j1], ""))
    # generic_hamming
    g0 = s.index("/// Calculates the number of positions in the two sequences where the elements")
    g1 = s.index("/// Calculates the number of positions in the two strings where the characters")
    by["generic-hamming"]["base"] = [(LIB["strsim"], s[g0:g1], ""),
        (LIB["strsim"], "    generic_hamming(a.chars(), b.chars())\n", """    let (mut ita, mut itb) = (a.chars(), b.chars());
    let mut count = 0;
    loop {
        match (ita.next(), itb.next()) {
            (Some(x), Some(y)) => {
                if x != y {
                    count += 1;
                }
            }
            (None, None) => return Ok(count),
            _ => return Err(StrSimError::DifferentLengthArgs),
        }
    }
""")]
    # error traits
    e0 = s.index("impl Display for StrSimError")
    e1 = s.index("pub type HammingResult")
    by["error-traits"]["base"] = [(LIB["strsim"], s[e0:e1], ""),
                                  (LIB["strsim"], "use std::error::Error;\n", ""),
                                  (LIB["strsim"], "use std::fmt::{self, Display, Formatter};\n", "")]
    # generic normalized levenshtein (ref adds it)
    by["generic-normalized-levenshtein"]["ref"] = [(LIB["strsim"], "/// Calculates a normalized score of the Levenshtein algorithm between 0.0 and\n/// 1.0 (inclusive), where 1.0 means the strings are the same.\n///\n/// ```\n/// use strsim::normalized_levenshtein;", """/// Calculates a normalized score of the Levenshtein algorithm between 0.0 and
/// 1.0 (inclusive) for two sequences, where 1.0 means the sequences are the same.
pub fn generic_normalized_levenshtein<'a, 'b, Iter1, Iter2, Elem1, Elem2>(
    a: &'a Iter1,
    b: &'b Iter2,
) -> f64
where
    &'a Iter1: IntoIterator<Item = Elem1>,
    &'b Iter2: IntoIterator<Item = Elem2>,
    Elem1: PartialEq<Elem2>,
{
    let a_len = a.into_iter().count();
    let b_len = b.into_iter().count();
    if a_len == 0 && b_len == 0 {
        return 1.0;
    }
    1.0 - (generic_levenshtein(a, b) as f64) / (max(a_len, b_len) as f64)
}

/// Calculates a normalized score of the Levenshtein algorithm between 0.0 and
/// 1.0 (inclusive), where 1.0 means the strings are the same.
///
/// ```
/// use strsim::normalized_levenshtein;""")]


def write_tree(crate, edits, dest):
    shutil.copytree(os.path.join(CRATES, crate), dest)
    for e in edits:
        if e is None:
            continue
        f, old, new = e
        p = os.path.join(dest, f)
        txt = open(p).read()
        if old not in txt:
            sys.exit("edit does not apply in %s: %r" % (f, old[:60]))
        open(p, "w").write(txt.replace(old, new, 1))


def main():
    fill()
    for t in T:
        d = os.path.join(HERE, t["id"])
        os.makedirs(d, exist_ok=True)
        with tempfile.TemporaryDirectory() as tmp:
            for kind in ("base", "ref"):
                shutil.copytree(os.path.join(CRATES, t["crate"]), os.path.join(tmp, "a"))
                write_tree(t["crate"], t[kind], os.path.join(tmp, "b"))
                r = subprocess.run(["diff", "-ruN", "a", "b"], cwd=tmp, capture_output=True, text=True)
                open(os.path.join(d, kind + ".patch"), "w").write(r.stdout)
                shutil.rmtree(os.path.join(tmp, "a")); shutil.rmtree(os.path.join(tmp, "b"))
        open(os.path.join(d, "hidden.rs"), "w").write(t["hidden"].lstrip("\n"))
        open(os.path.join(d, "STATEMENT.md"), "w").write(t["statement"] + "\n")
        json.dump(dict(id=t["id"], crate=t["crate"], kind=t["kind"], e5=t["e5"]), open(os.path.join(d, "task.json"), "w"), indent=1)
    print(len(T), "tasks")


main()

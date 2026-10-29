//! A small stand-in for the Y runtime, written by hand.
//!
//! Rust lowered from Y calls this crate for what Y has and Rust does not: capabilities (`System`, `Console`, `Random`,
//! `Clock`, `Forever`), the standard library's text operations on runes and list operations by number, refined types' constraint violations, and faults.
//! It is only as large as the examples need. Where it is simpler than Y's semantics, the item says so.
use std::collections::VecDeque;
use std::fmt;
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

/// A fault: a bug, which ends the program with the stable identity of the construct that faulted (Y § 3.4).
pub fn fault(identity: &str, what: &str) -> ! {
    panic!("fault at {identity}: {what}")
}

/// The run-time check of a `requires` the compiler did not prove. A violation is a fault.
pub fn require(holds: bool, identity: &str) {
    if !holds {
        fault(identity, "precondition violated");
    }
}

/// The run-time check of an `invariant` the compiler did not prove, where a value may have broken it. A violation is a
/// fault.
pub fn invariant(holds: bool, identity: &str) {
    if !holds {
        fault(identity, "invariant violated");
    }
}

/// One character as a player sees it.
///
/// Y's `Rune` is an extended grapheme cluster. This stand-in holds one Unicode scalar value, which is the same thing
/// for the letters the examples use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rune(pub char);

impl Rune {
    pub fn is_ascii_lowercase(self) -> bool {
        self.0.is_ascii_lowercase()
    }
}

impl fmt::Display for Rune {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Text operations of Y's standard library that Rust's `str` does not have in the same form.
pub mod text {
    use super::Rune;

    /// The runes of a text, in order.
    pub fn runes(text: &str) -> impl Iterator<Item = Rune> + '_ {
        text.chars().map(Rune)
    }

    /// How many runes a text has: Y's `Count<Rune>`.
    pub fn count(text: &str) -> usize {
        text.chars().count()
    }

    /// The text's only rune, or none when it has no rune or more than one.
    pub fn single_rune(text: &str) -> Option<Rune> {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(only), None) => Some(Rune(only)),
            _ => None,
        }
    }

    /// The text repeated `times` times: Y's `string.repeated(times)`.
    pub fn repeated(text: &str, times: u32) -> String {
        (0..times).map(|_| text).collect()
    }

    /// Runes joined into a text with a separator between each two.
    pub fn join_runes(runes: &[Rune], separator: &str) -> String {
        let parts: Vec<String> = runes.iter().map(|rune| rune.to_string()).collect();
        parts.join(separator)
    }
}

/// The standard library's operations on lists that Rust's slices do not have in the same form: by number, counting from
/// 1 as a player sees them, and counted as a `uint32`.
pub mod list {
    use super::fault;

    /// How many elements a list has, as a `uint32`: Y's `list.count().to_uint32()`.
    pub fn count<T>(items: &[T]) -> u32 {
        u32::try_from(items.len()).unwrap_or_else(|_| fault("y_runtime::list::count", "more elements than a uint32 holds"))
    }

    /// How many elements a list has, as an `int32`: Y's `list.count().to_int32()`.
    pub fn count_int32<T>(items: &[T]) -> i32 {
        i32::try_from(items.len()).unwrap_or_else(|_| fault("y_runtime::list::count_int32", "more elements than an int32 holds"))
    }

    /// A list of `times` elements, each equal to `value`: Y's `list.repeated(value, times: n)`.
    pub fn repeated<T: Clone>(value: T, times: u32) -> Vec<T> {
        (0..times).map(|_| value.clone()).collect()
    }

    /// The element numbered `number`, counting from 1. A number outside the list is a fault.
    pub fn at_number<T: Copy>(items: &[T], number: u32) -> T {
        match position(items.len(), number) {
            Some(at) => items[at],
            None => fault("y_runtime::list::at_number", "no element with that number"),
        }
    }

    /// Replaces the element numbered `number`, counting from 1. A number outside the list is a fault.
    pub fn set_number<T>(items: &mut [T], number: u32, value: T) {
        match position(items.len(), number) {
            Some(at) => items[at] = value,
            None => fault("y_runtime::list::set_number", "no element with that number"),
        }
    }

    /// The number, counting from 1, of the first element equal to `value`. A value not in the list is a fault.
    pub fn number_of<T: PartialEq>(items: &[T], value: &T) -> u32 {
        match items.iter().position(|item| item == value) {
            Some(at) => count(&items[..=at]),
            None => fault("y_runtime::list::number_of", "the value is not in the list"),
        }
    }

    /// The largest element. An empty list is a fault.
    pub fn largest<T: Copy + Ord>(items: &[T]) -> T {
        match items.iter().copied().max() {
            Some(most) => most,
            None => fault("y_runtime::list::largest", "the list is empty"),
        }
    }

    /// The list without its last element; empty when the list is.
    pub fn without_last<T>(items: &[T]) -> &[T] {
        &items[..items.len().saturating_sub(1)]
    }

    fn position(length: usize, number: u32) -> Option<usize> {
        let at = usize::try_from(number).ok()?.checked_sub(1)?;
        (at < length).then_some(at)
    }
}

/// The failure of a refined type's constructor: the name of the constraint that does not hold (Y § 3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintViolation {
    pub constraint: &'static str,
}

/// The root capability. The entry point receives it, and every other capability derives from it (Y § 3.7).
pub struct System {
    _private: (),
}

impl System {
    /// The root, made once by the generated entry point.
    pub fn root() -> System {
        System { _private: () }
    }

    pub fn console(&self) -> Console {
        Console { input: Input::Terminal, written: Vec::new() }
    }

    /// A random source seeded from the clock. Replay would record the seed (Y § 4.7); this stand-in does not.
    pub fn random(&self) -> Random {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        Random::seeded(nanos as u64 ^ u64::from(std::process::id()))
    }

    /// The clock of the machine the program runs on.
    pub fn clock(&self) -> Clock {
        Clock { fake: None }
    }

    pub fn forever(&self) -> Forever {
        Forever { _private: () }
    }
}

/// Why the console could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleError {
    pub message: String,
}

impl ConsoleError {
    /// The error a fake console gives when it is told to fail reading.
    pub fn fake_read_failure() -> ConsoleError {
        ConsoleError { message: "the fake console was told to fail reading".to_string() }
    }
}

impl From<std::io::Error> for ConsoleError {
    fn from(error: std::io::Error) -> ConsoleError {
        ConsoleError { message: error.to_string() }
    }
}

enum Input {
    Terminal,
    Script { lines: VecDeque<String>, read_fails: bool },
}

/// The terminal, or a fake of it for tests: it reads lines and writes text.
pub struct Console {
    input: Input,
    written: Vec<String>,
}

impl Console {
    /// A fake console that reads the given lines, then reports that its input is closed; with `read_fails`, every
    /// read fails instead. It records what is written.
    pub fn fake(lines: &[&str], read_fails: bool) -> Console {
        let lines = lines.iter().map(|line| line.to_string()).collect();
        Console { input: Input::Script { lines, read_fails }, written: Vec::new() }
    }

    /// Writes a line.
    pub fn write_line(&mut self, text: &str) -> Result<(), ConsoleError> {
        match self.input {
            Input::Terminal => writeln!(std::io::stdout(), "{text}")?,
            Input::Script { .. } => self.written.push(text.to_string()),
        }
        Ok(())
    }

    /// Writes text with no line ending, as a prompt.
    pub fn write(&mut self, text: &str) -> Result<(), ConsoleError> {
        match self.input {
            Input::Terminal => {
                let mut out = std::io::stdout();
                write!(out, "{text}")?;
                out.flush()?;
            }
            Input::Script { .. } => self.written.push(text.to_string()),
        }
        Ok(())
    }

    /// Reads a line without its line ending, or none when the input is closed. The wait has no bound, so it needs
    /// `Forever`: the claim `forever because "..."` in Y.
    pub fn read_line(&mut self, _forever: &Forever) -> Result<Option<String>, ConsoleError> {
        match &mut self.input {
            Input::Terminal => {
                let mut line = String::new();
                let read = std::io::stdin().lock().read_line(&mut line)?;
                if read == 0 {
                    return Ok(None);
                }
                Ok(Some(line.trim_end_matches(['\n', '\r']).to_string()))
            }
            Input::Script { read_fails: true, .. } => Err(ConsoleError::fake_read_failure()),
            Input::Script { lines, .. } => Ok(lines.pop_front()),
        }
    }

    /// What a fake console has been given to write, one entry per call. Empty for the terminal.
    pub fn written(&self) -> &[String] {
        &self.written
    }
}

/// A source of random numbers: xorshift64*, which is deterministic for a seed.
pub struct Random {
    state: u64,
}

impl Random {
    pub fn seeded(seed: u64) -> Random {
        Random { state: seed | 1 }
    }

    /// An index below `count`, which must not be zero.
    pub fn index_below(&mut self, count: usize) -> usize {
        require(count > 0, "y_runtime::Random::index_below#count_positive");
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        let drawn = self.state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (drawn % count as u64) as usize
    }
}

/// The capability to read the time (Y § 3.7). Y's `instant` and `duration` lower to `std::time::Instant` and
/// `std::time::Duration`.
pub struct Clock {
    /// For a fake: when it started, how far each reading moves it on, and how many readings it has given.
    fake: Option<(Instant, Duration, u32)>,
}

impl Clock {
    /// A fake clock for a test: each reading is `tick` after the one before.
    pub fn fake(tick: Duration) -> Clock {
        Clock { fake: Some((Instant::now(), tick, 0)) }
    }

    /// The time now.
    pub fn now(&mut self) -> Instant {
        match &mut self.fake {
            None => Instant::now(),
            Some((start, tick, readings)) => {
                *readings += 1;
                *start + *tick * *readings
            }
        }
    }
}

/// The capability to wait with no bound, which `forever because "..."` requires (Y § 3.8, § 3.15).
pub struct Forever {
    _private: (),
}

impl Forever {
    /// The capability, for a test.
    pub fn fake() -> Forever {
        Forever { _private: () }
    }
}

//! A small stand-in for the Y runtime, written by hand.
//!
//! Rust lowered from Y calls this crate for what Y has and Rust does not: capabilities (`System`, `Console`, `Random`,
//! `Clock`, `Forever`), deadlines (`Deadline`, `Expired`), the standard library's text operations on runes and list operations by number, refined types' constraint violations, and faults;
//! and, in [`decimal`], the decimals, rationals and rounding modes of Y § 3.3.
//! It is only as large as the examples need. Where it is simpler than Y's semantics, the item says so.
pub mod decimal;

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

/// The run-time check of an `ensures` the compiler did not prove, on the value a function returns. A violation is a
/// fault.
pub fn ensure(holds: bool, identity: &str) {
    if !holds {
        fault(identity, "guarantee broken");
    }
}

/// The run-time check of an `assert` the compiler did not prove: `assert name = (p)` is `fail not_reachable` when `p`
/// does not hold. A violation is a fault.
pub fn assertion(holds: bool, identity: &str) {
    if !holds {
        fault(identity, "assertion broken");
    }
}

/// Y's `fail not_reachable`: a path the source declares cannot be travelled. It is not a failure case of the function;
/// reaching it is a fault, which the build's fault setting handles (Y § 3.5): this runtime terminates the process.
pub fn not_reachable(identity: &str) -> ! {
    fault(identity, "a path declared not reachable was reached")
}

/// Y's `fail not_implemented`: a path the source has not implemented yet. Like `not_reachable`, it is not a failure case
/// of the function, and reaching it is a fault.
pub fn not_implemented(identity: &str) -> ! {
    fault(identity, "a path not implemented yet was reached")
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

    /// The element numbered `number`, counting from 1, borrowed: Y's `at_number` on a list of values that are not
    /// copied, such as pictures. A number outside the list is a fault.
    pub fn at_number_ref<T>(items: &[T], number: u32) -> &T {
        match position(items.len(), number) {
            Some(at) => &items[at],
            None => fault("y_runtime::list::at_number_ref", "no element with that number"),
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

    /// The last element: Y's `list.last()` on a list that cannot be empty. An empty list is a fault.
    pub fn last<T: Clone>(items: &[T]) -> T {
        match items.last() {
            Some(found) => found.clone(),
            None => fault("y_runtime::list::last", "the list is empty"),
        }
    }

    /// The smallest element. An empty list is a fault.
    pub fn smallest<T: Copy + Ord>(items: &[T]) -> T {
        match items.iter().copied().min() {
            Some(least) => least,
            None => fault("y_runtime::list::smallest", "the list is empty"),
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

/// Y's conversions between whole numbers, which fault where Rust's `as` would wrap.
pub mod number {
    use super::fault;

    /// An `int32` as a `uint32`: Y's `int32.to_uint32()`. A number below zero is a fault.
    pub fn to_uint32(value: i32) -> u32 {
        u32::try_from(value).unwrap_or_else(|_| fault("y_runtime::number::to_uint32", "a number below zero"))
    }

    /// A `uint32` as an `int32`: Y's `uint32.to_int32()`. A number too large for an `int32` is a fault.
    pub fn to_int32(value: u32) -> i32 {
        i32::try_from(value).unwrap_or_else(|_| fault("y_runtime::number::to_int32", "a number too large for an int32"))
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

    /// The error a fake console gives when it is told to fail writing.
    pub fn fake_write_failure() -> ConsoleError {
        ConsoleError { message: "the fake console was told to fail writing".to_string() }
    }
}

impl From<std::io::Error> for ConsoleError {
    fn from(error: std::io::Error) -> ConsoleError {
        ConsoleError { message: error.to_string() }
    }
}

enum Input {
    Terminal,
    /// `writes_left`: how many more writes succeed before every write fails, if they ever do.
    /// `fails_when_out`: whether a read fails, instead of reporting that the input is closed, once the lines run out.
    Script { lines: VecDeque<String>, read_fails: bool, writes_left: Option<usize>, fails_when_out: bool },
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
        Console { input: Input::Script { lines, read_fails, writes_left: None, fails_when_out: false }, written: Vec::new() }
    }

    /// A fake console that reads the given lines, and whose writes succeed `writes` times, after which every write
    /// fails: Y's `fake console(lines: …, failing_after_writes: n)`.
    pub fn fake_failing_writes(lines: &[&str], writes: usize) -> Console {
        let lines = lines.iter().map(|line| line.to_string()).collect();
        Console { input: Input::Script { lines, read_fails: false, writes_left: Some(writes), fails_when_out: false }, written: Vec::new() }
    }

    /// A fake console that reads the given lines, after which every read fails, instead of reporting that its input
    /// is closed: Y's `fake console(input: …, failing_after_input: true)`.
    pub fn fake_failing_reads(lines: &[&str]) -> Console {
        let lines = lines.iter().map(|line| line.to_string()).collect();
        Console { input: Input::Script { lines, read_fails: false, writes_left: None, fails_when_out: true }, written: Vec::new() }
    }

    /// Spends one of a fake console's writes, or fails when none is left.
    fn spend_write(&mut self) -> Result<(), ConsoleError> {
        if let Input::Script { writes_left: Some(left), .. } = &mut self.input {
            if *left == 0 {
                return Err(ConsoleError::fake_write_failure());
            }
            *left -= 1;
        }
        Ok(())
    }

    /// Writes a line.
    pub fn write_line(&mut self, text: &str) -> Result<(), ConsoleError> {
        self.spend_write()?;
        match self.input {
            Input::Terminal => writeln!(std::io::stdout(), "{text}")?,
            Input::Script { .. } => self.written.push(text.to_string()),
        }
        Ok(())
    }

    /// Writes text with no line ending, as a prompt.
    pub fn write(&mut self, text: &str) -> Result<(), ConsoleError> {
        self.spend_write()?;
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
            Input::Script { lines, fails_when_out, .. } => match lines.pop_front() {
                None if *fails_when_out => Err(ConsoleError::fake_read_failure()),
                line => Ok(line),
            },
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

    /// The list in an order drawn at random: Y's `random.shuffled(items)`.
    pub fn shuffled<T: Clone>(&mut self, items: &[T]) -> Vec<T> {
        let mut out = items.to_vec();
        for last in (1..out.len()).rev() {
            let other = self.index_below(last + 1);
            out.swap(last, other);
        }
        out
    }

    /// One element of the list, drawn at random: Y's `random.one_of(items)`. An empty list is a fault.
    pub fn one_of<T: Clone>(&mut self, items: &[T]) -> T {
        require(!items.is_empty(), "y_runtime::Random::one_of#not_empty");
        items[self.index_below(items.len())].clone()
    }

    /// `count` elements of the list, each drawn at most once, in the order drawn: Y's `random.chosen(from:, count:)`.
    /// A count larger than the list is a fault.
    pub fn chosen<T: Clone>(&mut self, items: &[T], count: u32) -> Vec<T> {
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        require(count <= items.len(), "y_runtime::Random::chosen#enough");
        self.shuffled(items).into_iter().take(count).collect()
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

/// Y's `within d or fail:` (Y § 3.8): an absolute deadline shared by every wait inside the scope. It is read from the
/// runtime's monotonic time, not from a `clock` capability.
#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    at: Instant,
}

impl Deadline {
    /// The deadline `bound` from now.
    pub fn after(bound: Duration) -> Deadline {
        Deadline { at: Instant::now() + bound }
    }

    /// The time left before the deadline, or `Expired` when it has passed.
    pub fn remaining(&self) -> Result<Duration, Expired> {
        self.at.checked_duration_since(Instant::now()).filter(|left| !left.is_zero()).ok_or(Expired { _private: () })
    }
}

/// Y's `expired`: the failure of a wait whose deadline passed (Y § 3.8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expired {
    _private: (),
}

impl Expired {
    /// An expiry, for a fake capability whose waits always expire.
    pub fn fake() -> Expired {
        Expired { _private: () }
    }
}

/// How a call that waits inside `within` fails: with its own failure, or because the deadline passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Within<E> {
    Failed(E),
    Expired(Expired),
}

/// Y's operations on `duration` that Rust's `Duration` does not have in the same form.
pub mod time {
    use std::time::Duration;

    /// The whole seconds in a duration, as a `uint32`: Y's `duration.whole_seconds()`. The part of a second left over
    /// is dropped, and a duration too long for a `uint32` gives its largest value.
    pub fn whole_seconds(span: Duration) -> u32 {
        u32::try_from(span.as_secs()).unwrap_or(u32::MAX)
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

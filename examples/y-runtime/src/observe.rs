//! `log` and `metric` (Y § 3.7, "Observability is the one ambient exception"): write-only statements that need no
//! capability. A stand-in for Y's observability: each entry goes to the process's standard error, with the stable
//! identity of the construct that wrote it, as one line:
//!
//! ```text
//! y log warning orders::place: the hold 7 expired
//! y metric orders::place orders_placed 1
//! ```
//!
//! Inside an atomic region, entries are buffered, written if the run commits and dropped if it does not, so a run the
//! runtime discards or repeats leaves none ([`crate::shared`] calls [`begin_run`], [`commit_run`] and
//! [`discard_run`]). The entries a thread wrote are also kept, for the runtime's own tests ([`written`]); nothing in a Y
//! program can read them, as Y requires.
use std::cell::RefCell;

thread_local! {
    /// The entries of the atomic run under way on this thread, if one is: written when it commits.
    static BUFFER: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    /// The entries this thread wrote.
    static WRITTEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn emit(entry: String) {
    let buffered = BUFFER.with(|b| match b.borrow_mut().as_mut() {
        Some(v) => {
            v.push(entry.clone());
            true
        }
        None => false,
    });
    if !buffered {
        write(entry);
    }
}

fn write(entry: String) {
    eprintln!("{entry}");
    WRITTEN.with(|w| w.borrow_mut().push(entry));
}

/// `log level "message"` in the construct `identity`.
pub fn log(level: &str, identity: &str, message: &str) {
    emit(format!("y log {level} {identity}: {message}"));
}

/// `metric name = value` in the construct `identity`.
pub fn metric(identity: &str, name: &str, value: impl std::fmt::Display) {
    emit(format!("y metric {identity} {name} {value}"));
}

/// An atomic run starts: its entries are held until it ends.
pub fn begin_run() {
    BUFFER.with(|b| *b.borrow_mut() = Some(Vec::new()));
}

/// The run commits: its entries are written.
pub fn commit_run() {
    let held = BUFFER.with(|b| b.borrow_mut().take()).unwrap_or_default();
    for e in held {
        write(e);
    }
}

/// The run is discarded, to be run again or abandoned: its entries are dropped.
pub fn discard_run() {
    BUFFER.with(|b| *b.borrow_mut() = None);
}

/// The entries this thread has written, for the runtime's tests.
pub fn written() -> Vec<String> {
    WRITTEN.with(|w| w.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::{atomic, Aborted, Shared, Step};

    #[test]
    fn an_entry_outside_a_transaction_is_written_at_once() {
        log("info", "m::f", "hello");
        assert!(written().contains(&"y log info m::f: hello".to_string()));
    }

    #[test]
    fn a_discarded_run_writes_nothing_and_a_committed_one_writes_once() {
        let n = Shared::new(0u32);
        crate::faults::force(crate::faults::Kind::Conflict, "m::f#count", 2);
        let r: Result<(), Aborted<()>> = atomic("m::f#count", None, |tx| {
            tx.set(&n, tx.get(&n) + 1);
            metric("m::f", "counted", 1);
            Ok(Step::Done(()))
        });
        assert_eq!(r, Ok(()));
        assert_eq!(n.snapshot(), 1, "the discarded runs left no trace");
        assert_eq!(written().iter().filter(|e| e.as_str() == "y metric m::f counted 1").count(), 1);
        assert_eq!(crate::faults::unmet(), 0);
    }
}

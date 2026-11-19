//! A test's `faults:` (Y § 3.14): what it forces where the code under test runs. A stand-in, kept per thread, for the
//! fault injection of Y's runtime: a test names a region by its address, `module::function#region`, and the region's
//! runner consults it.
//!
//! - `conflict at f#txn, times: n`: the next `n` runs of the atomic region that would commit are discarded and run
//!   again, as a conflicting commit would make them ([`crate::shared`]).
//! - `expire at f#txn`: the atomic region's deadline passes before it runs.
//!
//! `crash after commit` and `cancel at f#task` are not kept by this stand-in; the Y compiler refuses a test that forces
//! them, naming the fault.
use std::cell::RefCell;

/// What a test forces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Conflict,
    Expire,
}

thread_local! {
    /// What is still to be forced: each fault, where, and how many times more.
    static FORCED: RefCell<Vec<(Kind, String, u32)>> = const { RefCell::new(Vec::new()) };
}

/// Forces a fault at a region `times` times, for the test running on this thread.
pub fn force(kind: Kind, at: &str, times: u32) {
    FORCED.with(|f| f.borrow_mut().push((kind, at.to_string(), times)));
}

/// Whether a fault is forced at a region now: once taken, it is forced one time fewer.
pub fn take(kind: Kind, at: &str) -> bool {
    FORCED.with(|f| {
        let mut f = f.borrow_mut();
        match f.iter_mut().find(|(k, a, n)| *k == kind && a == at && *n > 0) {
            Some(entry) => {
                entry.2 -= 1;
                true
            }
            None => false,
        }
    })
}

/// How many forced faults the code under test has not yet met: what a test may check after its `when`.
pub fn unmet() -> u32 {
    FORCED.with(|f| f.borrow().iter().map(|e| e.2).sum())
}

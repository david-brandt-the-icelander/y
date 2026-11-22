//! A test's `faults:` (Y § 3.14): what it forces where the code under test runs. A stand-in, kept per thread, for the
//! fault injection of Y's runtime: a test names a region by its address, `module::function#region`, and the region's
//! runner consults it.
//!
//! - `conflict at f#txn, times: n`: the next `n` runs of the atomic region that would commit are discarded and run
//!   again, as a conflicting commit would make them ([`crate::shared`]).
//! - `expire at f#txn`: the atomic region's deadline passes before it runs.
//! - `crash after commit f#txn`: the program crashes once the atomic region has committed, and starts again. The crash
//!   is a [`Crashed`] panic, which nothing but [`restarting`] catches; the test's `when` runs inside it, and runs again
//!   after the crash, as a restarted program would. What committed stays committed, and what the journal recorded stays
//!   recorded (`crate::journal`): a workflow resumes, and does not make its recorded steps again.
//! - `cancel at f#task`: the task is cancelled before it starts. It does not run and is not restarted, and its siblings
//!   and its region go on: a cancellation is not a failure that the region's supervision answers.
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

/// What a test forces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Conflict,
    Expire,
    Crash,
    Cancel,
}

/// A crash a test forced: where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crashed {
    pub at: String,
}

/// Crashes the program, as a test forced: a panic that only [`restarting`] catches.
pub fn crash(at: &str) -> ! {
    std::panic::panic_any(Crashed { at: at.to_string() })
}

/// Runs `run`, and runs it again each time it crashes, as a program that crashed starts again.
pub fn restarting<T>(mut run: impl FnMut() -> T) -> T {
    loop {
        match catch_unwind(AssertUnwindSafe(&mut run)) {
            Ok(v) => return v,
            Err(p) if p.is::<Crashed>() => RESTARTS.with(|r| r.set(r.get() + 1)),
            Err(p) => resume_unwind(p),
        }
    }
}

/// How many times the program restarted after a crash, on this thread.
pub fn restarts() -> u32 {
    RESTARTS.with(Cell::get)
}

thread_local! {
    /// What is still to be forced: each fault, where, and how many times more.
    static FORCED: RefCell<Forced> = RefCell::new(Forced::default());
    static RESTARTS: Cell<u32> = const { Cell::new(0) };
}

/// What a test forces, each fault with where and how many times more: kept per thread, and shared with the workers
/// that run part of the test's code on threads of their own ([`home`], [`adopt`]).
pub type Forced = Arc<Mutex<Vec<(Kind, String, u32)>>>;

fn forced<T>(f: impl FnOnce(&mut Vec<(Kind, String, u32)>) -> T) -> T {
    FORCED.with(|table| f(&mut table.borrow().lock().unwrap_or_else(|p| p.into_inner())))
}

/// This thread's table of forced faults, to hand to a worker.
pub fn home() -> Forced {
    FORCED.with(|f| Arc::clone(&f.borrow()))
}

/// Makes this thread, a worker's, force what the table of the thread that started it forces.
pub fn adopt(table: Forced) {
    FORCED.with(|f| *f.borrow_mut() = table);
}

/// Forces a fault at a region `times` times, for the test running on this thread and its workers.
pub fn force(kind: Kind, at: &str, times: u32) {
    forced(|f| f.push((kind, at.to_string(), times)));
}

/// Whether a fault is forced at a region now: once taken, it is forced one time fewer.
pub fn take(kind: Kind, at: &str) -> bool {
    forced(|f| {
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
    forced(|f| f.iter().map(|e| e.2).sum())
}

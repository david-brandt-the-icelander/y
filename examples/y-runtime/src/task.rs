//! Y's tasks and the execution constraints on their regions (Y § 3.8): structured scopes, supervision, detached work,
//! cancellation and `uncancellable`, kept with threads.
//!
//! - [`supervised`] is a region with tasks and a `supervise(…)` policy: each task runs on its own thread, the region does
//!   not end until every task has, a task that fails is restarted as the policy allows, and a failure past the policy
//!   cancels the other tasks and fails the region.
//! - [`detach`] is a region that is `detach because "…", within d`: its work runs on a thread that may outlive the
//!   scope, and is waited for, when anything waits, no longer than `d`.
//! - [`Cancellation`] is how cancellation reaches a task: at the points where it checks, as a failure. A region that is
//!   `uncancellable, within d` holds it off until the region ends ([`Cancellation::held`]).
use super::{Deadline, Expired};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// Y's `restart_policy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    /// A plain region: the first failure fails it.
    Never,
    /// `restart_policy.each_failed`: a task that fails is started again from its initial state.
    EachFailed,
}

/// Y's `supervise(restart: …, at_most: n)`: how many restarts the region allows in all, before a failure fails it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Supervision {
    pub restart: Restart,
    pub at_most: u32,
}

/// Y's cancellation of a task: it arrives as a failure where the task checks for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cancelled;

thread_local! {
    /// How many `uncancellable` regions the current thread is inside.
    static HELD: Cell<u32> = const { Cell::new(0) };
}

/// What a task checks to learn that it is cancelled.
#[derive(Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>,
}

impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels every task that holds this.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// A point where cancellation may arrive: `Cancelled` when it has, unless an `uncancellable` region holds it off.
    pub fn check(&self) -> Result<(), Cancelled> {
        let held = HELD.with(Cell::get) > 0;
        if self.flag.load(Ordering::SeqCst) && !held {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }

    /// A region that is `uncancellable, within d`: cancellation is held off while `body` runs, and the region must end
    /// by the deadline; one that does not is a fault, since the hold must be bounded.
    pub fn held<R>(&self, name: &str, within: Duration, body: impl FnOnce() -> R) -> R {
        let deadline = Deadline::after(within);
        HELD.with(|h| h.set(h.get() + 1));
        let r = body();
        HELD.with(|h| h.set(h.get() - 1));
        if deadline.remaining().is_err() {
            super::fault(name, "an uncancellable region outlived its bound");
        }
        r
    }
}

/// The work of one task: run with the region's cancellation, it succeeds or fails with `E`.
pub type TaskBody<'a, E> = Box<dyn Fn(&Cancellation) -> Result<(), E> + Send + Sync + 'a>;

/// How a supervised region failed: the failure that went past the policy, and how many restarts it allowed first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed<E> {
    pub failure: E,
    pub restarts: u32,
}

/// A region with tasks under a `supervise(…)` policy. Every task runs on a thread of its own; the region returns once all
/// have ended. A task's failure restarts it while the policy allows; the first failure past it cancels the others and
/// is the region's failure.
pub fn supervised<E: Send + Clone>(policy: Supervision, tasks: Vec<TaskBody<'_, E>>) -> Result<u32, Failed<E>> {
    let cancellation = Cancellation::new();
    let restarts = Mutex::new(0u32);
    let first: Mutex<Option<E>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for task in &tasks {
            let (cancellation, restarts, first) = (&cancellation, &restarts, &first);
            scope.spawn(move || loop {
                match task(cancellation) {
                    Ok(()) => break,
                    Err(e) => {
                        let mut n = restarts.lock().unwrap_or_else(|p| p.into_inner());
                        if policy.restart == Restart::EachFailed && *n < policy.at_most && cancellation.check().is_ok() {
                            *n += 1;
                            continue;
                        }
                        drop(n);
                        let mut f = first.lock().unwrap_or_else(|p| p.into_inner());
                        if f.is_none() {
                            *f = Some(e);
                        }
                        cancellation.cancel();
                        break;
                    }
                }
            });
        }
    });
    let restarts = restarts.into_inner().unwrap_or_else(|p| p.into_inner());
    match first.into_inner().unwrap_or_else(|p| p.into_inner()) {
        None => Ok(restarts),
        Some(failure) => Err(Failed { failure, restarts }),
    }
}

/// Detached work: a region that is `detach because "…", within d`.
pub struct Detached<R> {
    done: mpsc::Receiver<R>,
    deadline: Deadline,
}

/// Starts detached work on a thread of its own, which may outlive the scope that started it.
pub fn detach<R: Send + 'static>(within: Duration, body: impl FnOnce() -> R + Send + 'static) -> Detached<R> {
    let (send, done) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(body());
    });
    Detached { done, deadline: Deadline::after(within) }
}

impl<R> Detached<R> {
    /// Waits for the work to end, no longer than its bound: `expired` when it did not end in time.
    pub fn wait(self) -> Result<R, Expired> {
        let left = self.deadline.remaining()?;
        self.done.recv_timeout(left).map_err(|_| Expired::fake())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn a_region_waits_for_every_task() {
        let done = AtomicU32::new(0);
        let tasks: Vec<TaskBody<'_, ()>> = (0..4)
            .map(|_| -> TaskBody<'_, ()> {
                Box::new(|_| {
                    done.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
            })
            .collect();
        assert_eq!(supervised(Supervision { restart: Restart::Never, at_most: 0 }, tasks), Ok(0));
        assert_eq!(done.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn a_failed_task_is_restarted_as_the_policy_allows() {
        let runs = AtomicU32::new(0);
        let flaky: TaskBody<'_, &str> = Box::new(|_| if runs.fetch_add(1, Ordering::SeqCst) < 2 { Err("dropped") } else { Ok(()) });
        let policy = Supervision { restart: Restart::EachFailed, at_most: 2 };
        assert_eq!(supervised(policy, vec![flaky]), Ok(2));
        let always: TaskBody<'_, &str> = Box::new(|_| Err("broken"));
        assert_eq!(supervised(policy, vec![always]), Err(Failed { failure: "broken", restarts: 2 }));
    }

    #[test]
    fn a_failure_past_the_policy_cancels_the_others() {
        let failing: TaskBody<'_, &str> = Box::new(|_| Err("broken"));
        let patient: TaskBody<'_, &str> = Box::new(|c| loop {
            if c.check().is_err() {
                return Err("cancelled");
            }
            std::thread::sleep(Duration::from_millis(1));
        });
        let r = supervised(Supervision { restart: Restart::Never, at_most: 0 }, vec![failing, patient]);
        assert_eq!(r.map_err(|f| f.failure), Err("broken"));
    }

    #[test]
    fn an_uncancellable_region_holds_cancellation_off() {
        let c = Cancellation::new();
        c.cancel();
        assert_eq!(c.held("flush", Duration::from_secs(1), || c.check()), Ok(()));
        assert_eq!(c.check(), Err(Cancelled));
    }

    #[test]
    fn detached_work_is_waited_for_within_its_bound() {
        assert_eq!(detach(Duration::from_secs(5), || 6 * 7).wait(), Ok(42));
        let slow = detach(Duration::from_millis(20), || std::thread::sleep(Duration::from_millis(500)));
        assert!(slow.wait().is_err());
    }
}

//! Y's shared state and atomic regions (Y § 3.8, § 3.9), kept the way the first backend keeps them: with one lock.
//!
//! - A [`Shared<T>`] is read and written only inside an atomic region, through its [`Tx`].
//! - Every atomic region takes one global lock for its whole run, so regions are serializable by construction and none
//!   is ever re-run because of a conflict.
//! - Writes are buffered in the [`Tx`] and applied when the region ends; a failure, or a [`Step::Retry`], discards them,
//!   so a failure leaves no trace.
//! - `waits until p` and `retry` wait on one condition variable, which every commit notifies, until the region's
//!   deadline. A wait that the deadline ends is the failure `expired`.
//!
//! The lowering of a region `region reserve is atomic, within 5s or fail:` is a call of [`atomic`] with the region's
//! name, its deadline and a closure for its body. Multi-version concurrency control would keep the same constraints with
//! more concurrency; it is a later backend's choice.
use super::{Deadline, Expired};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// The one lock, and the version every commit increments.
static LOCK: Mutex<u64> = Mutex::new(0);
/// Notified by every commit, for the regions that wait.
static CHANGED: Condvar = Condvar::new();

/// Y's `shared<value_type=t>`: mutable state reachable from more than one task, read and written only inside an
/// atomic region.
pub struct Shared<T> {
    value: Arc<Mutex<T>>,
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared { value: Arc::clone(&self.value) }
    }
}

impl<T: Clone + Send + 'static> Shared<T> {
    /// Y's `shared.new(initial)`.
    pub fn new(initial: T) -> Self {
        Shared { value: Arc::new(Mutex::new(initial)) }
    }

    /// Y's `snapshot()`: an immutable, consistent copy, taken as a read-only transaction.
    pub fn snapshot(&self) -> T {
        let _held = hold();
        self.value.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn key(&self) -> usize {
        Arc::as_ptr(&self.value) as *const () as usize
    }
}

/// What a run of an atomic region's body decided.
pub enum Step<R> {
    /// The region is done, with this outcome; its writes commit.
    Done(R),
    /// Y's `retry`: abandon this run, discard its writes, and wait until something changes.
    Retry,
}

/// Writes buffered by a run, by the address of the shared state they are for.
type Pending = HashMap<usize, Box<dyn Any + Send>>;
/// Applies one buffered write when the region commits.
type Apply = Box<dyn FnOnce(&mut Pending) + Send>;
/// One alternative of an `orelse`: a run of an atomic region's body.
pub type Alternative<'a, R, E> = &'a mut dyn FnMut(&mut Tx) -> Result<Step<R>, E>;

/// The transaction of one run of an atomic region: its reads see its own writes, and its writes wait for the end.
pub struct Tx {
    pending: Pending,
    apply: Vec<Apply>,
}

impl Tx {
    fn new() -> Tx {
        Tx { pending: HashMap::new(), apply: Vec::new() }
    }

    /// The value of a shared state, as this run sees it: Y reads a `shared` inside `atomic` as a `mutable` value.
    pub fn get<T: Clone + Send + 'static>(&self, shared: &Shared<T>) -> T {
        match self.pending.get(&shared.key()).and_then(|v| v.downcast_ref::<T>()) {
            Some(written) => written.clone(),
            None => shared.value.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        }
    }

    /// Writes a shared state; the write becomes visible to others when the region ends.
    pub fn set<T: Clone + Send + 'static>(&mut self, shared: &Shared<T>, value: T) {
        let key = shared.key();
        if !self.pending.contains_key(&key) {
            let target = Arc::clone(&shared.value);
            self.apply.push(Box::new(move |pending| {
                if let Some(v) = pending.remove(&key).and_then(|v| v.downcast::<T>().ok()) {
                    *target.lock().unwrap_or_else(|e| e.into_inner()) = *v;
                }
            }));
        }
        self.pending.insert(key, Box::new(value));
    }

    fn commit(mut self) {
        for apply in self.apply.drain(..) {
            apply(&mut self.pending);
        }
    }
}

fn hold() -> MutexGuard<'static, u64> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Waits on the one condition variable until a commit, or the deadline. The lock is given up while waiting.
fn wait(held: MutexGuard<'static, u64>, deadline: Option<&Deadline>) -> Result<MutexGuard<'static, u64>, Expired> {
    match deadline {
        None => Ok(CHANGED.wait(held).unwrap_or_else(|e| e.into_inner())),
        Some(d) => {
            let left = d.remaining()?;
            let (held, _) = CHANGED.wait_timeout(held, left.min(Duration::from_secs(3600))).unwrap_or_else(|e| e.into_inner());
            Ok(held)
        }
    }
}

/// How an atomic region ends that does not commit: with its body's failure, or because its deadline passed while it
/// waited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Aborted<E> {
    Failed(E),
    Expired(Expired),
}

/// Runs an atomic region: `region name is atomic` with an optional deadline (`within`). The body runs under the one
/// lock with a fresh [`Tx`]; `Ok(Step::Done(r))` commits and returns `r`; `Ok(Step::Retry)` discards the run and waits
/// for a commit; `Err(e)` discards the run and fails with `e`. Waiting past the deadline fails with `expired`.
pub fn atomic<R, E>(
    _name: &str,
    deadline: Option<&Deadline>,
    mut body: impl FnMut(&mut Tx) -> Result<Step<R>, E>,
) -> Result<R, Aborted<E>> {
    orelse(_name, deadline, vec![&mut body])
}

/// `waits until name = (p)` on an atomic region: the body runs only once `ready` holds, and waits for a commit until
/// then. The condition reads through the same [`Tx`], so its facts hold in the body.
pub fn atomic_when<R, E>(
    name: &str,
    deadline: Option<&Deadline>,
    ready: impl Fn(&Tx) -> bool,
    mut body: impl FnMut(&mut Tx) -> Result<R, E>,
) -> Result<R, Aborted<E>> {
    atomic(name, deadline, |tx| if ready(tx) { body(tx).map(Step::Done) } else { Ok(Step::Retry) })
}

/// Y's `orelse`: the alternatives, left to right, under one hold of the lock. The first that does not retry decides;
/// when every one retries, the region waits for a commit and tries them all again.
pub fn orelse<R, E>(
    _name: &str,
    deadline: Option<&Deadline>,
    mut alternatives: Vec<Alternative<'_, R, E>>,
) -> Result<R, Aborted<E>> {
    let mut held = hold();
    loop {
        for alternative in alternatives.iter_mut() {
            let mut tx = Tx::new();
            match alternative(&mut tx) {
                Ok(Step::Done(r)) => {
                    let changed = !tx.apply.is_empty();
                    tx.commit();
                    if changed {
                        *held += 1;
                        CHANGED.notify_all();
                    }
                    return Ok(r);
                }
                Ok(Step::Retry) => {}
                Err(e) => return Err(Aborted::Failed(e)),
            }
        }
        held = wait(held, deadline).map_err(Aborted::Expired)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn a_region_commits_its_writes_together() {
        let stock = Shared::new(3u32);
        let sold = Shared::new(0u32);
        let r: Result<u32, Aborted<()>> = atomic("sell", None, |tx| {
            let left = tx.get(&stock);
            tx.set(&stock, left - 1);
            tx.set(&sold, tx.get(&sold) + 1);
            assert_eq!(tx.get(&stock), 2, "a run reads its own writes");
            Ok(Step::Done(left - 1))
        });
        assert_eq!(r, Ok(2));
        assert_eq!((stock.snapshot(), sold.snapshot()), (2, 1));
    }

    #[test]
    fn a_failure_leaves_no_trace() {
        let stock = Shared::new(3u32);
        let r: Result<(), Aborted<&str>> = atomic("sell", None, |tx| {
            tx.set(&stock, 0);
            Err("declined")
        });
        assert_eq!(r, Err(Aborted::Failed("declined")));
        assert_eq!(stock.snapshot(), 3);
    }

    #[test]
    fn a_wait_ends_when_another_region_commits() {
        let stock = Shared::new(0u32);
        let filler = stock.clone();
        let waiter = thread::spawn(move || {
            let d = Deadline::after(Duration::from_secs(5));
            atomic_when("take", Some(&d), |tx| tx.get(&stock) > 0, |tx| -> Result<u32, ()> {
                let n = tx.get(&stock);
                tx.set(&stock, n - 1);
                Ok(n)
            })
        });
        thread::sleep(Duration::from_millis(50));
        let _: Result<(), Aborted<()>> = atomic("fill", None, |tx| {
            tx.set(&filler, 2);
            Ok(Step::Done(()))
        });
        assert_eq!(waiter.join().unwrap(), Ok(2));
        assert_eq!(filler.snapshot(), 1);
    }

    #[test]
    fn a_wait_past_the_deadline_expires() {
        let stock = Shared::new(0u32);
        let d = Deadline::after(Duration::from_millis(30));
        let r = atomic_when("take", Some(&d), |tx| tx.get(&stock) > 0, |_| -> Result<(), ()> { Ok(()) });
        assert!(matches!(r, Err(Aborted::Expired(_))));
    }

    #[test]
    fn orelse_takes_the_first_alternative_that_does_not_retry() {
        let pending = Shared::new(Vec::<u32>::new());
        let retried = Shared::new(vec![7u32]);
        let mut from_pending = |tx: &mut Tx| -> Result<Step<u32>, ()> {
            let mut p = tx.get(&pending);
            Ok(p.pop().map_or(Step::Retry, |j| {
                tx.set(&pending, p.clone());
                Step::Done(j)
            }))
        };
        let mut from_retried = |tx: &mut Tx| -> Result<Step<u32>, ()> {
            let mut r = tx.get(&retried);
            Ok(r.pop().map_or(Step::Retry, |j| {
                tx.set(&retried, r.clone());
                Step::Done(j)
            }))
        };
        let got = orelse("take_job", None, vec![&mut from_pending, &mut from_retried]);
        assert_eq!(got, Ok(7));
        assert!(retried.snapshot().is_empty());
    }

    #[test]
    fn concurrent_regions_are_serializable() {
        let counter = Shared::new(0u64);
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let c = counter.clone();
                thread::spawn(move || {
                    for _ in 0..500 {
                        let _: Result<(), Aborted<()>> = atomic("add", None, |tx| {
                            let n = tx.get(&c);
                            tx.set(&c, n + 1);
                            Ok(Step::Done(()))
                        });
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(counter.snapshot(), 4000);
    }
}

//! Y's shared state and atomic regions (Y § 3.8, § 3.9).
//!
//! - A [`Shared<T>`] is read and written only inside an atomic region, through its [`Tx`].
//! - Writes are buffered in the [`Tx`] and applied when the region ends; a failure, or a [`Step::Retry`], discards them,
//!   so a failure leaves no trace (`on failure unchanged`).
//! - `waits until p` and `retry` wait on one condition variable, which every commit notifies, until the region's
//!   deadline. A wait that the deadline ends is the failure `expired`.
//!
//! Two engines keep the same constraints, and one is chosen for the whole program, before its first region runs: by
//! the root capability, `System::root` (from the environment's `Y_ATOMIC`) or `System::use_engine`, or else by the
//! crate's feature `optimistic` (`cargo test --features y-runtime/optimistic`). Once a region has run, the engine is
//! fixed: choosing another is a fault. An atomic region reads the choice, and
//! not the environment: running one is `shared`, not `effects` (Y § 3.19).
//!
//! - **One lock** (the default, the first backend's): every atomic region takes one global lock for its whole run, so
//!   regions are serializable by construction and none is ever re-run.
//! - **Optimistic** (`Y_ATOMIC=optimistic`, or the feature `optimistic`): a region runs without the lock against the versions committed when it
//!   started, and takes the lock only to commit. A commit checks the versions the run depended on: every state it read
//!   or wrote, for `isolation serializable`; only what it wrote, first committer wins, for `isolation snapshot`, which
//!   may suffer write skew. A run that a commit invalidated is run again. This is what `may run again` permits, and why
//!   an atomic region may hold no external effect. A run that retries waits until a commit changes something it read.
//!
//! The lowering of a region `region reserve is atomic, within 5s or fail:` is a call of [`atomic`] with the region's
//! name, its deadline and a closure for its body; `isolation snapshot` lowers to [`atomic_with`] and
//! [`Isolation::Snapshot`].
use super::{Deadline, Expired};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// The one lock, and the version every commit increments.
static LOCK: Mutex<u64> = Mutex::new(0);
/// Notified by every commit, for the regions that wait.
static CHANGED: Condvar = Condvar::new();
/// How many runs of atomic regions a commit invalidated, so that they ran again.
static RERUNS: AtomicU64 = AtomicU64::new(0);

/// What an atomic region observes (Y § 3.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Isolation {
    /// As if no other region ran while it did. The default.
    Serializable,
    /// One consistent snapshot; a concurrent region's write to what it only read does not stop it (write skew).
    Snapshot,
}

/// How the program keeps its atomic regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    OneLock,
    Optimistic,
}

/// The engine chosen for the program (`set_engine`): `NONE` for none yet, `ONE_LOCK`, or `OPTIMISTIC`; with `RAN` once
/// an atomic region has run, after which the engine is fixed. One atomic value holds both, so that a choice and the
/// first region cannot pass each other.
static ENGINE: AtomicU8 = AtomicU8::new(NONE);
const NONE: u8 = 0;
const ONE_LOCK: u8 = 1;
const OPTIMISTIC: u8 = 2;
const RAN: u8 = 4;

/// The engine a value of `ENGINE` stands for: the one chosen, or else the crate's feature `optimistic`'s, or else one
/// lock.
fn engine_of(v: u8) -> Engine {
    match v & !RAN {
        ONE_LOCK => Engine::OneLock,
        OPTIMISTIC => Engine::Optimistic,
        _ if cfg!(feature = "optimistic") => Engine::Optimistic,
        _ => Engine::OneLock,
    }
}

fn code(engine: Engine) -> u8 {
    if engine == Engine::Optimistic { OPTIMISTIC } else { ONE_LOCK }
}

/// The engine the program's atomic regions run with: the one chosen through the root capability (`System::root`,
/// `System::use_engine`), or else the crate's feature `optimistic`'s, or else one lock.
pub fn engine() -> Engine {
    engine_of(ENGINE.load(Ordering::SeqCst))
}

/// The engine for a region about to run: from now on, the program's engine is fixed.
fn engine_for_a_run() -> Engine {
    run_with(&ENGINE)
}

fn run_with(cell: &AtomicU8) -> Engine {
    engine_of(cell.fetch_or(RAN, Ordering::SeqCst))
}

/// Chooses the program's engine. Only the root capability chooses it (`crate::System`), and only before the first
/// atomic region runs: then a region ran with another engine, and switching would mix the two (their locking and their
/// validation do not see each other's commits), so a choice of another engine fails. Choosing the engine already in
/// use does not.
pub(crate) fn set_engine(engine: Engine) -> Result<(), String> {
    choose(&ENGINE, engine)
}

fn choose(cell: &AtomicU8, engine: Engine) -> Result<(), String> {
    let mut v = cell.load(Ordering::SeqCst);
    loop {
        if v & RAN != 0 {
            return if engine_of(v) == engine { Ok(()) } else { Err(format!("an atomic region already ran with the engine {:?}, and the program's engine is fixed once one has", engine_of(v))) };
        }
        match cell.compare_exchange(v, code(engine), Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(()),
            Err(now) => v = now,
        }
    }
}

thread_local! {
    /// What `after_next_read` asked to run.
    static AFTER_READ: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

/// For tests of the optimistic engine: runs `hook` once, on this thread, right after the next read of an optimistic run,
/// as another task's commit would land between a region's read and its commit. It makes a re-run happen when a test
/// wants one, instead of when the scheduler happens to give one.
pub fn after_next_read(hook: impl FnOnce() + 'static) {
    AFTER_READ.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
}

/// How many runs of atomic regions have been run again since the program started.
pub fn reruns() -> u64 {
    RERUNS.load(Ordering::SeqCst)
}

/// A shared state's value and the version of the commit that wrote it.
struct Slot<T> {
    version: u64,
    value: T,
}

/// Y's `shared<value_type=t>`: mutable state reachable from more than one task, read and written only inside an
/// atomic region.
pub struct Shared<T> {
    value: Arc<Mutex<Slot<T>>>,
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared { value: Arc::clone(&self.value) }
    }
}

impl<T: Clone + Send + 'static> Shared<T> {
    /// Y's `shared.new(initial)`.
    pub fn new(initial: T) -> Self {
        Shared { value: Arc::new(Mutex::new(Slot { version: 0, value: initial })) }
    }

    /// Y's `snapshot()`: an immutable, consistent copy, taken as a read-only transaction.
    pub fn snapshot(&self) -> T {
        let _held = hold();
        self.slot().value.clone()
    }

    fn slot(&self) -> MutexGuard<'_, Slot<T>> {
        self.value.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn key(&self) -> usize {
        Arc::as_ptr(&self.value) as *const () as usize
    }

    fn version_of(&self) -> Arc<dyn Fn() -> u64 + Send + Sync> {
        let v = Arc::clone(&self.value);
        Arc::new(move || v.lock().unwrap_or_else(|e| e.into_inner()).version)
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
/// Applies one buffered write when the region commits, with the commit's version.
type Apply = Box<dyn FnOnce(&mut Pending, u64) + Send>;
/// The version of one shared state now.
type VersionOf = Arc<dyn Fn() -> u64 + Send + Sync>;
/// One alternative of an `orelse`: a run of an atomic region's body.
pub type Alternative<'a, R, E> = &'a mut dyn FnMut(&mut Tx) -> Result<Step<R>, E>;

/// The transaction of one run of an atomic region: its reads see its own writes, and its writes wait for the end.
pub struct Tx {
    pending: Pending,
    apply: Vec<Apply>,
    /// Optimistic runs: the version the run reads at, what it read and wrote, and whether a read saw a later commit.
    stamp: Option<u64>,
    read: RefCell<HashMap<usize, VersionOf>>,
    written: HashMap<usize, VersionOf>,
    stale: Cell<bool>,
}

impl Tx {
    fn new(stamp: Option<u64>) -> Tx {
        Tx { pending: HashMap::new(), apply: Vec::new(), stamp, read: RefCell::new(HashMap::new()), written: HashMap::new(), stale: Cell::new(false) }
    }

    /// The value of a shared state, as this run sees it: Y reads a `shared` inside `atomic` as a `mutable` value.
    pub fn get<T: Clone + Send + 'static>(&self, shared: &Shared<T>) -> T {
        if let Some(written) = self.pending.get(&shared.key()).and_then(|v| v.downcast_ref::<T>()) {
            return written.clone();
        }
        let value = {
            let slot = shared.slot();
            if let Some(stamp) = self.stamp {
                // a value committed after the run started is not in its snapshot: the run is run again
                if slot.version > stamp {
                    self.stale.set(true);
                }
                self.read.borrow_mut().entry(shared.key()).or_insert_with(|| shared.version_of());
            }
            slot.value.clone()
        };
        if self.stamp.is_some() {
            if let Some(hook) = AFTER_READ.with(|h| h.borrow_mut().take()) {
                hook();
            }
        }
        value
    }

    /// Writes a shared state; the write becomes visible to others when the region ends.
    pub fn set<T: Clone + Send + 'static>(&mut self, shared: &Shared<T>, value: T) {
        let key = shared.key();
        if !self.pending.contains_key(&key) {
            let target = Arc::clone(&shared.value);
            self.apply.push(Box::new(move |pending, version| {
                if let Some(v) = pending.remove(&key).and_then(|v| v.downcast::<T>().ok()) {
                    *target.lock().unwrap_or_else(|e| e.into_inner()) = Slot { version, value: *v };
                }
            }));
            if self.stamp.is_some() {
                self.written.insert(key, shared.version_of());
            }
        }
        self.pending.insert(key, Box::new(value));
    }

    /// Whether an optimistic run may commit, under the lock: nothing it depended on was committed since it started.
    fn valid(&self, isolation: Isolation) -> bool {
        let Some(stamp) = self.stamp else { return true };
        let unchanged = |v: &VersionOf| v() <= stamp;
        !self.stale.get()
            && self.written.values().all(unchanged)
            && (isolation == Isolation::Snapshot || self.read.borrow().values().all(unchanged))
    }

    fn commit(mut self, version: u64) {
        for apply in self.apply.drain(..) {
            apply(&mut self.pending, version);
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

/// Runs an atomic region: `region name is atomic` with an optional deadline (`within`), serializable. The body runs
/// with a fresh [`Tx`]; `Ok(Step::Done(r))` commits and returns `r`; `Ok(Step::Retry)` discards the run and waits for
/// a commit; `Err(e)` discards the run and fails with `e`. Waiting past the deadline fails with `expired`.
pub fn atomic<R, E>(
    name: &str,
    deadline: Option<&Deadline>,
    body: impl FnMut(&mut Tx) -> Result<Step<R>, E>,
) -> Result<R, Aborted<E>> {
    atomic_with(name, deadline, Isolation::Serializable, body)
}

/// As [`atomic`], with the region's isolation: `isolation snapshot`.
pub fn atomic_with<R, E>(
    name: &str,
    deadline: Option<&Deadline>,
    isolation: Isolation,
    mut body: impl FnMut(&mut Tx) -> Result<Step<R>, E>,
) -> Result<R, Aborted<E>> {
    orelse_with(name, deadline, isolation, vec![&mut body])
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

/// Y's `orelse`, serializable: the alternatives, left to right. The first that does not retry decides; when every one
/// retries, the region waits for a commit and tries them all again.
pub fn orelse<R, E>(name: &str, deadline: Option<&Deadline>, alternatives: Vec<Alternative<'_, R, E>>) -> Result<R, Aborted<E>> {
    orelse_with(name, deadline, Isolation::Serializable, alternatives)
}

/// As [`orelse`], with the region's isolation.
pub fn orelse_with<R, E>(
    name: &str,
    deadline: Option<&Deadline>,
    isolation: Isolation,
    mut alternatives: Vec<Alternative<'_, R, E>>,
) -> Result<R, Aborted<E>> {
    // a test that forces the region's deadline to pass (`faults: expire at f#region`)
    if crate::faults::take(crate::faults::Kind::Expire, name) {
        return Err(Aborted::Expired(Expired::fake()));
    }
    let r = match engine_for_a_run() {
        Engine::OneLock => one_lock(name, deadline, &mut alternatives),
        Engine::Optimistic => optimistic(name, deadline, isolation, &mut alternatives),
    };
    // a test that crashes the program once the region has committed (`faults: crash after commit f#region`)
    if r.is_ok() && crate::faults::take(crate::faults::Kind::Crash, name) {
        crate::journal::crash_after_commit(name);
    }
    r
}

/// Whether a run that would commit is made to conflict instead, by a test's `faults: conflict at f#region`: it is
/// discarded, with what it logged, and run again, as a conflicting commit would make it.
fn forced_conflict(name: &str) -> bool {
    let forced = crate::faults::take(crate::faults::Kind::Conflict, name);
    if forced {
        RERUNS.fetch_add(1, Ordering::SeqCst);
        crate::observe::discard_run();
    }
    forced
}

/// Every alternative under one hold of the lock: never run again.
fn one_lock<R, E>(name: &str, deadline: Option<&Deadline>, alternatives: &mut [Alternative<'_, R, E>]) -> Result<R, Aborted<E>> {
    let mut held = hold();
    'run: loop {
        for alternative in alternatives.iter_mut() {
            let mut tx = Tx::new(None);
            crate::observe::begin_run();
            match alternative(&mut tx) {
                // a conflicting commit makes the region run again, at once
                Ok(Step::Done(_)) if forced_conflict(name) => continue 'run,
                Ok(Step::Done(r)) => {
                    if !tx.apply.is_empty() {
                        *held += 1;
                        tx.commit(*held);
                        CHANGED.notify_all();
                    }
                    crate::observe::commit_run();
                    return Ok(r);
                }
                Ok(Step::Retry) => crate::observe::discard_run(),
                Err(e) => {
                    crate::observe::discard_run();
                    return Err(Aborted::Failed(e));
                }
            }
        }
        held = wait(held, deadline).map_err(Aborted::Expired)?;
    }
}

/// Each run without the lock, at the version committed when it starts; the lock only to commit. A run that a commit
/// invalidated, whatever it decided, is run again.
fn optimistic<R, E>(name: &str, deadline: Option<&Deadline>, isolation: Isolation, alternatives: &mut [Alternative<'_, R, E>]) -> Result<R, Aborted<E>> {
    'run: loop {
        let stamp = *hold();
        // what the alternatives that retried read: the run waits until a commit changes some of it
        let mut watched: Vec<VersionOf> = vec![];
        for alternative in alternatives.iter_mut() {
            let mut tx = Tx::new(Some(stamp));
            crate::observe::begin_run();
            let decided = alternative(&mut tx);
            let mut held = hold();
            if !tx.valid(isolation) || (matches!(decided, Ok(Step::Done(_))) && crate::faults::take(crate::faults::Kind::Conflict, name)) {
                drop(held);
                crate::observe::discard_run();
                RERUNS.fetch_add(1, Ordering::SeqCst);
                continue 'run;
            }
            match decided {
                Ok(Step::Done(r)) => {
                    if !tx.apply.is_empty() {
                        *held += 1;
                        tx.commit(*held);
                        CHANGED.notify_all();
                    }
                    crate::observe::commit_run();
                    return Ok(r);
                }
                Ok(Step::Retry) => {
                    crate::observe::discard_run();
                    watched.extend(tx.read.borrow().values().cloned())
                }
                Err(e) => {
                    crate::observe::discard_run();
                    return Err(Aborted::Failed(e));
                }
            }
        }
        // every alternative retried: wait until a commit changes what they read (Y § 3.9: `retry` waits until something
        // it read changes), or the deadline
        let mut held = hold();
        while !watched.iter().any(|v| v() > stamp) {
            held = wait(held, deadline).map_err(Aborted::Expired)?;
        }
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

    /// Held by a test that chooses the engine, or reads which is chosen.
    static CHOSEN: Mutex<()> = Mutex::new(());

    /// Runs `f` with the optimistic engine. The tests that choose it hold this lock, so that no other test of the file
    /// sees the engine change under it.
    fn optimistically<R>(f: impl FnOnce() -> R) -> R {
        let _g = CHOSEN.lock().unwrap_or_else(|e| e.into_inner());
        // the tests switch the engine between their regions, which a program may not: around the check
        let before = ENGINE.load(Ordering::SeqCst);
        ENGINE.store((before & RAN) | OPTIMISTIC, Ordering::SeqCst);
        let r = f();
        ENGINE.store(before, Ordering::SeqCst);
        r
    }

    #[test]
    fn the_engine_is_the_one_chosen_or_else_the_features() {
        let unchosen = || {
            let _g = CHOSEN.lock().unwrap_or_else(|e| e.into_inner());
            engine()
        };
        assert_eq!(unchosen() == Engine::Optimistic, cfg!(feature = "optimistic"));
        assert_eq!(optimistically(engine), Engine::Optimistic);
        assert_eq!(unchosen() == Engine::Optimistic, cfg!(feature = "optimistic"), "the choice is restored");
    }

    #[test]
    fn the_engine_is_fixed_once_a_region_ran() {
        let cell = AtomicU8::new(NONE);
        // before any region, the engine may change, as often as the program likes
        assert_eq!(choose(&cell, Engine::Optimistic), Ok(()));
        assert_eq!(choose(&cell, Engine::OneLock), Ok(()));
        assert_eq!(run_with(&cell), Engine::OneLock);
        // after one, choosing the same engine is no change, and choosing another fails
        assert_eq!(choose(&cell, Engine::OneLock), Ok(()));
        assert!(choose(&cell, Engine::Optimistic).unwrap_err().contains("already ran with the engine OneLock"));
        assert_eq!(engine_of(cell.load(Ordering::SeqCst)), Engine::OneLock);
        // and the root capability makes it a fault: once this region ran, the other engine is refused
        let _g = CHOSEN.lock().unwrap_or_else(|e| e.into_inner());
        let r: Result<u32, Aborted<()>> = atomic("first", None, |_| Ok(Step::Done(1)));
        assert_eq!(r, Ok(1));
        let now = engine();
        let other = if now == Engine::OneLock { Engine::Optimistic } else { Engine::OneLock };
        crate::System::root().use_engine(now);
        let late = std::panic::catch_unwind(|| crate::System::root().use_engine(other));
        let why = late.unwrap_err().downcast::<String>().map(|s| *s).unwrap_or_default();
        assert!(why.contains("fault at y_runtime::System::use_engine: an atomic region already ran"), "{why}");
        assert_eq!(engine(), now);
    }

    /// A run reads `a`, another region commits a write to `read_by_other`, and the run then writes `b`. Returns how many
    /// times the run ran.
    fn interleaved(isolation: Isolation, written_by_other_is_read: bool) -> u32 {
        let a = Shared::new(1u32);
        let b = Shared::new(0u32);
        let other_target = if written_by_other_is_read { a.clone() } else { b.clone() };
        let runs = Cell::new(0u32);
        let r: Result<(), Aborted<()>> = atomic_with("skew", None, isolation, |tx| {
            runs.set(runs.get() + 1);
            let seen = tx.get(&a);
            if runs.get() == 1 {
                // another task commits in the middle of the first run
                let _: Result<(), Aborted<()>> = atomic("other", None, |tx2| {
                    tx2.set(&other_target, 10);
                    Ok(Step::Done(()))
                });
            }
            tx.set(&b, seen);
            Ok(Step::Done(()))
        });
        assert_eq!(r, Ok(()));
        runs.get()
    }

    #[test]
    fn optimistic_serializable_runs_again_when_what_it_read_changed() {
        optimistically(|| {
            assert_eq!(interleaved(Isolation::Serializable, true), 2);
        });
    }

    #[test]
    fn optimistic_snapshot_allows_write_skew_and_stops_lost_updates() {
        optimistically(|| {
            // only read: the snapshot run commits, with what it saw when it started
            assert_eq!(interleaved(Isolation::Snapshot, true), 1);
            // written by both: first committer wins, so the run is run again
            assert_eq!(interleaved(Isolation::Snapshot, false), 2);
        });
    }

    #[test]
    fn optimistic_regions_stay_serializable_and_a_failure_leaves_no_trace() {
        optimistically(|| {
            let counter = Shared::new(0u64);
            let threads: Vec<_> = (0..8)
                .map(|_| {
                    let c = counter.clone();
                    thread::spawn(move || {
                        for _ in 0..300 {
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
            assert_eq!(counter.snapshot(), 2400);
            let r: Result<(), Aborted<&str>> = atomic("fail", None, |tx| {
                tx.set(&counter, 0);
                Err("declined")
            });
            assert_eq!(r, Err(Aborted::Failed("declined")));
            assert_eq!(counter.snapshot(), 2400);
        });
    }

    #[test]
    fn optimistic_retry_waits_for_what_it_read_to_change() {
        optimistically(|| {
            let stock = Shared::new(0u32);
            let other = Shared::new(0u32);
            let (filler, unrelated) = (stock.clone(), other.clone());
            let runs = Arc::new(AtomicU64::new(0));
            let counted = Arc::clone(&runs);
            let waiter = thread::spawn(move || {
                let d = Deadline::after(Duration::from_secs(5));
                atomic("take", Some(&d), |tx| -> Result<Step<u32>, ()> {
                    counted.fetch_add(1, Ordering::SeqCst);
                    let n = tx.get(&stock);
                    if n == 0 {
                        return Ok(Step::Retry);
                    }
                    tx.set(&stock, n - 1);
                    Ok(Step::Done(n))
                })
            });
            thread::sleep(Duration::from_millis(50));
            // commits to what the waiter did not read do not wake it to run again
            for i in 0..5 {
                let _: Result<(), Aborted<()>> = atomic("other", None, |tx| {
                    tx.set(&unrelated, i);
                    Ok(Step::Done(()))
                });
            }
            thread::sleep(Duration::from_millis(50));
            assert_eq!(runs.load(Ordering::SeqCst), 1, "the waiter ran again for a commit it did not read");
            let _: Result<(), Aborted<()>> = atomic("fill", None, |tx| {
                tx.set(&filler, 2);
                Ok(Step::Done(()))
            });
            assert_eq!(waiter.join().unwrap(), Ok(2));
            assert_eq!(runs.load(Ordering::SeqCst), 2);
        });
    }
}

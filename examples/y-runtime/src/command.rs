//! Commands, their handlers, and the workflows that submit them (Y § 3.10). A stand-in for Y's durable command
//! runtime, made durable by the journal (`crate::journal`).
//!
//! - A command is a value with a delivery [`Policy`] ([`Command`]).
//! - `enqueue c(…)` adds the command to an [`Outbox`] held in shared state, inside an atomic region: the command is
//!   there if and only if the region commits, and a command with the key of one already there collapses into it
//!   (`deduplicate_by`). A [`Worker`] delivers what it holds; a test without one reads it.
//! - `submit c(…)` delivers the command to its handler through a [`Submit`] and waits for the outcome. A handler that
//!   fails has failed: `undelivered.failed(error)`. A handler that cannot be delivered to (here: one that faults) is
//!   tried again as the policy says, with its backoff, while its deadline allows and only when it is idempotent; one
//!   that exhausts the policy is abandoned, `undelivered.abandoned(abandonment)`, and then compensated by the command
//!   its `on_abandon` builds, or parked as a dead letter. Each outcome is recorded against the command's key.
//!
//! Inside a workflow, `submit` is a durable step (`crate::journal`): the command is recorded before it is delivered,
//! and its outcome once it is known, under the workflow's identity. A workflow that resumes gets the recorded outcome
//! and does not deliver the command again; one that crashed while a handler ran delivers it again. A handler runs
//! outside the workflow: its own atomic regions are no steps of it. A crash is not caught as a failed delivery.
//!
//! A handler runs on a worker, a thread of its own: a submit waits while one delivers (`Submit`), and a [`Worker`]
//! delivers what an outbox's committed transactions enqueued, taking each in a transaction of its own.
//!
//! A command enqueued is recorded in the journal when its transaction commits ([`Outbox::enqueue`]), and a worker that
//! starts first takes back each one recorded with no outcome: an enqueued command outlives the process, and one whose
//! delivery crashed is delivered again. A worker shares the faults and the journal of the thread that started it.
use crate::journal::{case, Durable, Tokens};
use crate::shared::{atomic, Shared, Step};
use crate::Deadline;
use std::convert::Infallible;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// What happens to a command that exhausts its policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnAbandon {
    /// `on_abandon: compensate c(…)`: the compensating command is delivered.
    Compensate,
    /// `on_abandon: park`: the command is kept as a dead letter.
    Park,
}

/// A command's delivery policy: `retry: backoff(first: …, factor: …, attempts: …)`, `deadline: …`, `idempotent: …`,
/// `on_abandon: …`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub attempts: u32,
    pub first: Duration,
    pub factor: u32,
    pub deadline: Duration,
    pub idempotent: bool,
    pub on_abandon: OnAbandon,
}

/// A command: its policy, and the key duplicates collapse on (`deduplicate_by`).
pub trait Command: Clone {
    fn policy() -> Policy;
    fn key(&self) -> String;
}

/// The commands an atomic region has enqueued and committed: `enqueue<command_type=c>`, held in shared state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outbox<C> {
    pending: Vec<C>,
}

impl<C> Default for Outbox<C> {
    fn default() -> Self {
        Outbox { pending: vec![] }
    }
}

impl<C: Command> Outbox<C> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `enqueue c`: the command, unless one with its key is already there.
    pub fn add(&mut self, command: C) {
        if !self.pending.iter().any(|p| p.key() == command.key()) {
            self.pending.push(command);
        }
    }

    /// `enqueue c` in the transaction `tx`: the command, unless one with its key is already there, and its record in the
    /// journal once the transaction commits (`crate::journal::enqueued`), so that it outlives the process.
    pub fn enqueue(&mut self, tx: &mut crate::shared::Tx, command: C)
    where
        C: Durable,
    {
        if self.pending.iter().any(|p| p.key() == command.key()) {
            return;
        }
        let (key, data) = (command.key(), crate::journal::tokens(&command));
        tx.on_commit(move || crate::journal::enqueued(&key, data));
        self.pending.push(command);
    }

    /// The oldest command waiting, taken out: what a worker delivers next.
    pub fn take_first(&mut self) -> Option<C> {
        (!self.pending.is_empty()).then(|| self.pending.remove(0))
    }

    /// How many commands are waiting.
    pub fn count(&self) -> usize {
        self.pending.len()
    }

    /// The commands waiting, in the order they were enqueued.
    pub fn pending(&self) -> Vec<C> {
        self.pending.clone()
    }
}

/// Why abandonment happened: how many times delivery was tried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Abandonment {
    pub attempts: u32,
}

impl Durable for Abandonment {
    fn put(&self, out: &mut Vec<String>) {
        self.attempts.put(out);
    }
    fn take(input: &mut Tokens<'_>) -> Option<Self> {
        Some(Abandonment { attempts: u32::take(input)? })
    }
}

/// Y's `undelivered<failure_type=e>`: why a submitted command has no outcome to give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Undelivered<E> {
    /// The handler ran, and failed.
    Failed { error: E },
    /// Delivery exhausted the command's policy.
    Abandoned { abandonment: Abandonment },
}

impl<E: Durable> Durable for Undelivered<E> {
    fn put(&self, out: &mut Vec<String>) {
        match self {
            Undelivered::Failed { error } => {
                case(out, 0);
                error.put(out);
            }
            Undelivered::Abandoned { abandonment } => {
                case(out, 1);
                abandonment.put(out);
            }
        }
    }
    fn take(input: &mut Tokens<'_>) -> Option<Self> {
        match input.case()? {
            0 => Some(Undelivered::Failed { error: E::take(input)? }),
            1 => Some(Undelivered::Abandoned { abandonment: Abandonment::take(input)? }),
            _ => None,
        }
    }
}

/// What happened to a command, recorded against its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed { key: String },
    Failed { key: String },
    Abandoned { key: String, compensated: bool, parked: bool },
}

/// `submit<command_type=c>`: where a workflow submits a command to its handler and waits for the outcome. Made with the
/// handler, its capabilities taken where the capability is made, and the compensation `on_abandon` names.
pub struct Submit<'a, C, R, E> {
    handler: Box<dyn Fn(&C) -> Result<R, E> + Send + Sync + 'a>,
    compensation: Option<Box<dyn Fn(&C) + Send + Sync + 'a>>,
    outcomes: Mutex<Vec<Outcome>>,
}

impl<'a, C: Command + Durable + Send + Sync, R: Durable + Send, E: Durable + Send> Submit<'a, C, R, E> {
    /// `submit.to(handler)`.
    pub fn to(
        handler: impl Fn(&C) -> Result<R, E> + Send + Sync + 'a,
        compensation: Option<Box<dyn Fn(&C) + Send + Sync + 'a>>,
    ) -> Self {
        Submit { handler: Box::new(handler), compensation, outcomes: Mutex::new(vec![]) }
    }

    /// `submit c`: the handler's result, or why there is none; in a workflow, a durable step. The workflow waits while a
    /// worker delivers the command.
    pub fn submit(&self, command: C) -> Result<R, Undelivered<E>> {
        crate::journal::step("submit", || {
            crate::journal::command(&command.key(), &command);
            crate::journal::outside(|| self.deliver(command))
        })
    }

    /// Runs `run` on a worker: a thread of its own, which shares nothing of the caller's thread but the faults its test
    /// forced and its journal (`crate::faults::home`, `crate::journal::home`). A panic there, a crash among them, goes on
    /// here.
    fn on_worker<T: Send>(run: impl FnOnce() -> T + Send) -> T {
        let (forced, journal) = (crate::faults::home(), crate::journal::home());
        let r = std::thread::scope(|workers| {
            workers
                .spawn(move || {
                    crate::faults::adopt(forced);
                    crate::journal::adopt(journal);
                    catch_unwind(AssertUnwindSafe(run))
                })
                .join()
                .unwrap_or_else(|p| resume_unwind(p))
        });
        r.unwrap_or_else(|p| resume_unwind(p))
    }

    fn record(&self, outcome: Outcome) {
        self.outcomes.lock().unwrap_or_else(|p| p.into_inner()).push(outcome);
    }

    /// Delivers a command to its handler on a worker, as its policy says: tried again while the handler cannot be
    /// delivered to, then abandoned and compensated or parked.
    fn deliver(&self, command: C) -> Result<R, Undelivered<E>> {
        let policy = C::policy();
        let started = Instant::now();
        let mut wait = policy.first;
        let tries = if policy.idempotent { policy.attempts.max(1) } else { 1 };
        let mut attempts = 0;
        while attempts < tries {
            attempts += 1;
            match Self::on_worker(|| catch_unwind(AssertUnwindSafe(|| (self.handler)(&command)))) {
                Ok(Ok(r)) => {
                    self.record(Outcome::Completed { key: command.key() });
                    return Ok(r);
                }
                Ok(Err(e)) => {
                    self.record(Outcome::Failed { key: command.key() });
                    return Err(Undelivered::Failed { error: e });
                }
                // a crash is the program's, not the delivery's
                Err(p) if p.is::<crate::faults::Crashed>() => resume_unwind(p),
                // the handler could not be delivered to: tried again after the backoff, within the deadline
                Err(_) => {
                    if attempts == tries || started.elapsed() + wait > policy.deadline {
                        break;
                    }
                    std::thread::sleep(wait);
                    wait *= policy.factor.max(1);
                }
            }
        }
        let compensated = policy.on_abandon == OnAbandon::Compensate && self.compensation.is_some();
        if let (OnAbandon::Compensate, Some(c)) = (policy.on_abandon, &self.compensation) {
            Self::on_worker(|| c(&command));
        }
        self.record(Outcome::Abandoned {
            key: command.key(),
            compensated,
            parked: policy.on_abandon == OnAbandon::Park,
        });
        Err(Undelivered::Abandoned { abandonment: Abandonment { attempts } })
    }

    /// Every outcome so far, in order.
    pub fn outcomes(&self) -> Vec<Outcome> {
        self.outcomes.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// How many commands completed.
    pub fn completed(&self) -> usize {
        self.outcomes().iter().filter(|o| matches!(o, Outcome::Completed { .. })).count()
    }

    /// How many were abandoned and compensated.
    pub fn compensated(&self) -> usize {
        self.outcomes().iter().filter(|o| matches!(o, Outcome::Abandoned { compensated: true, .. })).count()
    }

    /// How many were abandoned and parked as dead letters.
    pub fn parked(&self) -> usize {
        self.outcomes().iter().filter(|o| matches!(o, Outcome::Abandoned { parked: true, .. })).count()
    }
}

/// How far a [`Worker`] has got: how many times it looked and found nothing, and whether it is to stop.
#[derive(Default)]
struct Busy {
    idle_rounds: u64,
    closed: bool,
}

/// A worker that delivers the commands an outbox holds (Y § 3.10): `enqueue c(…)` puts a command there when its
/// transaction commits, and the worker, on a thread of its own, takes each in an atomic region of its own (so a command
/// leaves the outbox once, when the taking commits) and delivers it to its handler as its policy says. Its outcomes are
/// the [`Submit`]'s it delivers through. It runs until it is dropped, in the thread scope it was started in.
pub struct Worker<'env, C: Command + 'static, R, E> {
    delivery: Arc<Submit<'env, C, R, E>>,
    busy: Arc<(Mutex<Busy>, Condvar)>,
}

impl<'env, C: Command + Durable + Send + Sync + 'static, R: Durable + Send + 'env, E: Durable + Send + 'env>
    Worker<'env, C, R, E>
{
    /// Starts a worker on the outbox, in a thread scope, delivering through `delivery`.
    pub fn start<'scope>(
        workers: &'scope std::thread::Scope<'scope, 'env>,
        outbox: &'env Shared<Outbox<C>>,
        delivery: Submit<'env, C, R, E>,
    ) -> Self {
        let delivery = Arc::new(delivery);
        let busy: Arc<(Mutex<Busy>, Condvar)> = Arc::default();
        let (d, b) = (Arc::clone(&delivery), Arc::clone(&busy));
        let (forced, journal) = (crate::faults::home(), crate::journal::home());
        workers.spawn(move || {
            crate::faults::adopt(forced);
            crate::journal::adopt(journal);
            // a worker that starts, as one does when its program starts again, first takes back what the journal
            // holds as enqueued and not delivered
            Self::recover(outbox);
            loop {
                if b.0.lock().unwrap_or_else(|p| p.into_inner()).closed {
                    break;
                }
                // the oldest command waiting, taken in a transaction; none within a moment, and the worker looks again
                let wait = Deadline::after(Duration::from_millis(20));
                let taken = atomic("y_runtime::command::Worker#take", Some(&wait), |tx| {
                    let mut held = tx.get(outbox);
                    match held.take_first() {
                        Some(c) => {
                            tx.set(outbox, held);
                            Ok::<_, Infallible>(Step::Done(c))
                        }
                        None => Ok(Step::Retry),
                    }
                });
                match taken {
                    Ok(command) => {
                        let key = command.key();
                        match catch_unwind(AssertUnwindSafe(|| d.deliver(command))) {
                            // the outcome, recorded against the command's identity (Y § 3.10)
                            Ok(_) => {
                                if let Some(o) = d.outcomes().last() {
                                    crate::journal::outcome(&key, o);
                                }
                            }
                            // the program crashed while the command was delivered: it starts again, and its worker takes
                            // back what the journal holds as not delivered, this command among them
                            Err(p) if p.is::<crate::faults::Crashed>() => Self::recover(outbox),
                            Err(p) => resume_unwind(p),
                        }
                    }
                    // a round that found nothing: what `settle` waits for
                    Err(_) => {
                        b.0.lock().unwrap_or_else(|p| p.into_inner()).idle_rounds += 1;
                        b.1.notify_all();
                    }
                }
            }
        });
        Worker { delivery, busy }
    }

    /// Puts back in the outbox each command the journal holds as enqueued and not delivered, in one transaction; one
    /// with the key of a command still waiting there is that command.
    fn recover(outbox: &Shared<Outbox<C>>) {
        let lost: Vec<C> =
            crate::journal::undelivered().iter().filter_map(|data| crate::journal::value(data)).collect();
        if lost.is_empty() {
            return;
        }
        let _ = atomic("y_runtime::command::Worker#recover", None, |tx| {
            let mut held = tx.get(outbox);
            for c in &lost {
                held.add(c.clone());
            }
            tx.set(outbox, held);
            Ok::<_, Infallible>(Step::Done(()))
        });
    }

    /// Waits until every command committed so far has been delivered: until the worker ends a round that found the
    /// outbox empty, which, as a commit wakes a round that waits, saw every commit made before `settle` was called. It
    /// is what a test's `then` reads after its `when`.
    pub fn settle(&self) {
        let mut state = self.busy.0.lock().unwrap_or_else(|p| p.into_inner());
        let start = state.idle_rounds;
        while state.idle_rounds == start {
            state = self.busy.1.wait(state).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// The outcomes of what the worker delivered.
    pub fn outcomes(&self) -> Vec<Outcome> {
        self.delivery.outcomes()
    }
}

impl<C: Command + 'static, R, E> Drop for Worker<'_, C, R, E> {
    fn drop(&mut self) {
        self.busy.0.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
        self.busy.1.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A number a handler counts with, on a worker's thread.
    #[derive(Default)]
    struct Count(AtomicU32);

    impl Count {
        fn get(&self) -> u32 {
            self.0.load(Ordering::SeqCst)
        }
        fn set(&self, v: u32) {
            self.0.store(v, Ordering::SeqCst)
        }
    }

    #[derive(Clone)]
    struct Charge(u32);

    impl Durable for Charge {
        fn put(&self, out: &mut Vec<String>) {
            self.0.put(out);
        }
        fn take(input: &mut Tokens<'_>) -> Option<Self> {
            Some(Charge(u32::take(input)?))
        }
    }

    impl Command for Charge {
        fn policy() -> Policy {
            Policy {
                attempts: 3,
                first: Duration::from_millis(1),
                factor: 2,
                deadline: Duration::from_secs(1),
                idempotent: true,
                on_abandon: OnAbandon::Compensate,
            }
        }
        fn key(&self) -> String {
            self.0.to_string()
        }
    }

    #[test]
    fn an_outbox_collapses_duplicates() {
        let mut o = Outbox::new();
        o.add(Charge(1));
        o.add(Charge(1));
        o.add(Charge(2));
        assert_eq!(o.count(), 2);
    }

    #[test]
    fn a_handler_that_cannot_be_delivered_to_is_retried_then_compensated() {
        let tries = Count::default();
        let undone = Count::default();
        let s: Submit<Charge, u32, ()> = Submit::to(
            |_| {
                tries.set(tries.get() + 1);
                panic!("unreachable gateway")
            },
            Some(Box::new(|_| undone.set(undone.get() + 1))),
        );
        let r = s.submit(Charge(7));
        assert_eq!(r, Err(Undelivered::Abandoned { abandonment: Abandonment { attempts: 3 } }));
        assert_eq!((tries.get(), undone.get(), s.compensated()), (3, 1, 1));
    }

    #[test]
    fn a_handler_that_fails_has_failed() {
        let s: Submit<Charge, u32, String> = Submit::to(|_| Err("declined".into()), None);
        assert_eq!(s.submit(Charge(1)), Err(Undelivered::Failed { error: "declined".to_string() }));
        let s: Submit<Charge, u32, String> = Submit::to(|c| Ok(c.0 * 2), None);
        assert_eq!((s.submit(Charge(4)), s.completed()), (Ok(8), 1));
    }

    #[test]
    fn a_resumed_workflow_does_not_deliver_a_command_again() {
        crate::journal::clear();
        let delivered = Count::default();
        let s: Submit<Charge, u32, String> = Submit::to(
            |c| {
                delivered.set(delivered.get() + 1);
                Ok(c.0 + 1000)
            },
            None,
        );
        // the program crashes after the outcome is recorded, and the workflow runs again
        let crashed = Cell::new(false);
        let receipt = crate::faults::restarting(|| {
            crate::journal::workflow("m::fulfil", "v1", vec!["7".into()], || {
                let r = s.submit(Charge(7));
                if !crashed.replace(true) {
                    crate::faults::crash("m::fulfil");
                }
                r
            })
        });
        assert_eq!((receipt, delivered.get()), (Ok(1007), 1));
        let kinds: Vec<String> = crate::journal::records().into_iter().map(|r| r.kind).collect();
        assert_eq!(kinds, ["command", "submit", "result"]);
    }

    #[test]
    fn a_worker_delivers_what_an_outbox_committed() {
        let outbox = Shared::new(Outbox::new());
        let charged = Count::default();
        std::thread::scope(|workers| {
            let worker = Worker::start(
                workers,
                &outbox,
                Submit::to(
                    |c: &Charge| -> Result<u32, String> {
                        charged.set(charged.get() + c.0);
                        Ok(c.0)
                    },
                    None,
                ),
            );
            // a transaction enqueues two, one of them twice: the duplicate collapses
            let r = atomic("m::f#place", None, |tx| {
                let mut o = tx.get(&outbox);
                o.add(Charge(3));
                o.add(Charge(3));
                o.add(Charge(4));
                tx.set(&outbox, o);
                Ok::<_, Infallible>(Step::Done(()))
            });
            assert!(r.is_ok());
            worker.settle();
            assert_eq!((charged.get(), outbox.snapshot().count(), worker.outcomes().len()), (7, 0, 2));
        });
    }

    /// Enqueues commands in one transaction, as `enqueue` lowers.
    fn place(outbox: &Shared<Outbox<Charge>>, charges: &[u32]) {
        let r = atomic("m::place#placing", None, |tx| {
            let mut o = tx.get(outbox);
            for c in charges {
                o.enqueue(tx, Charge(*c));
            }
            tx.set(outbox, o);
            Ok::<_, Infallible>(Step::Done(()))
        });
        assert!(r.is_ok());
    }

    #[test]
    fn an_enqueued_command_outlives_its_outbox() {
        crate::journal::clear();
        // a program enqueues a charge and ends before any worker delivers it
        place(&Shared::new(Outbox::new()), &[5]);
        // the program starts again, with an empty outbox: its worker takes the charge back from the journal
        let outbox = Shared::new(Outbox::new());
        let charged = Count::default();
        std::thread::scope(|workers| {
            let worker = Worker::start(
                workers,
                &outbox,
                Submit::to(
                    |c: &Charge| -> Result<u32, String> {
                        charged.set(charged.get() + c.0);
                        Ok(c.0)
                    },
                    None,
                ),
            );
            worker.settle();
        });
        assert_eq!(charged.get(), 5);
        assert!(crate::journal::undelivered().is_empty());
    }

    #[test]
    fn a_worker_that_crashes_delivers_again_when_it_starts_again() {
        crate::journal::clear();
        let outbox = Shared::new(Outbox::new());
        let (tries, charged) = (Count::default(), Count::default());
        std::thread::scope(|workers| {
            let worker = Worker::start(
                workers,
                &outbox,
                Submit::to(
                    |c: &Charge| -> Result<u32, String> {
                        charged.set(charged.get() + c.0);
                        // the program crashes after the first charge took effect, before its outcome is recorded
                        tries.set(tries.get() + 1);
                        if tries.get() == 1 {
                            crate::faults::crash("m::charge#taken");
                        }
                        Ok(c.0)
                    },
                    None,
                ),
            );
            place(&outbox, &[5]);
            worker.settle();
        });
        assert_eq!((tries.get(), charged.get()), (2, 10), "delivered again: at least once");
    }
}

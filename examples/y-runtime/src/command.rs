//! Commands, their handlers, and the workflows that submit them (Y § 3.10). A stand-in, in memory, for Y's durable
//! command runtime: it keeps the meaning of each construct within one process, and none of its durability.
//!
//! - A command is a value with a delivery [`Policy`] ([`Command`]).
//! - `enqueue c(…)` adds the command to an [`Outbox`] held in shared state, inside an atomic region: the command is
//!   there if and only if the region commits, and a command with the key of one already there collapses into it
//!   (`deduplicate_by`). The stand-in delivers nothing from an outbox; a test reads what it holds.
//! - `submit c(…)` delivers the command to its handler through a [`Submit`] and waits for the outcome. A handler that
//!   fails has failed: `undelivered.failed(error)`. A handler that cannot be delivered to (here: one that faults) is
//!   tried again as the policy says, with its backoff, while its deadline allows and only when it is idempotent; one
//!   that exhausts the policy is abandoned, `undelivered.abandoned(abandonment)`, and then compensated by the command
//!   its `on_abandon` builds, or parked as a dead letter. Each outcome is recorded against the command's key.
//!
//! What it does not keep: commands and outcomes do not outlive the process, a workflow does not resume after a crash,
//! and a submitted command is delivered at once rather than by a separate worker.
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
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

/// Y's `undelivered<failure_type=e>`: why a submitted command has no outcome to give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Undelivered<E> {
    /// The handler ran, and failed.
    Failed { error: E },
    /// Delivery exhausted the command's policy.
    Abandoned { abandonment: Abandonment },
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
    handler: Box<dyn Fn(&C) -> Result<R, E> + 'a>,
    compensation: Option<Box<dyn Fn(&C) + 'a>>,
    outcomes: RefCell<Vec<Outcome>>,
}

impl<'a, C: Command, R, E> Submit<'a, C, R, E> {
    /// `submit.to(handler)`.
    pub fn to(handler: impl Fn(&C) -> Result<R, E> + 'a, compensation: Option<Box<dyn Fn(&C) + 'a>>) -> Self {
        Submit { handler: Box::new(handler), compensation, outcomes: RefCell::new(vec![]) }
    }

    /// `submit c`: the handler's result, or why there is none.
    pub fn submit(&self, command: C) -> Result<R, Undelivered<E>> {
        let policy = C::policy();
        let started = Instant::now();
        let mut wait = policy.first;
        let tries = if policy.idempotent { policy.attempts.max(1) } else { 1 };
        let mut attempts = 0;
        while attempts < tries {
            attempts += 1;
            match catch_unwind(AssertUnwindSafe(|| (self.handler)(&command))) {
                Ok(Ok(r)) => {
                    self.outcomes.borrow_mut().push(Outcome::Completed { key: command.key() });
                    return Ok(r);
                }
                Ok(Err(e)) => {
                    self.outcomes.borrow_mut().push(Outcome::Failed { key: command.key() });
                    return Err(Undelivered::Failed { error: e });
                }
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
            c(&command);
        }
        self.outcomes.borrow_mut().push(Outcome::Abandoned { key: command.key(), compensated, parked: policy.on_abandon == OnAbandon::Park });
        Err(Undelivered::Abandoned { abandonment: Abandonment { attempts } })
    }

    /// Every outcome so far, in order.
    pub fn outcomes(&self) -> Vec<Outcome> {
        self.outcomes.borrow().clone()
    }

    /// How many commands completed.
    pub fn completed(&self) -> usize {
        self.outcomes.borrow().iter().filter(|o| matches!(o, Outcome::Completed { .. })).count()
    }

    /// How many were abandoned and compensated.
    pub fn compensated(&self) -> usize {
        self.outcomes.borrow().iter().filter(|o| matches!(o, Outcome::Abandoned { compensated: true, .. })).count()
    }

    /// How many were abandoned and parked as dead letters.
    pub fn parked(&self) -> usize {
        self.outcomes.borrow().iter().filter(|o| matches!(o, Outcome::Abandoned { parked: true, .. })).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Clone)]
    struct Charge(u32);

    impl Command for Charge {
        fn policy() -> Policy {
            Policy { attempts: 3, first: Duration::from_millis(1), factor: 2, deadline: Duration::from_secs(1), idempotent: true, on_abandon: OnAbandon::Compensate }
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
        let tries = Cell::new(0);
        let undone = Cell::new(0);
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
        let s: Submit<Charge, u32, &str> = Submit::to(|_| Err("declined"), None);
        assert_eq!(s.submit(Charge(1)), Err(Undelivered::Failed { error: "declined" }));
        let s: Submit<Charge, u32, &str> = Submit::to(|c| Ok(c.0 * 2), None);
        assert_eq!((s.submit(Charge(4)), s.completed()), (Ok(8), 1));
    }
}

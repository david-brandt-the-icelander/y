//! Records the path each call of an instrumented function takes, for checking the paths a test says it covers.
//! Each record is a line `test<TAB>function<TAB>step|step|...`, appended to the file `$YTRACE_OUT`.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::io::Write;
use std::sync::Mutex;

static OUT: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct Frame {
    name: &'static str,
    steps: Vec<String>,
    /// For each loop being run: its id, where its pass's steps start, and how many passes it has made.
    loops: Vec<(u32, usize, u32)>,
    /// What the last predicate evaluated showed, for the step that follows it.
    pending: Option<String>,
    /// What the predicate being evaluated has shown so far: for each operand, its position and order, if any.
    building: Vec<Vec<(usize, Option<&'static str>)>>,
    operand: Vec<usize>,
}

thread_local! {
    static STACK: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
}

fn with<R>(f: impl FnOnce(&mut Frame) -> R) -> Option<R> {
    STACK.with(|s| s.borrow_mut().last_mut().map(f))
}

pub struct Guard;

pub fn enter(name: &'static str) -> Guard {
    STACK.with(|s| s.borrow_mut().push(Frame { name, ..Default::default() }));
    Guard
}

impl Drop for Guard {
    fn drop(&mut self) {
        let frame = STACK.with(|s| s.borrow_mut().pop());
        // a call that a fault ended took no path: a fault is a bug, not an outcome (Y § 3.4), and a path that ends in
        // one, `fail not_reachable` or a failed `assert`, needs no test
        if std::thread::panicking() {
            return;
        }
        if let (Some(frame), Ok(path)) = (frame, std::env::var("YTRACE_OUT")) {
            let test = std::thread::current().name().unwrap_or("?").to_string();
            let _lock = OUT.lock();
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{test}\t{}\t{}", frame.name, frame.steps.join("|"));
            }
        }
    }
}

/// A named outcome: takes what the predicate before it showed.
pub fn step(name: &str) {
    with(|f| {
        let suffix = f.pending.take().unwrap_or_default();
        f.steps.push(format!("{name}{suffix}"));
    });
}

/// A pass of loop `id`. The steps are kept as tokens: `{id:name` opens the loop's first pass, `|id:name` each later
/// one, and `}id` closes the loop; the paths are made from them afterwards.
pub fn pass(id: u32, name: &str) {
    with(|f| {
        let suffix = f.pending.take().unwrap_or_default();
        if f.loops.iter().any(|l| l.0 == id) {
            f.steps.push(format!("|{id}:{name}{suffix}"));
        } else {
            f.loops.push((id, 0, 1));
            f.steps.push(format!("{{{id}:{name}{suffix}"));
        }
    });
}

/// Loop `id` has ended: when it never ran, its `else`, if it has one with no block.
pub fn end_loop(id: u32, otherwise: Option<&str>) {
    with(|f| {
        let suffix = f.pending.take().unwrap_or_default();
        if let Some(at) = f.loops.iter().position(|l| l.0 == id) {
            f.loops.truncate(at);
            f.steps.push(format!("}}{id}"));
        } else if let Some(name) = otherwise {
            f.steps.push(format!("{name}{suffix}"));
        }
    });
}

/// A predicate: `holds` is its value; what its operands showed becomes the next step's suffix.
pub fn cond(holds: bool) -> bool {
    with(|f| {
        let parts = f.building.pop().unwrap_or_default();
        f.pending = Some(render(&parts, holds));
    });
    holds
}

/// Starts recording a predicate.
pub fn begin() {
    with(|f| {
        f.building.push(Vec::new());
        f.operand.push(0);
    });
}

fn render(parts: &[(usize, Option<&'static str>)], _holds: bool) -> String {
    let mut out = String::new();
    for (k, order) in parts {
        if *k == 0 {
            if let Some(o) = order {
                out.push_str(&format!("'{o}"));
            }
        } else {
            out.push_str(&format!("'{k}{}", order.unwrap_or("")));
        }
    }
    out
}

/// A chain of operands joined by `&&` (`all` true) or `||`: evaluates them in order, and records where it stopped.
pub fn chain(and: bool, operands: &mut [&mut dyn FnMut() -> bool]) -> bool {
    let n = operands.len();
    let mut result = and;
    let mut evaluated = Vec::new();
    for (k, op) in operands.iter_mut().enumerate() {
        with(|f| *f.operand.last_mut().unwrap() = k + 1);
        let before = with(|f| f.building.last().map_or(0, |b| b.len())).unwrap_or(0);
        let v = op();
        let order = with(|f| {
            let b = f.building.last_mut().unwrap();
            let o = if b.len() > before { b.last().and_then(|p| p.1) } else { None };
            b.truncate(before);
            o
        })
        .flatten();
        evaluated.push((k + 1, order));
        if v != and {
            result = !and;
            with(|f| f.building.last_mut().unwrap().push((k + 1, order)));
            with(|f| *f.operand.last_mut().unwrap() = 0);
            return result;
        }
    }
    let _ = n;
    with(|f| {
        let b = f.building.last_mut().unwrap();
        for (k, o) in evaluated {
            if o.is_some() {
                b.push((k, o));
            }
        }
        *f.operand.last_mut().unwrap() = 0;
    });
    result
}

pub struct Probe<'a, T: ?Sized>(pub &'a T, pub &'a T);

pub trait ViaOrder {
    fn order(&self) -> Option<Ordering>;
    fn equal(&self) -> bool;
}

impl<T: PartialOrd + ?Sized> ViaOrder for Probe<'_, T> {
    fn order(&self) -> Option<Ordering> {
        self.0.partial_cmp(self.1)
    }
    fn equal(&self) -> bool {
        self.0 == self.1
    }
}

pub trait ViaEquality {
    fn order(&self) -> Option<Ordering>;
    fn equal(&self) -> bool;
}

impl<T: PartialEq + ?Sized> ViaEquality for &Probe<'_, T> {
    fn order(&self) -> Option<Ordering> {
        None
    }
    fn equal(&self) -> bool {
        self.0 == self.1
    }
}

/// Records the order of a comparison's operands, when they have one, and gives the comparison's value.
pub fn compared(order: Option<Ordering>, equal: bool, op: &str) -> bool {
    let name = order.map(|o| match o {
        Ordering::Less => "lt",
        Ordering::Equal => "eq",
        Ordering::Greater => "gt",
    });
    with(|f| {
        if let Some(b) = f.building.last_mut() {
            b.push((0, name));
        }
    });
    match (op, order) {
        ("==", _) => equal,
        ("!=", _) => !equal,
        ("<", Some(o)) => o == Ordering::Less,
        ("<=", Some(o)) => o != Ordering::Greater,
        (">", Some(o)) => o == Ordering::Greater,
        (">=", Some(o)) => o != Ordering::Less,
        _ => false,
    }
}

/// The order a `compare` found, for its arm's step.
pub fn order(o: Ordering) -> Ordering {
    with(|f| {
        f.pending = Some(match o {
            Ordering::Less => "'lt",
            Ordering::Equal => "'eq",
            Ordering::Greater => "'gt",
        }
        .to_string())
    });
    o
}

/// A call that may fail with `or fail`: records the failure as a step of its own.
pub fn attempt<T, E>(r: Result<T, E>, name: &str) -> Result<T, E> {
    if r.is_err() {
        with(|f| f.steps.push(format!("{name}'fails")));
    }
    r
}

/// A named outcome that no predicate splits: drops whatever an earlier predicate left.
pub fn step_plain(name: &str) {
    with(|f| {
        f.pending = None;
        f.steps.push(name.to_string());
    });
}

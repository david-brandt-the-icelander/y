//! The journal: what makes commands, outcomes and workflows durable (Y § 3.10). A stand-in for Y's durable command log
//! and workflow store, small enough to read, that keeps their meaning across a crash and across processes.
//!
//! - **Where it is.** With the environment variable `Y_JOURNAL` naming a file, the journal is that file: read when it is
//!   first used, and each record appended and flushed as it is made, so it outlives the process. Without it, the journal
//!   is kept per thread, in memory, which is what a test wants: it outlives a crash that a test forces
//!   ([`crate::faults::restarting`]), and not the test.
//! - **A workflow's identity** is its name and its arguments, written as data (`orders::fulfil(7,20)`); its **version**
//!   is a hash of its body, which the compiler gives it. The journal keeps the identity and the version, never an
//!   address.
//! - **Each durable step** of a running workflow is numbered in order: each `submit` and each atomic region it opens.
//!   A step's value is recorded when it is made (a region's when it commits, a submit's when its outcome is known), and a
//!   workflow that runs again with the same identity resumes: a step already recorded gives its recorded value and is
//!   not made again. A submitted command is recorded before it is delivered; one delivered and not yet answered when the
//!   process crashed is delivered again (at-least-once delivery, exactly-once intent).
//! - **A workflow that finished** gives its recorded result, and does not run again.
//! - **A workflow recorded under another version** cannot resume: that is a fault naming both versions. The proposal's
//!   compatibility signatures (§ 4.4), which would let a compatible version resume, are not kept.
//!
//! What it does not keep: separate workers (a command is delivered in the workflow's own thread), timers, and the time
//! of a workflow's last durable step.
use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// A value the journal can write and read back: tokens of text, in order.
pub trait Durable: Sized {
    fn put(&self, out: &mut Vec<String>);
    fn take(input: &mut Tokens<'_>) -> Option<Self>;
}

/// The tokens a value is read from.
pub struct Tokens<'a> {
    items: &'a [String],
    at: usize,
}

impl<'a> Tokens<'a> {
    pub fn new(items: &'a [String]) -> Self {
        Tokens { items, at: 0 }
    }

    pub fn next(&mut self) -> Option<&'a str> {
        let t = self.items.get(self.at)?;
        self.at += 1;
        Some(t)
    }

    /// A case's number, written by [`case`].
    pub fn case(&mut self) -> Option<u32> {
        self.next()?.parse().ok()
    }
}

/// Writes a case's number, for a closed type.
pub fn case(out: &mut Vec<String>, k: u32) {
    out.push(k.to_string());
}

/// A value's tokens.
pub fn tokens<T: Durable>(v: &T) -> Vec<String> {
    let mut out = vec![];
    v.put(&mut out);
    out
}

/// A value read from its tokens, all of them.
pub fn value<T: Durable>(items: &[String]) -> Option<T> {
    let mut t = Tokens::new(items);
    let v = T::take(&mut t)?;
    (t.at == items.len()).then_some(v)
}

macro_rules! durable_number {
    ($($t:ty),*) => {$(
        impl Durable for $t {
            fn put(&self, out: &mut Vec<String>) {
                out.push(self.to_string());
            }
            fn take(input: &mut Tokens<'_>) -> Option<Self> {
                input.next()?.parse().ok()
            }
        }
    )*};
}
durable_number!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, usize, bool);

impl Durable for String {
    fn put(&self, out: &mut Vec<String>) {
        out.push(self.clone());
    }
    fn take(input: &mut Tokens<'_>) -> Option<Self> {
        input.next().map(str::to_string)
    }
}

// nothing, the value of a region that gives none: written by a macro, as bare-rust's strip reads no impl for `()`
macro_rules! durable_unit {
    () => {
        impl Durable for () {
            fn put(&self, _: &mut Vec<String>) {}
            fn take(_: &mut Tokens<'_>) -> Option<Self> {
                Some(())
            }
        }
    };
}
durable_unit!();

impl<T: Durable> Durable for Option<T> {
    fn put(&self, out: &mut Vec<String>) {
        match self {
            None => case(out, 0),
            Some(v) => {
                case(out, 1);
                v.put(out);
            }
        }
    }
    fn take(input: &mut Tokens<'_>) -> Option<Self> {
        match input.case()? {
            0 => Some(None),
            1 => Some(Some(T::take(input)?)),
            _ => None,
        }
    }
}

impl<T: Durable, E: Durable> Durable for Result<T, E> {
    fn put(&self, out: &mut Vec<String>) {
        match self {
            Ok(v) => {
                case(out, 0);
                v.put(out);
            }
            Err(e) => {
                case(out, 1);
                e.put(out);
            }
        }
    }
    fn take(input: &mut Tokens<'_>) -> Option<Self> {
        match input.case()? {
            0 => Some(Ok(T::take(input)?)),
            1 => Some(Err(E::take(input)?)),
            _ => None,
        }
    }
}

impl<T: Durable> Durable for Vec<T> {
    fn put(&self, out: &mut Vec<String>) {
        out.push(self.len().to_string());
        for v in self {
            v.put(out);
        }
    }
    fn take(input: &mut Tokens<'_>) -> Option<Self> {
        let n: usize = input.next()?.parse().ok()?;
        (0..n).map(|_| T::take(input)).collect()
    }
}

/// One record: the workflow it belongs to, the version it ran under, its step, what it is, and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub workflow: String,
    pub version: String,
    pub step: u32,
    /// `command` (a submitted command, before delivery), `submit` (its outcome), `region` (a committed atomic
    /// region's value), or `result` (the workflow's own).
    pub kind: String,
    pub data: Vec<String>,
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

impl Record {
    fn line(&self) -> String {
        let mut parts = vec![escape(&self.workflow), escape(&self.version), self.step.to_string(), escape(&self.kind)];
        parts.extend(self.data.iter().map(|d| escape(d)));
        parts.join("\t")
    }

    fn parse(line: &str) -> Option<Record> {
        let parts: Vec<String> = line.split('\t').map(unescape).collect();
        if parts.len() < 4 {
            return None;
        }
        Some(Record { workflow: parts[0].clone(), version: parts[1].clone(), step: parts[2].parse().ok()?, kind: parts[3].clone(), data: parts[4..].to_vec() })
    }
}

thread_local! {
    static MEMORY: RefCell<Vec<Record>> = const { RefCell::new(Vec::new()) };
    /// The workflows running on this thread, innermost last: identity, version, and the steps made so far.
    static ACTIVE: RefCell<Vec<(String, String, u32)>> = const { RefCell::new(Vec::new()) };
    /// A crash a test forced after a region committed, made once the region's step is recorded.
    static CRASH: RefCell<Option<String>> = const { RefCell::new(None) };
    static OUTSIDE: Cell<u32> = const { Cell::new(0) };
}

/// The file journal, when `Y_JOURNAL` names one: its path and what it holds.
static FILE: Mutex<Option<(PathBuf, Vec<Record>)>> = Mutex::new(None);

fn file_path() -> Option<PathBuf> {
    std::env::var_os("Y_JOURNAL").filter(|p| !p.is_empty()).map(PathBuf::from)
}

/// Every record, in the order it was made.
pub fn records() -> Vec<Record> {
    match file_path() {
        Some(path) => {
            let mut f = FILE.lock().unwrap_or_else(|p| p.into_inner());
            if f.as_ref().is_none_or(|(p, _)| *p != path) {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                *f = Some((path, text.lines().filter_map(Record::parse).collect()));
            }
            f.as_ref().map(|(_, r)| r.clone()).unwrap_or_default()
        }
        None => MEMORY.with(|m| m.borrow().clone()),
    }
}

fn append(r: Record) {
    match file_path() {
        Some(path) => {
            let _ = records();
            let mut f = FILE.lock().unwrap_or_else(|p| p.into_inner());
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap_or_else(|e| crate::fault("journal", &format!("{}: {e}", path.display())));
            let _ = writeln!(file, "{}", r.line());
            let _ = file.flush();
            if let Some((_, rs)) = f.as_mut() {
                rs.push(r);
            }
        }
        None => MEMORY.with(|m| m.borrow_mut().push(r)),
    }
}

/// Forgets every record kept in memory on this thread: a test that wants a journal of its own starts with this.
pub fn clear() {
    MEMORY.with(|m| m.borrow_mut().clear());
}

/// The identity of a running workflow, and its next step; none outside a workflow, and none while a handler runs.
fn next_step() -> Option<(String, String, u32)> {
    if OUTSIDE.with(Cell::get) > 0 {
        return None;
    }
    ACTIVE.with(|a| {
        let mut a = a.borrow_mut();
        let top = a.last_mut()?;
        top.2 += 1;
        Some(top.clone())
    })
}

/// Whether a workflow is running here, whose steps are recorded.
pub fn in_workflow() -> bool {
    OUTSIDE.with(Cell::get) == 0 && ACTIVE.with(|a| !a.borrow().is_empty())
}

fn recorded(id: &str, version: &str, step: u32, kind: &str) -> Option<Vec<String>> {
    records().into_iter().find(|r| r.workflow == id && r.version == version && r.step == step && r.kind == kind).map(|r| r.data)
}

/// Pops the workflow when its run ends, by returning or by a crash.
struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.borrow_mut().pop());
    }
}

/// Runs a workflow (Y § 3.10): its recorded result when it finished; otherwise its body, resuming from the steps the
/// journal holds for its identity.
pub fn workflow<R: Durable>(name: &str, version: &str, identity: Vec<String>, body: impl FnOnce() -> R) -> R {
    let id = format!("{name}({})", identity.join(","));
    let mine: Vec<Record> = records().into_iter().filter(|r| r.workflow == id).collect();
    if let Some(r) = mine.iter().find(|r| r.kind == "result") {
        match value(&r.data) {
            Some(v) => return v,
            None => crate::fault(name, &format!("the journal's result of {id} does not read as the workflow's result")),
        }
    }
    if let Some(r) = mine.iter().find(|r| r.version != version) {
        crate::fault(name, &format!("{id} was recorded under version {} and cannot resume under version {version}", r.version));
    }
    ACTIVE.with(|a| a.borrow_mut().push((id.clone(), version.to_string(), 0)));
    let running = Running;
    let v = body();
    drop(running);
    append(Record { workflow: id, version: version.to_string(), step: 0, kind: "result".into(), data: tokens(&v) });
    v
}

/// A durable step that gives a value: its recorded value when the journal has it, otherwise `run`'s, recorded.
pub fn step<T: Durable>(kind: &str, run: impl FnOnce() -> T) -> T {
    let Some((id, version, n)) = next_step() else { return run() };
    if let Some(data) = recorded(&id, &version, n, kind) {
        return value(&data).unwrap_or_else(|| crate::fault(&id, &format!("step {n} of the journal does not read as its value")));
    }
    let v = run();
    append(Record { workflow: id, version, step: n, kind: kind.into(), data: tokens(&v) });
    v
}

/// An atomic region a workflow opens: a step recorded when it commits, and given again when the workflow resumes. A
/// region that did not commit records nothing, and runs again.
pub fn region<T: Durable, X>(run: impl FnOnce() -> Result<T, X>) -> Result<T, X> {
    let Some((id, version, n)) = next_step() else { return run() };
    if let Some(data) = recorded(&id, &version, n, "region") {
        return Ok(value(&data).unwrap_or_else(|| crate::fault(&id, &format!("step {n} of the journal does not read as the region's value"))));
    }
    let r = run();
    if let Ok(v) = &r {
        append(Record { workflow: id, version, step: n, kind: "region".into(), data: tokens(v) });
    }
    if let Some(at) = CRASH.with(|c| c.borrow_mut().take()) {
        crate::faults::crash(&at);
    }
    r
}

/// Records a command a workflow submits, in the step that submits it, before it is delivered: its key and its fields.
pub fn command<C: Durable>(key: &str, command: &C) {
    if let Some((id, version, n)) = ACTIVE.with(|a| a.borrow().last().cloned()).filter(|_| OUTSIDE.with(Cell::get) == 0) {
        let mut data = vec![key.to_string()];
        command.put(&mut data);
        append(Record { workflow: id, version, step: n, kind: "command".into(), data });
    }
}

/// Runs a handler, which is no step of the workflow that submitted its command.
pub fn outside<T>(run: impl FnOnce() -> T) -> T {
    struct Back;
    impl Drop for Back {
        fn drop(&mut self) {
            OUTSIDE.with(|o| o.set(o.get() - 1));
        }
    }
    OUTSIDE.with(|o| o.set(o.get() + 1));
    let back = Back;
    let v = run();
    drop(back);
    v
}

/// A crash forced after a region committed: at once outside a workflow; inside one, once the region's step is recorded,
/// as a runtime that records the step in the region's own commit would.
pub fn crash_after_commit(at: &str) {
    if in_workflow() {
        CRASH.with(|c| *c.borrow_mut() = Some(at.to_string()));
    } else {
        crate::faults::crash(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_written_and_read_back() {
        let v: Result<Option<u32>, Vec<String>> = Err(vec!["a\tb".into(), "c\\".into()]);
        assert_eq!(value::<Result<Option<u32>, Vec<String>>>(&tokens(&v)), Some(v.clone()));
        let r = Record { workflow: "m::w(1)".into(), version: "v1".into(), step: 2, kind: "submit".into(), data: tokens(&v) };
        assert_eq!(Record::parse(&r.line()), Some(r));
    }

    #[test]
    fn a_workflow_resumes_from_its_recorded_steps() {
        clear();
        let made = Cell::new(0);
        let run = || {
            workflow("m::w", "v1", vec!["7".into()], || {
                let a: u32 = step("submit", || {
                    made.set(made.get() + 1);
                    40
                });
                let b: Result<u32, ()> = region(|| {
                    made.set(made.get() + 1);
                    Ok(2)
                });
                a + b.unwrap()
            })
        };
        assert_eq!(run(), 42);
        assert_eq!(made.get(), 2);
        // finished: its result, and nothing made again
        assert_eq!(run(), 42);
        assert_eq!(made.get(), 2);
        // a step recorded and the workflow not finished: the step is given, the rest made
        clear();
        append(Record { workflow: "m::w(7)".into(), version: "v1".into(), step: 1, kind: "submit".into(), data: tokens(&40u32) });
        assert_eq!(run(), 42);
        assert_eq!(made.get(), 3);
    }

    #[test]
    #[should_panic(expected = "cannot resume under version v2")]
    fn a_workflow_does_not_resume_under_another_version() {
        clear();
        append(Record { workflow: "m::w(7)".into(), version: "v1".into(), step: 1, kind: "submit".into(), data: tokens(&40u32) });
        workflow("m::w", "v2", vec!["7".into()], || step("submit", || 1u32));
    }
}

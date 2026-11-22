//! Y types, as far as the lowering needs them, and the Rust types they lower to.
use y_parse::Node;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    U32,
    I32,
    U64,
    U16,
    /// `count<unit_type=…>`, and the result of a collection's `count()`: `usize`.
    Count,
    /// An integer literal whose type the context decides.
    Int,
    Bool,
    /// Y's `string`: `YString`.
    Str,
    /// Host text: a `String` the runtime gives (a line read from the console), or the `format!` of an interpolation.
    HostStr,
    /// A string literal: `&'static str`.
    StrLit,
    Rune,
    List(Box<Ty>),
    /// A constant group's list, which lowers to a Rust array of the given length.
    Array(Box<Ty>, usize),
    Set(Box<Ty>),
    Opt(Box<Ty>),
    /// What a loop can run over once: `runes()`, `split(…)`.
    Iter(Box<Ty>),
    Res(Box<Ty>, Box<Ty>),
    /// A type the module declares, or one the runtime gives (`console_error`), by its Y name.
    Named(String),
    /// A capability: `console`, `forever`, `random`, `system`, `clock`.
    Cap(String),
    /// `decimal<precision=p, scale=s>`; a derived result without a precision has 38.
    Decimal(u32, u32),
    /// `integer<minimum=a, maximum=b>`.
    Integer(i128, i128),
    /// An exact fraction, from a value of the given precision.
    Rational(u32),
    /// `float64`.
    F64,
    /// `order.created`: a value of a `state` known to be in one of its states.
    StateCase(String, String),
    /// `enqueue<command_type=c>`'s outbox, held in shared state: the commands an atomic region submits.
    Outbox(Box<Ty>),
    /// `submit<command_type=c>`: where a workflow submits a command and waits for its handler's outcome.
    Submit(Box<Ty>),
    /// `undelivered<failure_type=e>`: why a submitted command has no outcome.
    Undelivered(Box<Ty>),
    /// `sensitive<value_type=t>`.
    Sensitive(Box<Ty>),
    /// A kind of collection or mapping the module declares, with its type arguments.
    Kind(String, Vec<(String, Ty)>),
    /// `string<character_set=ascii>`.
    Ascii,
    Bag(Box<Ty>),
    Queue(Box<Ty>),
    SortedMap(Box<Ty>, Box<Ty>),
    /// Another of `y_collections`' kinds (`SortedSet`, `Stack`, …), by its Rust name, with its type arguments in order:
    /// its methods are the crate's own.
    Coll(&'static str, Vec<Ty>),
    /// `shared<value_type=t>`: state that tasks share, changed in atomic regions.
    Shared(Box<Ty>),
    /// `store<schema_type=t>`.
    Store(Box<Ty>),
    /// `index<unit_type=t>`, into an arena.
    Index(Box<Ty>),
    Arena(Box<Ty>),
    Unit,
    Unknown,
}

/// The Rust integer that holds a decimal of a precision, or an integer of a range.
pub fn digits(precision: u32) -> &'static str {
    match precision {
        0..=4 => "i16",
        5..=9 => "i32",
        10..=18 => "i64",
        19..=38 => "i128",
        39..=76 => "I256",
        _ => "Big",
    }
}

/// The precision of a decimal that holds any number of digits: the runtime's `UNBOUNDED`.
pub const UNBOUNDED: u32 = u32::MAX;

/// The derived precision of a result, at most what the runtime's largest fixed size holds, and unbounded past it.
pub fn capped(p: u32) -> u32 {
    if p > 76 { UNBOUNDED } else { p.max(1) }
}

/// How many decimal digits the largest magnitude of a range has.
pub fn range_precision(lo: i128, hi: i128) -> u32 {
    let m = lo.unsigned_abs().max(hi.unsigned_abs());
    (m.checked_ilog10().unwrap_or(0) + 1).max(1)
}

pub fn range_digits(lo: i128, hi: i128) -> &'static str {
    if lo >= i8::MIN as i128 && hi <= i8::MAX as i128 {
        "i8"
    } else if lo >= i16::MIN as i128 && hi <= i16::MAX as i128 {
        "i16"
    } else if lo >= i32::MIN as i128 && hi <= i32::MAX as i128 {
        "i32"
    } else if lo >= i64::MIN as i128 && hi <= i64::MAX as i128 {
        "i64"
    } else {
        "i128"
    }
}

pub const CAPABILITIES: [&str; 8] = ["console", "forever", "random", "system", "clock", "window", "http", "reveal"];

/// Types the runtime gives whose values are copied, as numbers are: passed by value.
pub const COPIED: [&str; 5] = ["colour", "key", "instant", "duration", "point"];

/// Types the runtime gives, by their Y name: the Rust name and the crate that has it.
pub fn external(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "console_error" => ("ConsoleError", "y_runtime"),
        "console" => ("Console", "y_runtime"),
        "forever" => ("Forever", "y_runtime"),
        "random" => ("Random", "y_runtime"),
        "system" => ("System", "y_runtime"),
        "clock" => ("Clock", "y_runtime"),
        "constraint_violation" => ("ConstraintViolation", "y_runtime"),
        "window" => ("Window", "y_window"),
        "colour" => ("Colour", "y_window"),
        "key" => ("Key", "y_window"),
        "point" => ("Point", "y_window"),
        "http" => ("Http", "y_http"),
        "http_error" => ("HttpError", "y_http"),
        "expired" => ("Expired", "y_runtime"),
        "reveal" => ("Reveal", "y_runtime::claims"),
        "abandonment" => ("Abandonment", "y_runtime::command"),
        "inexact" => ("Inexact", "y_runtime::decimal"),
        "picture" => ("Picture", "y_window"),
        "picture_error" => ("PictureError", "y_window"),
        "frame" => ("Frame", "y_window"),
        "store_error" => ("StoreError", "y_runtime::store"),
        "record_failure" => ("RecordFailure", "y_runtime::store"),
        "instant" => ("Instant", "std::time"),
        "duration" => ("Duration", "std::time"),
        _ => return None,
    })
}

/// `lower_snake_case` to `UpperCamelCase`, the lowering's one renaming.
pub fn pascal(name: &str) -> String {
    name.split('_').filter(|p| !p.is_empty()).map(|p| {
        let mut c = p.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect()
}

pub fn upper(name: &str) -> String {
    name.to_uppercase()
}

impl Ty {
    /// The type a `type` node of the tree names.
    pub fn from_node(t: &Node) -> Ty {
        let name = t.str("name").unwrap_or("");
        let arg = |k: &str| t.nodes("args").into_iter().find(|a| a.str("name") == Some(k)).and_then(|a| a.node("value")).map(Ty::from_node);
        match name {
            "uint32" => Ty::U32,
            "int32" => Ty::I32,
            "uint64" => Ty::U64,
            "uint16" => Ty::U16,
            "count" => Ty::Count,
            "boolean" => Ty::Bool,
            "float64" => Ty::F64,
            "rational" => Ty::Rational(UNBOUNDED),
            "string" if !t.nodes("args").is_empty() => Ty::Ascii,
            "string" => Ty::Str,
            "rune" => Ty::Rune,
            "list" => Ty::List(Box::new(arg("element_type").unwrap_or(Ty::Unknown))),
            "set" => Ty::Set(Box::new(arg("element_type").unwrap_or(Ty::Unknown))),
            "optional" => Ty::Opt(Box::new(arg("value_type").unwrap_or(Ty::Unknown))),
            "decimal" => {
                let num = |k: &str| t.nodes("args").into_iter().find(|a| a.str("name") == Some(k)).and_then(|a| a.str("text")).and_then(|x| x.parse::<u32>().ok());
                Ty::Decimal(num("precision").unwrap_or(38), num("scale").unwrap_or(0))
            }
            "integer" => {
                let num = |k: &str| t.nodes("args").into_iter().find(|a| a.str("name") == Some(k)).and_then(|a| a.str("text")).and_then(|x| x.parse::<i128>().ok());
                Ty::Integer(num("minimum").unwrap_or(i64::MIN as i128), num("maximum").unwrap_or(i64::MAX as i128))
            }
            "bag" => Ty::Bag(Box::new(arg("element_type").unwrap_or(Ty::Unknown))),
            "queue" => Ty::Queue(Box::new(arg("element_type").unwrap_or(Ty::Unknown))),
            n if coll_kind(n).is_some() => Ty::Coll(coll_kind(n).unwrap().0, ["element_type", "key_type", "value_type"].iter().filter_map(|k| arg(k)).collect()),
            "sorted_map" => Ty::SortedMap(Box::new(arg("key_type").unwrap_or(Ty::Unknown)), Box::new(arg("value_type").unwrap_or(Ty::Unknown))),
            "shared" => Ty::Shared(Box::new(arg("value_type").unwrap_or(Ty::Unknown))),
            "store" => Ty::Store(Box::new(arg("schema_type").unwrap_or(Ty::Unknown))),
            "index" => Ty::Index(Box::new(arg("unit_type").unwrap_or(Ty::Unknown))),
            "result" => Ty::Res(Box::new(arg("value_type").unwrap_or(Ty::Unit)), Box::new(arg("failure_type").unwrap_or(Ty::Unknown))),
            "enqueue" => Ty::Shared(Box::new(Ty::Outbox(Box::new(arg("command_type").unwrap_or(Ty::Unknown))))),
            "submit" => Ty::Submit(Box::new(arg("command_type").unwrap_or(Ty::Unknown))),
            "undelivered" => Ty::Undelivered(Box::new(arg("failure_type").unwrap_or(Ty::Unknown))),
            "sensitive" => Ty::Sensitive(Box::new(arg("value_type").unwrap_or(Ty::Unknown))),
            n if CAPABILITIES.contains(&n) => Ty::Cap(n.to_string()),
            n if n.contains('.') => {
                let (st, case) = n.split_once('.').unwrap();
                Ty::StateCase(st.to_string(), case.to_string())
            }
            n if !t.nodes("args").is_empty() => Ty::Kind(n.to_string(), t.nodes("args").iter().map(|a| (a.str("name").unwrap_or("").to_string(), a.node("value").map(Ty::from_node).unwrap_or(Ty::Unknown))).collect()),
            n => Ty::Named(n.to_string()),
        }
    }

    pub fn elem(&self) -> Ty {
        match self {
            Ty::List(t) | Ty::Set(t) | Ty::Array(t, _) | Ty::Opt(t) | Ty::Iter(t) | Ty::Bag(t) | Ty::Queue(t) | Ty::Shared(t) | Ty::Store(t) | Ty::Arena(t) => (**t).clone(),
            Ty::Coll(_, a) if a.len() == 1 => a[0].clone(),
            _ => Ty::Unknown,
        }
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Ty::U32 | Ty::I32 | Ty::U64 | Ty::U16 | Ty::Count | Ty::Int)
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Ty::Str | Ty::HostStr | Ty::StrLit)
    }
}

/// The kinds of `y_collections` without a type of their own here: Y name, Rust name, Rust path.
pub const COLL_KINDS: &[(&str, &str, &str)] = &[
    ("stack", "Stack", "y_collections::Stack"),
    ("deque", "Deque", "y_collections::Deque"),
    ("sorted_list", "SortedList", "y_collections::SortedList"),
    ("sorted_set", "SortedSet", "y_collections::SortedSet"),
    ("unordered_set", "UnorderedSet", "y_collections::UnorderedSet"),
    ("unordered_map", "UnorderedMap", "y_collections::UnorderedMap"),
    ("multimap", "MultiMap", "y_collections::MultiMap"),
    ("map", "Map", "y_collections::Map"),
];

/// A kind of `COLL_KINDS`, by its Y name or its Rust name: (Rust name, Y name, Rust path).
pub fn coll_kind(n: &str) -> Option<(&'static str, &'static str, &'static str)> {
    COLL_KINDS.iter().find(|(y, r, _)| *y == n || *r == n).map(|(y, r, p)| (*r, *y, *p))
}

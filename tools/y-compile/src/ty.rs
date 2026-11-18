//! Y types, as far as the lowering needs them, and the Rust types they lower to.
use y_parse::Node;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    U32,
    I32,
    U64,
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
    Unit,
    Unknown,
}

pub const CAPABILITIES: [&str; 6] = ["console", "forever", "random", "system", "clock", "window"];

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
            "count" => Ty::Count,
            "boolean" => Ty::Bool,
            "string" => Ty::Str,
            "rune" => Ty::Rune,
            "list" => Ty::List(Box::new(arg("element_type").unwrap_or(Ty::Unknown))),
            "set" => Ty::Set(Box::new(arg("element_type").unwrap_or(Ty::Unknown))),
            "optional" => Ty::Opt(Box::new(arg("value_type").unwrap_or(Ty::Unknown))),
            "result" => Ty::Res(Box::new(arg("value_type").unwrap_or(Ty::Unit)), Box::new(arg("failure_type").unwrap_or(Ty::Unknown))),
            n if CAPABILITIES.contains(&n) => Ty::Cap(n.to_string()),
            n => Ty::Named(n.to_string()),
        }
    }

    pub fn elem(&self) -> Ty {
        match self {
            Ty::List(t) | Ty::Set(t) | Ty::Array(t, _) | Ty::Opt(t) | Ty::Iter(t) => (**t).clone(),
            _ => Ty::Unknown,
        }
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Ty::U32 | Ty::I32 | Ty::U64 | Ty::Count | Ty::Int)
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Ty::Str | Ty::HostStr | Ty::StrLit)
    }
}

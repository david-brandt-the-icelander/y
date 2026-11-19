//! A parser for Y (`proposal/Y-proposal.md`), as far as the repository's examples and the compiler's test programs
//! use it: their modules, `main` files and test files. It makes a tree of [`Node`]s, each with the line it starts on;
//! [`Node::to_json`] writes the tree for tools in other languages. See the README for the forms it accepts.
mod lex;
mod parse;
mod tree;

pub use tree::{Node, Value};

/// Why a source could not be parsed, and the line where it went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl Error {
    pub fn new(line: usize, message: &str) -> Error {
        Error { line, message: message.to_string() }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for Error {}

/// Parses a Y source: the tree of a `file` node, whose `decls` are its declarations and whose `comments` are all its
/// comments.
pub fn parse(src: &str) -> Result<Node, Error> {
    parse::parse(src)
}

/// Reads and parses a Y file. An error names the file and the line.
pub fn parse_file(path: &std::path::Path) -> Result<Node, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // a test file belongs to its module (Y § 3.14), and writes the quantities of its module's units
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let units = match name.strip_suffix(".test.y") {
        Some(m) => std::fs::read_to_string(path.with_file_name(format!("{m}.y"))).map(|s| parse::unit_symbols(&s)).unwrap_or_default(),
        None => vec![],
    };
    parse::parse_with_units(&src, &units).map_err(|e| format!("{}:{}: {}", path.display(), e.line, e.message))
}

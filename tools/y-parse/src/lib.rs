//! A parser for Y (`proposal/Y-proposal.md`), as far as the repository's examples use it: their modules, `main` files
//! and test files. It makes a tree of [`Node`]s, each with the line it starts on; [`Node::to_json`] writes the tree for
//! tools in other languages. See the README for the forms it accepts.
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
    parse(&src).map_err(|e| format!("{}:{}: {}", path.display(), e.line, e.message))
}

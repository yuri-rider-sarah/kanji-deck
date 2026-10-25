//! Errors in a text, with a position to report them at. The executables add the file's path.

use crate::parser::Pos;

#[derive(Debug, Clone)]
pub struct Error {
    pub line: usize,
    pub column: Option<usize>,
    pub message: String,
}

impl Error {
    /// An error at a position in the file.
    pub fn new(pos: Pos, message: impl Into<String>) -> Error {
        Error { line: pos.line, column: Some(pos.column), message: message.into() }
    }

    /// An error on a line of the file.
    pub fn at_line(line: usize, message: impl Into<String>) -> Error {
        Error { line, column: None, message: message.into() }
    }

    pub fn unexpected_node(pos: Pos, head: &str) -> Error {
        Error::new(pos, format!("unexpected {head}"))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

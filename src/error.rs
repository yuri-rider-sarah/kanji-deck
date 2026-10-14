//! Errors in a file, with a position to report them at.

use crate::parser::Pos;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Error {
    pub line: usize,
    pub column: Option<usize>,
    pub message: String,
}

impl Error {
    /// An error at a position in the file.
    pub(crate) fn new(pos: Pos, message: impl Into<String>) -> Error {
        Error { line: pos.line, column: Some(pos.column), message: message.into() }
    }

    /// An error on a line of the file.
    pub(crate) fn at_line(line: usize, message: impl Into<String>) -> Error {
        Error { line, column: None, message: message.into() }
    }

    pub(crate) fn unexpected_node(pos: Pos, head: &str) -> Error {
        Error::new(pos, format!("unexpected {head}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.column {
            Some(column) => write!(f, "{}:{column}: {}", self.line, self.message),
            None => write!(f, "{}: {}", self.line, self.message),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

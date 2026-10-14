//! Parses text files into trees of nodes, without interpreting what they
//! mean. The syntax is described in `read`. Callers go through the nodes with
//! `Args`.

mod lex;
mod read;

pub use read::{Args, parse};
use std::fmt;

/// A position in a file. Both the line and the column are counted from 1, and
/// the column is counted in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

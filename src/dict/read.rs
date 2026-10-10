//! Reads the tokens from `lex` into a tree of nodes, without interpreting what
//! they mean. The parser accesses the tree through `Args`.
//!
//! ```text
//! file         = { record } ;
//! record       = STRING, { node }, { segment }, NEWLINE,
//!                [ INDENT, { continuation }, { record }, DEDENT ]
//!              | node, NEWLINE ;
//! continuation = segment, { segment }, NEWLINE ;
//! segment      = "|", short record | RECORD_STRING ;
//! short record = STRING, { node } | node ;
//! node         = STRING, [ ":", node | "(", { node }, ")" ] ;
//! ```
//!
//! Every construct is a node with a name and arguments: `a b c` and `a(b c)`
//! are both `a` with the arguments `b` and `c`, `a:b` is `a(b)`, and `a` alone
//! is `a()`. A record's arguments are its own nodes, then its segments and
//! continuation lines, then the records indented under it.

use super::Error;
use super::lex::{Kind, Token, tokenize};
use std::collections::VecDeque;
use std::iter::Peekable;

struct Node {
    line: usize,
    name: String,
    args: Vec<Node>,
}

fn err(line: usize, message: impl Into<String>) -> Error {
    Error { line, message: message.into() }
}

/// Reads the text into its top-level records.
pub fn read(text: &str) -> Result<Args<'static>, Error> {
    let mut reader = Reader { tokens: tokenize(text)?.into_iter().peekable() };
    let mut nodes = VecDeque::new();
    while reader.tokens.peek().is_some() {
        nodes.push_back(reader.record()?);
    }
    Ok(Args { line: 0, head: "", nodes })
}

struct Reader {
    tokens: Peekable<std::vec::IntoIter<Token>>,
}

impl Reader {
    fn peek(&mut self) -> Option<&Kind> {
        self.tokens.peek().map(|t| &t.kind)
    }

    fn unexpected(&mut self) -> Error {
        let Some(token) = self.tokens.peek() else {
            return err(0, "unexpected end of file");
        };
        let record_string;
        let what = match &token.kind {
            Kind::Str(s) => s.as_str(),
            Kind::RecordStr(s) => {
                record_string = format!("|| {s}");
                &record_string
            }
            Kind::Open => "(",
            Kind::Close => ")",
            Kind::Colon => ":",
            Kind::Pipe => "|",
            Kind::Newline => "end of line",
            Kind::Indent => "indentation",
            Kind::Dedent => "unindentation",
        };
        err(token.line, format!("unexpected {what}"))
    }

    fn expect(&mut self, kind: Kind) -> Result<(), Error> {
        if self.peek() != Some(&kind) {
            return Err(self.unexpected());
        }
        self.tokens.next();
        Ok(())
    }

    fn string(&mut self) -> Result<(usize, String), Error> {
        match self.tokens.next_if(|t| matches!(t.kind, Kind::Str(_))) {
            Some(Token { line, kind: Kind::Str(s) }) => Ok((line, s)),
            _ => Err(self.unexpected()),
        }
    }

    /// Whether the next token starts a `":" node` or `"(" { node } ")"` suffix.
    fn at_suffix(&mut self) -> bool {
        matches!(self.peek(), Some(Kind::Colon | Kind::Open))
    }

    /// `node = STRING, [ ":", node | "(", { node }, ")" ]`
    fn node(&mut self) -> Result<Node, Error> {
        let (line, name) = self.string()?;
        self.suffix(line, name)
    }

    /// The rest of a node whose string has been read.
    fn suffix(&mut self, line: usize, name: String) -> Result<Node, Error> {
        let mut args = Vec::new();
        match self.peek() {
            Some(Kind::Colon) => {
                self.tokens.next();
                args.push(self.node()?);
            }
            Some(Kind::Open) => {
                self.tokens.next();
                while self.peek() != Some(&Kind::Close) {
                    args.push(self.node()?);
                }
                self.tokens.next();
            }
            _ => {}
        }
        Ok(Node { line, name, args })
    }

    /// `{ node }`
    fn nodes(&mut self) -> Result<Vec<Node>, Error> {
        let mut nodes = Vec::new();
        while matches!(self.peek(), Some(Kind::Str(_))) {
            nodes.push(self.node()?);
        }
        Ok(nodes)
    }

    /// `short record = STRING, { node } | node`
    fn short_record(&mut self) -> Result<Node, Error> {
        let (line, name) = self.string()?;
        if self.at_suffix() {
            return self.suffix(line, name);
        }
        Ok(Node { line, name, args: self.nodes()? })
    }

    /// `{ segment }`, where `segment = "|", short record | RECORD_STRING`
    fn segments(&mut self) -> Result<Vec<Node>, Error> {
        let mut segments = Vec::new();
        loop {
            match self.tokens.next_if(|t| matches!(t.kind, Kind::Pipe | Kind::RecordStr(_))) {
                Some(Token { kind: Kind::Pipe, .. }) => segments.push(self.short_record()?),
                Some(Token { line, kind: Kind::RecordStr(name) }) => segments.push(Node { line, name, args: Vec::new() }),
                _ => return Ok(segments),
            }
        }
    }

    fn record(&mut self) -> Result<Node, Error> {
        let (line, name) = self.string()?;
        if self.at_suffix() {
            let node = self.suffix(line, name)?;
            self.expect(Kind::Newline)?;
            if self.peek() == Some(&Kind::Indent) {
                return Err(err(line, "a record written as a single node can't have indented lines"));
            }
            return Ok(node);
        }
        let mut args = self.nodes()?;
        args.extend(self.segments()?);
        self.expect(Kind::Newline)?;
        if self.peek() == Some(&Kind::Indent) {
            self.tokens.next();
            while matches!(self.peek(), Some(Kind::Pipe | Kind::RecordStr(_))) {
                args.extend(self.segments()?);
                self.expect(Kind::Newline)?;
            }
            while matches!(self.peek(), Some(Kind::Str(_))) {
                args.push(self.record()?);
            }
            self.expect(Kind::Dedent)?;
        }
        Ok(Node { line, name, args })
    }
}

impl Node {
    /// Calls `f` with the name and arguments of this node. It's an error if
    /// `f` leaves any arguments unconsumed.
    fn take<T>(self, f: impl FnOnce(&str, &mut Args) -> Result<T, Error>) -> Result<T, Error> {
        let mut args = Args { line: self.line, head: &self.name, nodes: self.args.into() };
        let value = f(&self.name, &mut args)?;
        args.finish()?;
        Ok(value)
    }

    fn unexpected(&self) -> Error {
        if self.args.is_empty() {
            err(self.line, format!("unexpected {}", self.name))
        } else {
            err(self.line, format!("unexpected {}(…)", self.name))
        }
    }
}

/// The arguments of a node, consumed in order.
pub struct Args<'h> {
    pub line: usize,
    head: &'h str,
    nodes: VecDeque<Node>,
}

impl Args<'_> {
    fn take(&mut self) -> Result<Node, Error> {
        self.nodes.pop_front().ok_or_else(|| self.error(format!("missing item in {}", self.head)))
    }

    /// The name of the next node.
    fn next_name(&self) -> Option<&str> {
        self.nodes.front().map(|node| node.name.as_str())
    }

    fn finish(&self) -> Result<(), Error> {
        match self.nodes.front() {
            Some(node) => Err(node.unexpected()),
            None => Ok(()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Takes the next node, which must have no arguments, and returns its name.
    pub fn take_string(&mut self) -> Result<String, Error> {
        let node = self.take()?;
        if !node.args.is_empty() {
            return Err(node.unexpected());
        }
        Ok(node.name)
    }

    /// Takes the next node, and calls `f` with its name and arguments.
    pub fn take_list<T>(&mut self, f: impl FnOnce(&str, &mut Args) -> Result<T, Error>) -> Result<T, Error> {
        self.take()?.take(f)
    }

    /// Takes the next node if it's `name` without arguments, usually a `#flag`.
    pub fn take_flag(&mut self, name: &str) -> bool {
        let found = matches!(self.nodes.front(), Some(node) if node.name == name && node.args.is_empty());
        if found {
            self.nodes.pop_front();
        }
        found
    }

    /// Takes the next node if it's named `name`.
    pub fn take_optional<T>(&mut self, name: &str, f: impl FnOnce(&mut Args) -> Result<T, Error>) -> Result<Option<T>, Error> {
        if self.next_name() != Some(name) {
            return Ok(None);
        }
        self.take_list(|_, args| f(args)).map(Some)
    }

    /// Takes all consecutive nodes named `name`.
    pub fn take_many<T>(&mut self, name: &str, mut f: impl FnMut(&mut Args) -> Result<T, Error>) -> Result<Vec<T>, Error> {
        let mut values = Vec::new();
        while self.next_name() == Some(name) {
            values.push(self.take_list(|_, args| f(args))?);
        }
        Ok(values)
    }

    /// An error for a node whose name isn't accepted.
    pub fn unexpected(&self) -> Error {
        self.error(format!("unexpected {}", self.head))
    }

    pub fn error(&self, message: impl Into<String>) -> Error {
        err(self.line, message)
    }
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;

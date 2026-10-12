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

use super::lex::{Token, TokenKind, Tokens, tokenize};
use super::{Error, Pos, Result};
use std::collections::VecDeque;

struct Node {
    pos: Pos,
    name: String,
    args: Vec<Node>,
}

/// Reads the text into its top-level records.
pub fn read(text: &str) -> Result<Args<'static>> {
    let mut tokens = tokenize(text);
    let mut nodes = VecDeque::new();
    while tokens.peek()? != &TokenKind::Eof {
        nodes.push_back(record(&mut tokens)?);
    }
    Ok(Args { pos: Pos { line: 1, column: 1 }, head: "", nodes })
}

/// The second part of the returned tuple specifies whether the node had anything after the string token.
fn node_or_string(tokens: &mut Tokens) -> Result<(Node, bool)> {
    let (pos, name) = match tokens.next()? {
        Token { pos, kind: TokenKind::Str(s) } => (pos, s),
        token => return Err(token.unexpected()),
    };
    match tokens.peek()? {
        TokenKind::Colon => {
            tokens.next()?;
            let arg = node(tokens)?;
            Ok((Node { pos, name, args: vec![arg] }, true))
        },
        TokenKind::LeftParen => {
            let mut args = Vec::new();
            tokens.next()?;
            while tokens.peek()? != &TokenKind::RightParen {
                args.push(node(tokens)?);
            }
            tokens.next()?;
            Ok((Node { pos, name, args }, true))
        },
        _ => {
            Ok((Node { pos, name, args: Vec::new() }, false))
        },
    }
}

/// `node = STRING, [ ":", node | "(", { node }, ")" ]`
fn node(tokens: &mut Tokens) -> Result<Node> {
    let (node, _) = node_or_string(tokens)?;
    Ok(node)
}

/// `{ node }`
fn nodes(tokens: &mut Tokens) -> Result<Vec<Node>> {
    let mut nodes = Vec::new();
    while matches!(tokens.peek()?, TokenKind::Str(_)) {
        nodes.push(node(tokens)?);
    }
    Ok(nodes)
}

/// `short record = STRING, { node } | node`
fn short_record(tokens: &mut Tokens) -> Result<Node> {
    let (mut node, has_args) = node_or_string(tokens)?;
    if !has_args {
        node.args.extend(nodes(tokens)?);
    }
    Ok(node)
}

/// `{ segment }`, where `segment = "|", short record | RECORD_STRING`
fn segments(tokens: &mut Tokens) -> Result<Vec<Node>> {
    let mut segments = Vec::new();
    loop {
        match tokens.next_if(|kind| matches!(kind, TokenKind::Pipe | TokenKind::RecordStr(_)))? {
            Some(Token { kind: TokenKind::Pipe, .. }) => segments.push(short_record(tokens)?),
            Some(Token { pos, kind: TokenKind::RecordStr(name) }) => segments.push(Node { pos, name, args: Vec::new() }),
            _ => return Ok(segments),
        }
    }
}

fn record(tokens: &mut Tokens) -> Result<Node> {
    let (mut node, has_args) = node_or_string(tokens)?;
    if has_args {
        tokens.expect(TokenKind::Newline)?;
        return Ok(node);
    }
    node.args.extend(nodes(tokens)?);
    node.args.extend(segments(tokens)?);
    tokens.expect(TokenKind::Newline)?;
    if tokens.next_if(|kind| kind == &TokenKind::Indent)?.is_some() {
        while matches!(tokens.peek()?, TokenKind::Pipe | TokenKind::RecordStr(_)) {
            node.args.extend(segments(tokens)?);
            tokens.expect(TokenKind::Newline)?;
        }
        while matches!(tokens.peek()?, TokenKind::Str(_)) {
            node.args.push(record(tokens)?);
        }
        tokens.expect(TokenKind::Dedent)?;
    }
    Ok(node)
}

impl Node {
    /// Calls `f` with the name and arguments of this node. It's an error if
    /// `f` leaves any arguments unconsumed.
    fn take<T>(self, f: impl FnOnce(&str, &mut Args) -> Result<T>) -> Result<T> {
        let mut args = Args { pos: self.pos, head: &self.name, nodes: self.args.into() };
        let value = f(&self.name, &mut args)?;
        args.finish()?;
        Ok(value)
    }

    fn unexpected(&self) -> Error {
        if self.args.is_empty() {
            Error::new(self.pos, format!("unexpected {}", self.name))
        } else {
            Error::new(self.pos, format!("unexpected {}(…)", self.name))
        }
    }
}

/// The arguments of a node, consumed in order.
pub struct Args<'h> {
    pub pos: Pos,
    head: &'h str,
    nodes: VecDeque<Node>,
}

impl Args<'_> {
    fn take(&mut self) -> Result<Node> {
        self.nodes.pop_front().ok_or_else(|| self.error(format!("missing item in {}", self.head)))
    }

    /// The name of the next node.
    fn next_name(&self) -> Option<&str> {
        self.nodes.front().map(|node| node.name.as_str())
    }

    fn finish(&self) -> Result<()> {
        match self.nodes.front() {
            Some(node) => Err(node.unexpected()),
            None => Ok(()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Takes the next node, which must have no arguments, and returns its name.
    pub fn take_string(&mut self) -> Result<String> {
        let node = self.take()?;
        if !node.args.is_empty() {
            return Err(node.unexpected());
        }
        Ok(node.name)
    }

    /// Takes the next node, and calls `f` with its name and arguments.
    pub fn take_list<T>(&mut self, f: impl FnOnce(&str, &mut Args) -> Result<T>) -> Result<T> {
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
    pub fn take_optional<T>(&mut self, name: &str, f: impl FnOnce(&mut Args) -> Result<T>) -> Result<Option<T>> {
        if self.next_name() != Some(name) {
            return Ok(None);
        }
        self.take_list(|_, args| f(args)).map(Some)
    }

    /// Takes all consecutive nodes named `name`.
    pub fn take_many<T>(&mut self, name: &str, mut f: impl FnMut(&mut Args) -> Result<T>) -> Result<Vec<T>> {
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
        Error::new(self.pos, message)
    }
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;

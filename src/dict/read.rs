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

use super::{Error, Result};
use super::lex::{Token, TokenKind, tokenize};
use std::collections::VecDeque;
use std::iter::Peekable;

struct Node {
    line: usize,
    name: String,
    args: Vec<Node>,
}

/// Reads the text into its top-level records.
pub fn read(text: &str) -> Result<Args<'static>> {
    let mut tokens = tokenize(text)?.into_iter().peekable();
    let mut nodes = VecDeque::new();
    while tokens.peek().is_some() {
        nodes.push_back(record(&mut tokens)?);
    }
    Ok(Args { line: 0, head: "", nodes })
}

type Tokens = Peekable<std::vec::IntoIter<Token>>;

fn peek(tokens: &mut Tokens) -> Option<&TokenKind> {
    tokens.peek().map(|t| &t.kind)
}

fn unexpected(token: &Option<Token>) -> Error {
    let Some(token) = token else {
        return Error::new(0, "unexpected end of file");
    };
    let record_string;
    let name = match &token.kind {
        TokenKind::Str(s) => s.as_str(),
        TokenKind::RecordStr(s) => {
            record_string = format!("|| {s}");
            &record_string
        },
        TokenKind::LeftParen => "(",
        TokenKind::RightParen => ")",
        TokenKind::Colon => ":",
        TokenKind::Pipe => "|",
        TokenKind::Newline => "end of line",
        TokenKind::Indent => "indentation",
        TokenKind::Dedent => "unindentation",
    };
    Error::new(token.line, format!("unexpected {name}"))
}

fn expect(tokens: &mut Tokens, kind: TokenKind) -> Result<()> {
    match tokens.next() {
        Some(token) if token.kind == kind => Ok(()),
        token => Err(unexpected(&token)),
    }
}

/// The second part of the returned tuple specifies whether the node had anything after the string token.
fn node_or_string(tokens: &mut Tokens) -> Result<(Node, bool)> {
    let (line, name) = match tokens.next() {
        Some(Token { line, kind: TokenKind::Str(s) }) => (line, s),
        token => return Err(unexpected(&token)),
    };
    match peek(tokens) {
        Some(TokenKind::Colon) => {
            tokens.next();
            let arg = node(tokens)?;
            Ok((Node { line, name, args: vec![arg] }, true))
        },
        Some(TokenKind::LeftParen) => {
            let mut args = Vec::new();
            tokens.next();
            while peek(tokens) != Some(&TokenKind::RightParen) {
                args.push(node(tokens)?);
            }
            tokens.next();
            Ok((Node { line, name, args }, true))
        },
        _ => {
            Ok((Node { line, name, args: Vec::new() }, false))
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
    while matches!(peek(tokens), Some(TokenKind::Str(_))) {
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
        match tokens.next_if(|t| matches!(t.kind, TokenKind::Pipe | TokenKind::RecordStr(_))) {
            Some(Token { kind: TokenKind::Pipe, .. }) => segments.push(short_record(tokens)?),
            Some(Token { line, kind: TokenKind::RecordStr(name) }) => segments.push(Node { line, name, args: Vec::new() }),
            _ => return Ok(segments),
        }
    }
}

fn record(tokens: &mut Tokens) -> Result<Node> {
    let (mut node, has_args) = node_or_string(tokens)?;
    if has_args {
        expect(tokens, TokenKind::Newline)?;
        return Ok(node);
    }
    node.args.extend(nodes(tokens)?);
    node.args.extend(segments(tokens)?);
    expect(tokens, TokenKind::Newline)?;
    if peek(tokens) == Some(&TokenKind::Indent) {
        tokens.next();
        while matches!(peek(tokens), Some(TokenKind::Pipe | TokenKind::RecordStr(_))) {
            node.args.extend(segments(tokens)?);
            expect(tokens, TokenKind::Newline)?;
        }
        while matches!(peek(tokens), Some(TokenKind::Str(_))) {
            node.args.push(record(tokens)?);
        }
        expect(tokens, TokenKind::Dedent)?;
    }
    Ok(node)
}

impl Node {
    /// Calls `f` with the name and arguments of this node. It's an error if
    /// `f` leaves any arguments unconsumed.
    fn take<T>(self, f: impl FnOnce(&str, &mut Args) -> Result<T>) -> Result<T> {
        let mut args = Args { line: self.line, head: &self.name, nodes: self.args.into() };
        let value = f(&self.name, &mut args)?;
        args.finish()?;
        Ok(value)
    }

    fn unexpected(&self) -> Error {
        if self.args.is_empty() {
            Error::new(self.line, format!("unexpected {}", self.name))
        } else {
            Error::new(self.line, format!("unexpected {}(…)", self.name))
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
        Error::new(self.line, message)
    }
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;

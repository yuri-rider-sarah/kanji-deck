//! Reads the tokens from `lex` into a tree of nodes, which callers access
//! through `Args`.
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

use super::Pos;
use super::lex::{Token, TokenKind, Tokens, tokenize};
use crate::error::{Error, Result};
use std::collections::VecDeque;

struct Node {
    pos: Pos,
    name: String,
    args: Vec<Node>,
}

/// Parses the text into its top-level records.
pub fn parse(text: &str) -> Result<Args> {
    let mut tokens = tokenize(text);
    let mut nodes = VecDeque::new();
    while tokens.peek()? != &TokenKind::Eof {
        nodes.push_back(read_record(&mut tokens)?);
    }
    Ok(Args { pos: Pos { line: 1, column: 1 }, nodes })
}

/// An error saying a token wasn't expected.
fn unexpected_token_error(token: &Token) -> Error {
    let kind_name = match &token.kind {
        TokenKind::Str(_) => "string",
        TokenKind::RecordStr(_) => "record string",
        TokenKind::LeftParen => "(",
        TokenKind::RightParen => ")",
        TokenKind::Colon => ":",
        TokenKind::Pipe => "|",
        TokenKind::Newline => "end of line",
        TokenKind::Indent => "indentation",
        TokenKind::Dedent => "unindentation",
        TokenKind::Eof => "end of file",
    };
    Error::new(token.pos, format!("unexpected {kind_name}"))
}

/// Takes the next token, which must be of the given kind.
fn expect_token(tokens: &mut Tokens, kind: TokenKind) -> Result<()> {
    let token = tokens.next()?;
    if token.kind != kind {
        return Err(unexpected_token_error(&token));
    }
    Ok(())
}

/// Reads a node.
/// The second part of the returned tuple specifies whether the node contained
/// any tokens after the string token.
fn read_node_or_string(tokens: &mut Tokens) -> Result<(Node, bool)> {
    let mut node = match tokens.next()? {
        Token { pos, kind: TokenKind::Str(s) } => Node { pos, name: s, args: Vec::new() },
        token => return Err(unexpected_token_error(&token)),
    };
    match tokens.peek()? {
        TokenKind::Colon => {
            tokens.next()?;
            node.args.push(read_node(tokens)?);
            Ok((node, true))
        }
        TokenKind::LeftParen => {
            tokens.next()?;
            while tokens.peek()? != &TokenKind::RightParen {
                node.args.push(read_node(tokens)?);
            }
            tokens.next()?;
            Ok((node, true))
        }
        _ => Ok((node, false)),
    }
}

/// `node = STRING, [ ":", node | "(", { node }, ")" ]`
fn read_node(tokens: &mut Tokens) -> Result<Node> {
    let (node, _) = read_node_or_string(tokens)?;
    Ok(node)
}

/// `{ node }`
fn read_nodes(tokens: &mut Tokens) -> Result<Vec<Node>> {
    let mut nodes = Vec::new();
    while matches!(tokens.peek()?, TokenKind::Str(_)) {
        nodes.push(read_node(tokens)?);
    }
    Ok(nodes)
}

/// `short record = STRING, { node } | node`
fn read_short_record(tokens: &mut Tokens) -> Result<Node> {
    let (mut node, has_args) = read_node_or_string(tokens)?;
    if !has_args {
        node.args.extend(read_nodes(tokens)?);
    }
    Ok(node)
}

/// `{ segment }, NEWLINE`, where `segment = "|", short record | RECORD_STRING`
fn read_segments(tokens: &mut Tokens) -> Result<Vec<Node>> {
    let mut segments = Vec::new();
    loop {
        match tokens.next()? {
            Token { kind: TokenKind::Pipe, .. } => segments.push(read_short_record(tokens)?),
            Token { kind: TokenKind::RecordStr(name), pos } => {
                segments.push(Node { pos, name, args: Vec::new() })
            }
            Token { kind: TokenKind::Newline, .. } => return Ok(segments),
            token => return Err(unexpected_token_error(&token)),
        }
    }
}

/// Reads a multiline record.
fn read_record(tokens: &mut Tokens) -> Result<Node> {
    let (mut node, has_args) = read_node_or_string(tokens)?;
    if has_args {
        expect_token(tokens, TokenKind::Newline)?;
        return Ok(node);
    }
    node.args.extend(read_nodes(tokens)?);
    node.args.extend(read_segments(tokens)?);
    if tokens.peek()? == &TokenKind::Indent {
        tokens.next()?;
        while matches!(tokens.peek()?, TokenKind::Pipe | TokenKind::RecordStr(_)) {
            node.args.extend(read_segments(tokens)?);
        }
        while matches!(tokens.peek()?, TokenKind::Str(_)) {
            node.args.push(read_record(tokens)?);
        }
        expect_token(tokens, TokenKind::Dedent)?;
    }
    Ok(node)
}

/// The arguments of a node, consumed in order.
pub struct Args {
    pos: Pos,
    nodes: VecDeque<Node>,
}

impl Node {
    /// Calls `f` with the name and arguments of this node. It's an error if
    /// `f` leaves any arguments unconsumed.
    fn take<T>(self, f: impl FnOnce(String, &mut Args) -> Result<T>) -> Result<T> {
        let mut args = Args { pos: self.pos, nodes: self.args.into() };
        let value = f(self.name, &mut args)?;
        match args.nodes.front() {
            Some(node) => Err(Error::unexpected_node(node.pos, &node.name)),
            None => Ok(value),
        }
    }
}

impl Args {
    /// Takes the next node, which must have no arguments, and returns its name.
    pub fn take_string(&mut self) -> Result<String> {
        self.take_one(|head, _| Ok(head))
    }

    /// Takes the next node if it's `name` without arguments.
    pub fn take_flag(&mut self, name: &str) -> bool {
        if let Some(node) = self.nodes.front()
            && node.name == name
            && node.args.is_empty()
        {
            self.nodes.pop_front();
            true
        } else {
            false
        }
    }

    /// Takes the next node, and calls `f` with its name and arguments.
    pub fn take_one<T>(&mut self, f: impl FnOnce(String, &mut Args) -> Result<T>) -> Result<T> {
        let Some(node) = self.nodes.pop_front() else {
            return Err(Error::new(self.pos, "expected more arguments"));
        };
        node.take(f)
    }

    /// Takes the next node if it exists, and calls `f` with its name and arguments.
    pub fn take_optional<T>(
        &mut self,
        f: impl FnOnce(String, &mut Args) -> Result<T>,
    ) -> Result<Option<T>> {
        match self.nodes.pop_front() {
            Some(node) => Ok(Some(node.take(f)?)),
            None => Ok(None),
        }
    }

    /// Takes the next node if it's named `name`.
    pub fn take_optional_by_name<T>(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Args) -> Result<T>,
    ) -> Result<Option<T>> {
        match self.nodes.pop_front_if(|node| node.name == name) {
            Some(node) => Ok(Some(node.take(|_, args| f(args))?)),
            None => Ok(None),
        }
    }

    /// Takes all consecutive nodes named `name`.
    pub fn take_many_by_name<T>(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut Args) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut values = Vec::new();
        while let Some(node) = self.nodes.pop_front_if(|node| node.name == name) {
            values.push(node.take(|_, args| f(args))?);
        }
        Ok(values)
    }

    pub fn pos(&self) -> Pos {
        self.pos
    }
}

#[cfg(test)]
#[path = "tests/parse.rs"]
mod tests;

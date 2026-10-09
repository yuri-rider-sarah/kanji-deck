//! The first phase of parsing the dictionary: reading the text into a tree of
//! S-expressions, without interpreting what they mean.

use super::Error;
use std::collections::VecDeque;

pub struct Node {
    pub line: usize,
    pub value: Value,
}

pub enum Value {
    Atom(String),
    Str(String),
    /// A list, which always starts with an atom saying what it is.
    List { head: String, args: Vec<Node> },
}

fn err(line: usize, message: impl Into<String>) -> Error {
    Error { line, message: message.into() }
}

pub fn read(text: &str) -> Result<Vec<Node>, Error> {
    // Each open list is kept on the stack with the line it starts on.
    let mut stack: Vec<(usize, Vec<Node>)> = vec![(0, Vec::new())];
    let mut line = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '(' => stack.push((line, Vec::new())),
            ')' => {
                let (start, mut items) = stack.pop().filter(|_| !stack.is_empty()).ok_or(err(line, "unexpected )"))?;
                let head = match items.first() {
                    Some(Node { value: Value::Atom(head), .. }) => head.clone(),
                    _ => return Err(err(start, "list doesn't start with a word")),
                };
                items.remove(0);
                stack.last_mut().unwrap().1.push(Node { line: start, value: Value::List { head, args: items } });
            }
            '"' => {
                let start = line;
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some(c) => {
                            if c == '\n' {
                                line += 1;
                            }
                            s.push(c);
                        }
                        None => return Err(err(start, "unterminated string")),
                    }
                }
                stack.last_mut().unwrap().1.push(Node { line: start, value: Value::Str(s) });
            }
            c => {
                let mut s = String::from(c);
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || "()\"".contains(c) {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                stack.last_mut().unwrap().1.push(Node { line, value: Value::Atom(s) });
            }
        }
    }
    if stack.len() > 1 {
        return Err(err(stack.last().unwrap().0, "unclosed ("));
    }
    Ok(stack.pop().unwrap().1)
}

impl Node {
    /// Calls `f` with the head and arguments of this list. It's an error if
    /// this isn't a list, or if `f` leaves any arguments unconsumed.
    pub fn take_list<T>(self, f: impl FnOnce(&str, &mut Args) -> Result<T, Error>) -> Result<T, Error> {
        let Value::List { head, args } = self.value else {
            return Err(err(self.line, "expected a list"));
        };
        let mut args = Args { line: self.line, head: &head, nodes: args.into() };
        let value = f(&head, &mut args)?;
        args.finish()?;
        Ok(value)
    }

    fn unexpected(&self) -> Error {
        match &self.value {
            Value::List { head, .. } => err(self.line, format!("unexpected ({head})")),
            Value::Atom(atom) => err(self.line, format!("unexpected {atom}")),
            Value::Str(s) => err(self.line, format!("unexpected \"{s}\"")),
        }
    }
}

/// The arguments of a list, consumed in order.
pub struct Args<'h> {
    pub line: usize,
    head: &'h str,
    nodes: VecDeque<Node>,
}

impl Args<'_> {
    fn take(&mut self) -> Result<Node, Error> {
        self.nodes.pop_front().ok_or_else(|| self.error(format!("missing item in ({})", self.head)))
    }

    /// The head of the next node, if it's a list.
    fn next_head(&self) -> Option<&str> {
        match &self.nodes.front()?.value {
            Value::List { head, .. } => Some(head),
            _ => None,
        }
    }

    fn finish(&self) -> Result<(), Error> {
        match self.nodes.front() {
            Some(node) => Err(node.unexpected()),
            None => Ok(()),
        }
    }

    pub fn take_atom(&mut self) -> Result<String, Error> {
        let node = self.take()?;
        match node.value {
            Value::Atom(s) => Ok(s),
            _ => Err(err(node.line, "expected a word")),
        }
    }

    pub fn take_string(&mut self) -> Result<String, Error> {
        let node = self.take()?;
        match node.value {
            Value::Str(s) => Ok(s),
            _ => Err(err(node.line, "expected a string")),
        }
    }

    pub fn take_list<T>(&mut self, f: impl FnOnce(&str, &mut Args) -> Result<T, Error>) -> Result<T, Error> {
        self.take()?.take_list(f)
    }

    /// Takes all consecutive lists with one of these heads.
    pub fn take_many_with<T>(
        &mut self,
        heads: &[&str],
        mut f: impl FnMut(&str, &mut Args) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let mut values = Vec::new();
        while self.next_head().is_some_and(|head| heads.contains(&head)) {
            values.push(self.take_list(&mut f)?);
        }
        Ok(values)
    }

    /// Takes the next node if it's a list with this head.
    pub fn take_optional<T>(&mut self, head: &str, f: impl FnOnce(&mut Args) -> Result<T, Error>) -> Result<Option<T>, Error> {
        if self.next_head() != Some(head) {
            return Ok(None);
        }
        self.take_list(|_, args| f(args)).map(Some)
    }

    /// Takes all consecutive lists with this head.
    pub fn take_many<T>(&mut self, head: &str, mut f: impl FnMut(&mut Args) -> Result<T, Error>) -> Result<Vec<T>, Error> {
        self.take_many_with(&[head], |_, args| f(args))
    }

    /// An error for a list whose head isn't accepted.
    pub fn unexpected(&self) -> Error {
        self.error(format!("unexpected ({})", self.head))
    }

    pub fn error(&self, message: impl Into<String>) -> Error {
        err(self.line, message)
    }
}

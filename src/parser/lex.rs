//! Splits text into tokens.
//!
//! Indentation is turned into `Indent` and `Dedent` tokens, and every non-blank line
//! ends with a `Newline` token. Indentation is made of spaces or tabs.
//! Within a line, any whitespace separates tokens, but is otherwise ignored.
//! Tokens are read lazily.

use super::Pos;
use crate::error::{Error, Result};
use std::str::Chars;
use unicode_general_category::{GeneralCategory as GC, get_general_category};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A string, quoted or unquoted.
    Str(String),
    /// A string from `||` to the next `|` or the end of the line, trimmed.
    RecordStr(String),
    LeftParen,
    RightParen,
    Colon,
    Pipe,
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub pos: Pos,
    pub kind: TokenKind,
}

/// All Unicode characters serving as line terminators.
const NEWLINES: [char; 7] = ['\n', '\u{b}', '\u{c}', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// Whitespace separating tokens. Line breaks aren't included.
fn is_inline_space(c: char) -> bool {
    c.is_whitespace() && !NEWLINES.contains(&c)
}

/// Whether a character can be part of an unquoted string.
fn allowed_unquoted(c: char) -> bool {
    !matches!(c, '"' | ':' | '|')
        && !matches!(
            get_general_category(c),
            GC::Control
                | GC::SpaceSeparator
                | GC::LineSeparator
                | GC::ParagraphSeparator
                | GC::OpenPunctuation
                | GC::ClosePunctuation
                | GC::InitialPunctuation
                | GC::FinalPunctuation
        )
}

/// Whether a character can be part of a quoted or record string.
fn allowed_quoted(c: char) -> bool {
    c == '\t'
        || !matches!(
            get_general_category(c),
            GC::Control | GC::LineSeparator | GC::ParagraphSeparator
        )
}

pub fn tokenize(text: &str) -> Tokens<'_> {
    Tokens {
        chars: text.chars(),
        pos: Pos { line: 1, column: 1 },
        peeked_token: None,
        queued_dedents: 0,
        at_line_start: true,
        current_indent: 0,
        old_indents: Vec::new(),
        indent_char: None,
    }
}

/// The tokens of a text, read in order. After the last token, `Eof`, the
/// cursor stays at `Eof`.
pub struct Tokens<'a> {
    chars: Chars<'a>,
    /// The position of the next character.
    pos: Pos,
    /// The next token, if it has been peeked at.
    peeked_token: Option<Token>,
    /// Number of unindentations read, but not returned yet.
    queued_dedents: usize,
    /// Whether the next character starts a line.
    at_line_start: bool,
    /// Width of the current indentation level.
    current_indent: usize,
    /// Widths of the previous indentation levels.
    old_indents: Vec<usize>,
    /// The character used for indentation, set by the first indented line.
    indent_char: Option<char>,
}

impl Tokens<'_> {
    /// Get kind of next token without consuming it.
    pub fn peek(&mut self) -> Result<&TokenKind> {
        if self.peeked_token.is_none() {
            self.peeked_token = Some(self.read()?);
        }
        Ok(&self.peeked_token.as_ref().unwrap().kind)
    }

    /// Get next token.
    pub fn next(&mut self) -> Result<Token> {
        match self.peeked_token.take() {
            Some(token) => Ok(token),
            None => self.read(),
        }
    }

    /// Get the next character without skipping it.
    fn peek_char(&self) -> Option<char> {
        self.chars.clone().next()
    }

    /// Move to the next character.
    fn next_char(&mut self) {
        let c = self.chars.next();
        if c.is_some() {
            self.pos.column += 1;
        }
    }

    /// Expect a line break and read it.
    fn skip_line_break(&mut self) -> Result<()> {
        match self.peek_char() {
            Some('\n') => self.next_char(),
            Some('\r') => {
                self.next_char();
                match self.peek_char() {
                    Some('\n') => self.next_char(),
                    _ => return Err(Error::new(self.pos, "expected NL after CR")),
                }
            }
            _ => (),
        }
        self.pos = Pos { line: self.pos.line + 1, column: 1 };
        Ok(())
    }

    /// Read a token from the stream.
    fn read(&mut self) -> Result<Token> {
        // If at start of line, process indentations.
        if self.at_line_start {
            self.at_line_start = false;
            let width = self.read_indentation()?;
            if width > self.current_indent {
                self.old_indents.push(self.current_indent);
                self.current_indent = width;
                return Ok(Token { pos: self.pos, kind: TokenKind::Indent });
            }
            while width < self.current_indent
                && let Some(last_indent) = self.old_indents.pop()
            {
                self.current_indent = last_indent;
                self.queued_dedents += 1;
            }
            if width != self.current_indent {
                return Err(Error::new(self.pos, "indentation doesn't match any outer line"));
            }
        }
        // Emit queued unindentations.
        if self.queued_dedents > 0 {
            self.queued_dedents -= 1;
            return Ok(Token { pos: self.pos, kind: TokenKind::Dedent });
        }
        // Skip whitespace.
        while let Some(c) = self.peek_char()
            && is_inline_space(c)
        {
            self.next_char();
        }
        let pos = self.pos;
        let kind = match self.peek_char() {
            Some('|') => {
                self.next_char();
                match self.peek_char() {
                    Some('|') => {
                        self.next_char();
                        match self.peek_char() {
                            Some('|') => return Err(Error::new(pos, "ambiguous token: |||")),
                            _ => self.read_record_string(),
                        }
                    }
                    _ => TokenKind::Pipe,
                }
            }
            Some('(') => {
                self.next_char();
                TokenKind::LeftParen
            }
            Some(')') => {
                self.next_char();
                TokenKind::RightParen
            }
            Some(':') => {
                self.next_char();
                TokenKind::Colon
            }
            Some('"') => {
                self.next_char();
                TokenKind::Str(self.read_quoted_string()?)
            }
            Some(c) if allowed_unquoted(c) => {
                let mut s = String::new();
                while let Some(c) = self.peek_char()
                    && allowed_unquoted(c)
                {
                    self.next_char();
                    s.push(c);
                }
                TokenKind::Str(s)
            }
            Some('\n' | '\r') => {
                self.skip_line_break()?;
                self.at_line_start = true;
                TokenKind::Newline
            }
            Some(c) => return Err(Error::new(pos, format!("unexpected character {c:?}"))),
            None => {
                // The last line may end without a line break, so we still emit a newline token in
                // those cases.
                let kind = if self.at_line_start { TokenKind::Eof } else { TokenKind::Newline };
                self.at_line_start = true;
                kind
            }
        };
        if matches!(kind, TokenKind::Str(_))
            && self.peek_char().is_some_and(|c| c == '"' || allowed_unquoted(c))
        {
            return Err(Error::new(self.pos, "strings must be separated by whitespace"));
        }
        Ok(Token { pos, kind })
    }

    /// Read the indentation at the start of a line and return its width.
    fn read_indentation(&mut self) -> Result<usize> {
        let mut width = 0;
        loop {
            match self.peek_char() {
                Some(c @ (' ' | '\t')) => {
                    if *self.indent_char.get_or_insert(c) != c {
                        return Err(Error::new(self.pos, "indentation mixes tabs and spaces"));
                    }
                    self.next_char();
                    width += 1;
                }
                Some(c) if is_inline_space(c) => {
                    return Err(Error::new(
                        self.pos,
                        format!("unexpected character {c:?} in indentation"),
                    ));
                }
                Some('\n' | '\r') => {
                    self.skip_line_break()?;
                    width = 0;
                }
                None => {
                    // Set `at_line_start` so we know not to emit a newline token before EOF.
                    self.at_line_start = true;
                    width = 0;
                    break;
                }
                _ => break,
            }
        }
        Ok(width)
    }

    /// Read the rest of a record string after `||`.
    fn read_record_string(&mut self) -> TokenKind {
        while let Some(c) = self.peek_char()
            && is_inline_space(c)
        {
            self.next_char();
        }
        let mut s = String::new();
        while let Some(c) = self.peek_char()
            && allowed_quoted(c)
            && c != '|'
        {
            self.next_char();
            s.push(c);
        }
        s.truncate(s.trim_end().len());
        TokenKind::RecordStr(s)
    }

    /// Read the rest of a quoted string after `"`.
    fn read_quoted_string(&mut self) -> Result<String> {
        let mut s = String::new();
        while let Some(c) = self.peek_char()
            && allowed_quoted(c)
            && c != '"'
        {
            self.next_char();
            s.push(c);
        }
        match self.peek_char() {
            Some('"') => self.next_char(),
            Some('\n' | '\r') | None => return Err(Error::new(self.pos, "unterminated string")),
            Some(c) => {
                return Err(Error::new(self.pos, format!("unexpected character {c:?} in string")));
            }
        }
        Ok(s)
    }
}

#[cfg(test)]
#[path = "tests/lex.rs"]
mod tests;

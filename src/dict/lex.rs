//! Splits the dictionary text into tokens.
//!
//! Indentation is turned into `Indent` and `Dedent` tokens, and every
//! non-blank line ends with a `Newline` token. Indentation is made of spaces
//! or tabs. Within a line, any whitespace separates tokens, but is otherwise
//! ignored. Lines end with LF or CRLF, and other line breaks aren't allowed
//! anywhere. The last token is `Eof`.
//!
//! Tokens are read lazily, so errors in the text are reported when the
//! reader gets to them.

use super::{Error, Pos, Result};
use std::str::Chars;
use unicode_general_category::{GeneralCategory as G, get_general_category};

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

impl Token {
    /// An error saying this token wasn't expected here.
    pub fn unexpected(&self) -> Error {
        let record_string;
        let name = match &self.kind {
            TokenKind::Str(s) => s.as_str(),
            TokenKind::RecordStr(s) => {
                record_string = format!("|| {s}");
                &record_string
            }
            TokenKind::LeftParen => "(",
            TokenKind::RightParen => ")",
            TokenKind::Colon => ":",
            TokenKind::Pipe => "|",
            TokenKind::Newline => "end of line",
            TokenKind::Indent => "indentation",
            TokenKind::Dedent => "unindentation",
            TokenKind::Eof => "end of file",
        };
        Error::new(self.pos, format!("unexpected {name}"))
    }
}

// All Unicode characters serving as line terminators.
const NEWLINES: [char; 7] = ['\n', '\u{b}', '\u{c}', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// Whitespace separating tokens. Line breaks aren't included.
fn is_space(c: char) -> bool {
    c.is_whitespace() && !NEWLINES.contains(&c)
}

/// Whether a character can be part of an unquoted string.
fn is_unquoted(c: char) -> bool {
    !"\":|".contains(c)
        && !matches!(
            get_general_category(c),
            G::Control
                | G::OpenPunctuation
                | G::ClosePunctuation
                | G::InitialPunctuation
                | G::FinalPunctuation
                | G::SpaceSeparator
                | G::LineSeparator
                | G::ParagraphSeparator
        )
}

/// Whether a character can be part of a quoted or record string, apart from
/// the character ending it.
fn is_string_char(c: char) -> bool {
    c == '\t' || !matches!(get_general_category(c), G::Control | G::LineSeparator | G::ParagraphSeparator)
}

/// Whether a character can be part of a quoted string.
fn is_quotable(c: char) -> bool {
    c != '"' && is_string_char(c)
}

/// Whether a character can be part of a record string.
fn is_record_char(c: char) -> bool {
    c != '|' && is_string_char(c)
}

pub fn tokenize(text: &str) -> Tokens<'_> {
    Tokens {
        chars: text.chars(),
        pos: Pos { line: 1, column: 1 },
        peeked: None,
        queued: Vec::new(),
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
    peeked: Option<Token>,
    /// `Indent` or `Dedent` tokens read from the indentation of a line, but not returned yet.
    queued: Vec<Token>,
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
    pub fn peek(&mut self) -> Result<&TokenKind> {
        if self.peeked.is_none() {
            self.peeked = Some(self.read()?);
        }
        Ok(&self.peeked.as_ref().unwrap().kind)
    }

    pub fn next(&mut self) -> Result<Token> {
        match self.peeked.take() {
            Some(token) => Ok(token),
            None => self.read(),
        }
    }

    /// Takes the next token if `f` returns true for its kind.
    pub fn next_if(&mut self, f: impl FnOnce(&TokenKind) -> bool) -> Result<Option<Token>> {
        if f(self.peek()?) { self.next().map(Some) } else { Ok(None) }
    }

    /// Takes the next token, which must be of the given kind.
    pub fn expect(&mut self, kind: TokenKind) -> Result<()> {
        let token = self.next()?;
        if token.kind != kind {
            return Err(token.unexpected());
        }
        Ok(())
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.clone().next()
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.pos = Pos { line: self.pos.line + 1, column: 1 };
        } else {
            self.pos.column += 1;
        }
        Some(c)
    }

    /// Takes the next character if `f` returns true for it.
    fn next_char_if(&mut self, f: impl FnOnce(char) -> bool) -> Option<char> {
        if self.peek_char().is_some_and(f) { self.next_char() } else { None }
    }

    fn skip_spaces(&mut self) {
        while self.next_char_if(is_space).is_some() {}
    }

    /// Skips a line break (LF or CRLF), returning whether there was one.
    fn skip_line_break(&mut self) -> bool {
        let mut chars = self.chars.clone();
        let length = match (chars.next(), chars.next()) {
            (Some('\n'), _) => 1,
            (Some('\r'), Some('\n')) => 2,
            _ => return false,
        };
        for _ in 0..length {
            self.next_char();
        }
        true
    }

    fn read(&mut self) -> Result<Token> {
        if self.at_line_start {
            self.at_line_start = false;
            self.indentation()?;
        }
        if let Some(token) = self.queued.pop() {
            return Ok(token);
        }
        self.skip_spaces();
        let pos = self.pos;
        let Some(c) = self.peek_char() else {
            // The last line may end without a line break.
            let kind = if self.at_line_start { TokenKind::Eof } else { TokenKind::Newline };
            self.at_line_start = true;
            return Ok(Token { pos, kind });
        };
        if self.skip_line_break() {
            self.at_line_start = true;
            return Ok(Token { pos, kind: TokenKind::Newline });
        }
        let kind = match c {
            '|' => {
                self.next_char();
                if self.next_char_if(|c| c == '|').is_none() {
                    TokenKind::Pipe
                } else if self.peek_char() == Some('|') {
                    return Err(Error::new(pos, "unexpected |||"));
                } else {
                    self.record_string()
                }
            }
            '(' | ')' | ':' => {
                self.next_char();
                match c {
                    '(' => TokenKind::LeftParen,
                    ')' => TokenKind::RightParen,
                    _ => TokenKind::Colon,
                }
            }
            '"' => self.quoted_string(pos)?,
            c if is_unquoted(c) => {
                let mut s = String::new();
                while let Some(c) = self.next_char_if(is_unquoted) {
                    s.push(c);
                }
                TokenKind::Str(s)
            }
            c => return Err(Error::new(pos, format!("unexpected character {c:?}"))),
        };
        if matches!(kind, TokenKind::Str(_)) && self.peek_char().is_some_and(|c| c == '"' || is_unquoted(c)) {
            return Err(Error::new(self.pos, "strings must be separated by whitespace"));
        }
        Ok(Token { pos, kind })
    }

    /// Reads the indentation of the next line and queues the `Indent` or
    /// `Dedent` tokens it starts with. Lines with only indentation are
    /// skipped. At the end of the file, queues a `Dedent` for each open
    /// indentation level.
    fn indentation(&mut self) -> Result<()> {
        let width = loop {
            let mut width = 0;
            while let Some(c) = self.peek_char().filter(|&c| c == ' ' || c == '\t') {
                if *self.indent_char.get_or_insert(c) != c {
                    return Err(Error::new(self.pos, "indentation mixes tabs and spaces"));
                }
                self.next_char();
                width += 1;
            }
            if !self.skip_line_break() {
                break width;
            }
        };
        if self.peek_char().is_none() {
            for _ in self.old_indents.drain(..) {
                self.queued.push(Token { pos: self.pos, kind: TokenKind::Dedent });
            }
            self.at_line_start = true;
            return Ok(());
        }
        // Other whitespace right after the indentation would look like part of it.
        if let Some(c) = self.peek_char().filter(|&c| is_space(c)) {
            return Err(Error::new(self.pos, format!("unexpected character {c:?} in indentation")));
        }
        if width > self.current_indent {
            self.old_indents.push(self.current_indent);
            self.current_indent = width;
            self.queued.push(Token { pos: self.pos, kind: TokenKind::Indent });
        } else {
            while width < self.current_indent && let Some(last_indent) = self.old_indents.pop() {
                self.current_indent = last_indent;
                self.queued.push(Token { pos: self.pos, kind: TokenKind::Dedent });
            }
            if width != self.current_indent {
                return Err(Error::new(self.pos, "indentation doesn't match any outer line"));
            }
        }
        Ok(())
    }

    /// The rest of a record string after `||`.
    fn record_string(&mut self) -> TokenKind {
        self.skip_spaces();
        let mut s = String::new();
        while let Some(c) = self.next_char_if(is_record_char) {
            s.push(c);
        }
        s.truncate(s.trim_end().len());
        TokenKind::RecordStr(s)
    }

    /// A quoted string starting at `pos`.
    fn quoted_string(&mut self, pos: Pos) -> Result<TokenKind> {
        self.next_char();
        let mut s = String::new();
        while let Some(c) = self.next_char_if(is_quotable) {
            s.push(c);
        }
        match self.peek_char() {
            Some('"') => {
                self.next_char();
                Ok(TokenKind::Str(s))
            }
            None | Some('\n' | '\r') => Err(Error::new(pos, "unterminated string")),
            Some(c) => Err(Error::new(self.pos, format!("unexpected character {c:?} in string"))),
        }
    }
}

#[cfg(test)]
#[path = "tests/lex.rs"]
mod tests;

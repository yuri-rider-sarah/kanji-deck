//! Splits the dictionary text into tokens.
//!
//! Indentation is turned into `Indent` and `Dedent` tokens, and every
//! non-blank line ends with a `Newline` token. Indentation is made of spaces
//! or tabs. Within a line, any whitespace separates tokens, but is otherwise
//! ignored. Lines end with LF or CRLF, and other line breaks aren't allowed
//! anywhere.

use super::{Error, Result};
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub line: usize,
    pub kind: TokenKind,
}

// All Unicode characters serving as line terminators.
const NEWLINES: [char; 7] = ['\n', '\u{b}', '\u{c}', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

pub fn tokenize(text: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    // Width of the current indentation level.
    let mut current_indent = 0;
    // Widths of the previous indentation levels.
    let mut old_indents = Vec::new();
    // The character used for indentation, set by the first indented line.
    let mut indent_char = None;
    for (i, text) in text.lines().enumerate() {
        let line = i + 1;
        if let Some(c) = text.chars().find(|c| NEWLINES.contains(c)) {
            return Err(Error::new(line, format!("unexpected line break {c:?}")));
        }
        let content = text.trim_start_matches([' ', '\t']);
        if content.trim_start().is_empty() {
            continue;
        }
        // Other whitespace right after the indentation would look like part of it.
        if let Some(c) = content.chars().next().filter(|c| c.is_whitespace()) {
            return Err(Error::new(line, format!("unexpected character {c:?} in indentation")));
        }
        let indent = &text[..text.len() - content.len()];
        if let Some(first) = indent.chars().next() {
            let expected = *indent_char.get_or_insert(first);
            if indent.chars().any(|c| c != expected) {
                return Err(Error::new(line, "indentation mixes tabs and spaces"));
            }
        }
        let width = indent.len();
        if width > current_indent {
            old_indents.push(current_indent);
            current_indent = width;
            tokens.push(Token { line, kind: TokenKind::Indent });
        } else {
            while width < current_indent && let Some(last_indent) = old_indents.pop() {
                current_indent = last_indent;
                tokens.push(Token { line, kind: TokenKind::Dedent });
            }
            if width != current_indent {
                return Err(Error::new(line, "indentation doesn't match any outer line"));
            }
        }
        line_tokens(content, line, &mut tokens)?;
        tokens.push(Token { line, kind: TokenKind::Newline });
    }
    let line = text.lines().count();
    for _ in 0..old_indents.len() {
        tokens.push(Token { line, kind: TokenKind::Dedent });
    }
    Ok(tokens)
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

/// Whether a character can be part of a quoted string.
fn is_quotable(c: char) -> bool {
    c != '"' && !matches!(get_general_category(c), G::Control | G::LineSeparator | G::ParagraphSeparator)
}

fn line_tokens(text: &str, line: usize, tokens: &mut Vec<Token>) -> Result<()> {
    let mut push = |kind| tokens.push(Token { line, kind });
    let mut rest = text;
    // Whether the previous token is a string that ends right where `rest` starts.
    let mut after_string = false;
    loop {
        let trimmed = rest.trim_start();
        if trimmed.len() != rest.len() {
            after_string = false;
        }
        rest = trimmed;
        let Some(c) = rest.chars().next() else {
            return Ok(());
        };
        if (c == '"' || is_unquoted(c)) && after_string {
            return Err(Error::new(line, "strings must be separated by whitespace"));
        }
        after_string = false;
        match c {
            '|' => {
                if let Some(after) = rest.strip_prefix("||") {
                    if after.starts_with('|') {
                        return Err(Error::new(line, "unexpected |||"));
                    }
                    let end = after.find('|').unwrap_or(after.len());
                    push(TokenKind::RecordStr(after[..end].trim().into()));
                    rest = &after[end..];
                } else {
                    push(TokenKind::Pipe);
                    rest = &rest[1..];
                }
            }
            '(' | ')' | ':' => {
                push(match c {
                    '(' => TokenKind::LeftParen,
                    ')' => TokenKind::RightParen,
                    _ => TokenKind::Colon,
                });
                rest = &rest[1..];
            }
            '"' => {
                let after = &rest[1..];
                let end = after.find(|c| !is_quotable(c)).unwrap_or(after.len());
                match after[end..].chars().next() {
                    Some('"') => {}
                    Some(c) => return Err(Error::new(line, format!("unexpected character {c:?} in string"))),
                    None => return Err(Error::new(line, "unterminated string")),
                }
                push(TokenKind::Str(after[..end].into()));
                rest = &after[end + 1..];
                after_string = true;
            }
            c if is_unquoted(c) => {
                let end = rest.find(|c| !is_unquoted(c)).unwrap_or(rest.len());
                push(TokenKind::Str(rest[..end].into()));
                rest = &rest[end..];
                after_string = true;
            }
            c => return Err(Error::new(line, format!("unexpected character {c:?}"))),
        }
    }
}

#[cfg(test)]
#[path = "tests/lex.rs"]
mod tests;

//! Splits the dictionary text into tokens.
//!
//! Indentation is turned into `Indent` and `Dedent` tokens, and every
//! non-blank line ends with a `Newline` token. Indentation is made of spaces
//! or tabs. Within a line, any whitespace separates tokens, but is otherwise
//! ignored. Lines end with LF or CRLF, and other line breaks aren't allowed
//! anywhere.

use super::Error;
use unicode_general_category::{GeneralCategory as G, get_general_category};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A string, quoted or unquoted.
    Str(String),
    /// A string from `||` to the next `|` or the end of the line, trimmed.
    RecordStr(String),
    Open,
    Close,
    Colon,
    Pipe,
    Newline,
    Indent,
    Dedent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub line: usize,
    pub kind: Kind,
}

fn err(line: usize, message: impl Into<String>) -> Error {
    Error { line, message: message.into() }
}

pub fn tokenize(text: &str) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    // Widths of the open indentation levels.
    let mut levels = vec![0];
    // The character used for indentation, set by the first indented line.
    let mut indent_char = None;
    for (i, text) in text.lines().enumerate() {
        let line = i + 1;
        if let Some(c) = text.chars().find(|c| ['\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}'].contains(c)) {
            return Err(err(line, format!("unexpected line break {c:?}")));
        }
        let content = text.trim_start_matches([' ', '\t']);
        if content.trim_start().is_empty() {
            continue;
        }
        // Other whitespace right after the indentation would look like part of it.
        if let Some(c) = content.chars().next().filter(|c| c.is_whitespace()) {
            return Err(err(line, format!("unexpected character {c:?} in indentation")));
        }
        let indent = &text[..text.len() - content.len()];
        if let Some(first) = indent.chars().next() {
            let expected = *indent_char.get_or_insert(first);
            if indent.chars().any(|c| c != expected) {
                return Err(err(line, "indentation mixes tabs and spaces"));
            }
        }
        let width = indent.len();
        if width > *levels.last().unwrap() {
            levels.push(width);
            tokens.push(Token { line, kind: Kind::Indent });
        } else {
            while width < *levels.last().unwrap() {
                levels.pop();
                tokens.push(Token { line, kind: Kind::Dedent });
            }
            if width != *levels.last().unwrap() {
                return Err(err(line, "indentation doesn't match any outer line"));
            }
        }
        line_tokens(content, line, &mut tokens)?;
        tokens.push(Token { line, kind: Kind::Newline });
    }
    let line = text.lines().count();
    for _ in 1..levels.len() {
        tokens.push(Token { line, kind: Kind::Dedent });
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
    c != '"' && get_general_category(c) != G::Control
}

fn line_tokens(text: &str, line: usize, tokens: &mut Vec<Token>) -> Result<(), Error> {
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
            return Err(err(line, "strings must be separated by whitespace"));
        }
        after_string = false;
        match c {
            '|' => {
                if let Some(after) = rest.strip_prefix("||") {
                    if after.starts_with('|') {
                        return Err(err(line, "unexpected |||"));
                    }
                    let end = after.find('|').unwrap_or(after.len());
                    push(Kind::RecordStr(after[..end].trim().into()));
                    rest = &after[end..];
                } else {
                    push(Kind::Pipe);
                    rest = &rest[1..];
                }
            }
            '(' | ')' | ':' => {
                push(match c {
                    '(' => Kind::Open,
                    ')' => Kind::Close,
                    _ => Kind::Colon,
                });
                rest = &rest[1..];
            }
            '"' => {
                let after = &rest[1..];
                let end = after.find(|c| !is_quotable(c)).unwrap_or(after.len());
                match after[end..].chars().next() {
                    Some('"') => {}
                    Some(c) => return Err(err(line, format!("unexpected character {c:?} in string"))),
                    None => return Err(err(line, "unterminated string")),
                }
                push(Kind::Str(after[..end].into()));
                rest = &after[end + 1..];
                after_string = true;
            }
            c if is_unquoted(c) => {
                let end = rest.find(|c| !is_unquoted(c)).unwrap_or(rest.len());
                push(Kind::Str(rest[..end].into()));
                rest = &rest[end..];
                after_string = true;
            }
            c => return Err(err(line, format!("unexpected character {c:?}"))),
        }
    }
}

#[cfg(test)]
#[path = "tests/lex.rs"]
mod tests;

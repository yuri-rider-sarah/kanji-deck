use super::TokenKind::*;
use super::*;

/// All tokens up to and including `Eof`.
fn all_tokens(text: &str) -> Result<Vec<Token>> {
    let mut tokens = tokenize(text);
    let mut all = Vec::new();
    loop {
        let token = tokens.next()?;
        let eof = token.kind == Eof;
        all.push(token);
        if eof {
            return Ok(all);
        }
    }
}

fn kinds(text: &str) -> Vec<TokenKind> {
    let tokens = all_tokens(text).unwrap_or_else(|e| panic!("{}: {}", e.pos, e.message));
    tokens.into_iter().map(|t| t.kind).collect()
}

fn error(text: &str) -> String {
    match all_tokens(text) {
        Ok(_) => panic!("no error"),
        Err(e) => format!("{}: {}", e.pos, e.message),
    }
}

fn s(text: &str) -> TokenKind {
    Str(text.to_string())
}

fn r(text: &str) -> TokenKind {
    RecordStr(text.to_string())
}

#[test]
fn simple_line() {
    assert_eq!(kinds("a b:c(d)"), [s("a"), s("b"), Colon, s("c"), LeftParen, s("d"), RightParen, Newline, Eof]);
}

#[test]
fn japanese_and_symbols() {
    assert_eq!(kinds("reading #core *あ*がる 上がる"), [s("reading"), s("#core"), s("*あ*がる"), s("上がる"), Newline, Eof]);
}

#[test]
fn indentation() {
    assert_eq!(
        kinds("a\n    b\n        c\nd\n"),
        [s("a"), Newline, Indent, s("b"), Newline, Indent, s("c"), Newline, Dedent, Dedent, s("d"), Newline, Eof]
    );
}

#[test]
fn dedent_at_end_of_file() {
    assert_eq!(kinds("a\n  b"), [s("a"), Newline, Indent, s("b"), Newline, Dedent, Eof]);
}

#[test]
fn any_indentation_width() {
    assert_eq!(kinds("a\n b\n   c\n b"), [s("a"), Newline, Indent, s("b"), Newline, Indent, s("c"), Newline, Dedent, s("b"), Newline, Dedent, Eof]);
}

#[test]
fn tabs() {
    assert_eq!(kinds("a\n\tb\n\t\tc"), [s("a"), Newline, Indent, s("b"), Newline, Indent, s("c"), Newline, Dedent, Dedent, Eof]);
}

#[test]
fn blank_lines_ignored() {
    assert_eq!(kinds("a\n\n   \n  \nb\n "), [s("a"), Newline, s("b"), Newline, Eof]);
    assert_eq!(kinds("a\n\t\n\t\tb"), [s("a"), Newline, Indent, s("b"), Newline, Dedent, Eof]);
    assert_eq!(kinds(""), [Eof]);
}

#[test]
fn blank_lines_use_indentation_character() {
    assert_eq!(error("a\n  \n\t\nb"), "3:1: indentation mixes tabs and spaces");
    assert_eq!(error("a\n\u{3000}\nb"), "2:1: unexpected character '\\u{3000}' in indentation");
}

#[test]
fn mixed_tabs_and_spaces() {
    assert_eq!(error("a\n    b\n\tc"), "3:1: indentation mixes tabs and spaces");
    assert_eq!(error("a\n \tb"), "2:2: indentation mixes tabs and spaces");
}

#[test]
fn dedent_to_unknown_level() {
    assert_eq!(error("a\n    b\n  c"), "3:3: indentation doesn't match any outer line");
}

#[test]
fn pipes_and_record_strings() {
    assert_eq!(kinds("a | b || c d "), [s("a"), Pipe, s("b"), r("c d"), Newline, Eof]);
    assert_eq!(kinds("|| a || b"), [r("a"), r("b"), Newline, Eof]);
    assert_eq!(kinds("|| a | b c"), [r("a"), Pipe, s("b"), s("c"), Newline, Eof]);
    assert_eq!(kinds("||"), [r(""), Newline, Eof]);
}

#[test]
fn record_string_keeps_special_characters() {
    assert_eq!(kinds("|| to dry (tr.): \"x\" 「y」"), [r("to dry (tr.): \"x\" 「y」"), Newline, Eof]);
}

#[test]
fn record_string_characters() {
    assert_eq!(kinds("|| a \"b\" c"), [r("a \"b\" c"), Newline, Eof]);
    assert_eq!(error("|| a\u{7}b"), "1:5: unexpected character '\\u{7}'");
}

#[test]
fn triple_pipe() {
    assert_eq!(error("a ||| b"), "1:3: ambiguous token: |||");
}

#[test]
fn quoted_strings() {
    assert_eq!(kinds("\"a b\" \"\""), [s("a b"), s(""), Newline, Eof]);
    assert_eq!(kinds("(\"a\") x:\"b\" \"c\"|"), [LeftParen, s("a"), RightParen, s("x"), Colon, s("b"), s("c"), Pipe, Newline, Eof]);
    assert_eq!(kinds("\"a(b):|c\""), [s("a(b):|c"), Newline, Eof]);
}

#[test]
fn touching_strings() {
    assert_eq!(error("a\"b\""), "1:2: strings must be separated by whitespace");
    assert_eq!(error("\"a\"b"), "1:4: strings must be separated by whitespace");
    assert_eq!(error("\"a\"\"b\""), "1:4: strings must be separated by whitespace");
}

#[test]
fn unterminated_string() {
    assert_eq!(error("a\n\"b"), "2:3: unterminated string");
    assert_eq!(error("\"a\u{7}\""), "1:3: unexpected character '\\u{7}' in string");
}

#[test]
fn tabs_in_strings() {
    assert_eq!(kinds("\"a\tb\" || c\td\t"), [s("a\tb"), r("c\td"), Newline, Eof]);
}

#[test]
fn disallowed_characters() {
    assert_eq!(error("a 「b」"), "1:3: unexpected character '「'");
}

#[test]
fn any_whitespace_between_tokens() {
    assert_eq!(kinds("a\u{3000}b\u{a0}(c)\u{2003}"), [s("a"), s("b"), LeftParen, s("c"), RightParen, Newline, Eof]);
    assert_eq!(kinds("||\u{3000}a\u{3000}"), [r("a"), Newline, Eof]);
}

#[test]
fn other_whitespace_after_indentation() {
    assert_eq!(error("\u{3000}a"), "1:1: unexpected character '\\u{3000}' in indentation");
    assert_eq!(error("a\n    \u{a0}b"), "2:5: unexpected character '\\u{a0}' in indentation");
}

#[test]
fn other_line_breaks() {
    assert_eq!(kinds("a\r\nb\r\n"), [s("a"), Newline, s("b"), Newline, Eof]);
    assert_eq!(error("a\nb\rc"), "2:3: expected NL after CR");
    assert_eq!(error("a\n\u{b}"), "2:1: unexpected character '\\u{b}'");
    assert_eq!(error("a \u{c}"), "1:3: unexpected character '\\u{c}'");
    assert_eq!(error("|| a\u{85}"), "1:5: unexpected character '\\u{85}'");
    assert_eq!(error("\"a\u{2028}\""), "1:3: unexpected character '\\u{2028}' in string");
    assert_eq!(error("a\u{2029}"), "1:2: unexpected character '\\u{2029}'");
}

#[test]
fn positions() {
    let tokens = all_tokens("a\n\n  bc (\"d\" ||e\nf").unwrap();
    assert_eq!(
        tokens.iter().map(|t| t.pos.to_string()).collect::<Vec<_>>(),
        ["1:1", "1:2", "3:3", "3:3", "3:6", "3:7", "3:11", "3:14", "4:1", "4:1", "4:2", "4:2"]
    );
}

#[test]
fn cursor_stays_at_end_of_file() {
    let mut tokens = tokenize("a");
    assert_eq!(tokens.next().unwrap().kind, s("a"));
    assert_eq!(tokens.next().unwrap().kind, Newline);
    assert_eq!(tokens.next().unwrap().kind, Eof);
    assert_eq!(tokens.peek().unwrap(), &Eof);
    assert_eq!(tokens.next().unwrap().kind, Eof);
}

#[test]
fn errors_are_lazy() {
    let mut tokens = tokenize("a 「");
    assert_eq!(tokens.next().unwrap().kind, s("a"));
    let e = tokens.next().unwrap_err();
    assert_eq!(format!("{}: {}", e.pos, e.message), "1:3: unexpected character '「'");
}

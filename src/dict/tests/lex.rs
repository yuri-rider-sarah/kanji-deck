use super::Kind::*;
use super::*;

fn kinds(text: &str) -> Vec<Kind> {
    tokenize(text).unwrap().into_iter().map(|t| t.kind).collect()
}

fn error(text: &str) -> String {
    let e = tokenize(text).unwrap_err();
    format!("{}: {}", e.line, e.message)
}

fn s(text: &str) -> Kind {
    Str(text.into())
}

fn r(text: &str) -> Kind {
    RecordStr(text.into())
}

#[test]
fn simple_line() {
    assert_eq!(kinds("a b:c(d)"), [s("a"), s("b"), Colon, s("c"), Open, s("d"), Close, Newline]);
}

#[test]
fn japanese_and_symbols() {
    assert_eq!(kinds("reading #core *あ*がる 上がる"), [s("reading"), s("#core"), s("*あ*がる"), s("上がる"), Newline]);
}

#[test]
fn indentation() {
    assert_eq!(
        kinds("a\n    b\n        c\nd\n"),
        [s("a"), Newline, Indent, s("b"), Newline, Indent, s("c"), Newline, Dedent, Dedent, s("d"), Newline]
    );
}

#[test]
fn dedent_at_end_of_file() {
    assert_eq!(kinds("a\n  b"), [s("a"), Newline, Indent, s("b"), Newline, Dedent]);
}

#[test]
fn any_indentation_width() {
    assert_eq!(kinds("a\n b\n   c\n b"), [s("a"), Newline, Indent, s("b"), Newline, Indent, s("c"), Newline, Dedent, s("b"), Newline, Dedent]);
}

#[test]
fn tabs() {
    assert_eq!(kinds("a\n\tb\n\t\tc"), [s("a"), Newline, Indent, s("b"), Newline, Indent, s("c"), Newline, Dedent, Dedent]);
}

#[test]
fn blank_lines_ignored() {
    assert_eq!(kinds("a\n\n   \n\t\nb"), [s("a"), Newline, s("b"), Newline]);
}

#[test]
fn mixed_tabs_and_spaces() {
    assert_eq!(error("a\n    b\n\tc"), "3: indentation mixes tabs and spaces");
    assert_eq!(error("a\n \tb"), "2: indentation mixes tabs and spaces");
}

#[test]
fn dedent_to_unknown_level() {
    assert_eq!(error("a\n    b\n  c"), "3: indentation doesn't match any outer line");
}

#[test]
fn pipes_and_record_strings() {
    assert_eq!(kinds("a | b || c d "), [s("a"), Pipe, s("b"), r("c d"), Newline]);
    assert_eq!(kinds("|| a || b"), [r("a"), r("b"), Newline]);
    assert_eq!(kinds("|| a | b c"), [r("a"), Pipe, s("b"), s("c"), Newline]);
    assert_eq!(kinds("||"), [r(""), Newline]);
}

#[test]
fn record_string_keeps_special_characters() {
    assert_eq!(kinds("|| to dry (tr.): \"x\" 「y」"), [r("to dry (tr.): \"x\" 「y」"), Newline]);
}

#[test]
fn triple_pipe() {
    assert_eq!(error("a ||| b"), "1: unexpected |||");
}

#[test]
fn quoted_strings() {
    assert_eq!(kinds("\"a b\" \"\""), [s("a b"), s(""), Newline]);
    assert_eq!(kinds("(\"a\") x:\"b\" \"c\"|"), [Open, s("a"), Close, s("x"), Colon, s("b"), s("c"), Pipe, Newline]);
    assert_eq!(kinds("\"a(b):|c\""), [s("a(b):|c"), Newline]);
}

#[test]
fn touching_strings() {
    assert_eq!(error("a\"b\""), "1: strings must be separated by whitespace");
    assert_eq!(error("\"a\"b"), "1: strings must be separated by whitespace");
    assert_eq!(error("\"a\"\"b\""), "1: strings must be separated by whitespace");
}

#[test]
fn unterminated_string() {
    assert_eq!(error("a\n\"b"), "2: unterminated string");
    assert_eq!(error("\"a\u{7}\""), "1: unexpected character '\\u{7}' in string");
}

#[test]
fn disallowed_characters() {
    assert_eq!(error("a 「b」"), "1: unexpected character '「'");
}

#[test]
fn any_whitespace_between_tokens() {
    assert_eq!(kinds("a\u{3000}b\u{a0}(c)\u{2003}"), [s("a"), s("b"), Open, s("c"), Close, Newline]);
    assert_eq!(kinds("||\u{3000}a\u{3000}"), [r("a"), Newline]);
    assert_eq!(kinds("a\n\u{3000}\nb"), [s("a"), Newline, s("b"), Newline]);
}

#[test]
fn other_whitespace_after_indentation() {
    assert_eq!(error("\u{3000}a"), "1: unexpected character '\\u{3000}' in indentation");
    assert_eq!(error("a\n    \u{a0}b"), "2: unexpected character '\\u{a0}' in indentation");
}

#[test]
fn other_line_breaks() {
    assert_eq!(kinds("a\r\nb\r\n"), [s("a"), Newline, s("b"), Newline]);
    assert_eq!(error("a\nb\rc"), "2: unexpected line break '\\r'");
    assert_eq!(error("a\n\u{b}"), "2: unexpected line break '\\u{b}'");
    assert_eq!(error("a \u{c}"), "1: unexpected line break '\\u{c}'");
    assert_eq!(error("|| a\u{85}"), "1: unexpected line break '\\u{85}'");
    assert_eq!(error("\"a\u{2028}\""), "1: unexpected line break '\\u{2028}'");
    assert_eq!(error("a\u{2029}"), "1: unexpected line break '\\u{2029}'");
}

#[test]
fn line_numbers() {
    let tokens = tokenize("a\n\nb").unwrap();
    assert_eq!(tokens.iter().map(|t| t.line).collect::<Vec<_>>(), [1, 1, 3, 3]);
}

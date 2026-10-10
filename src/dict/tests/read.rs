use super::*;

fn render(node: &Node) -> String {
    let name = if node.name.is_empty() || node.name.contains([' ', '(', ')', ':', '|']) {
        format!("{:?}", node.name)
    } else {
        node.name.clone()
    };
    if node.args.is_empty() {
        name
    } else {
        format!("{name}({})", node.args.iter().map(render).collect::<Vec<_>>().join(" "))
    }
}

/// The top-level records, rendered as `a(b c)` and separated by `; `.
fn read_str(text: &str) -> String {
    let args = read(text).unwrap_or_else(|e| panic!("{}: {}", e.line, e.message));
    args.nodes.iter().map(render).collect::<Vec<_>>().join("; ")
}

fn error(text: &str) -> String {
    match read(text) {
        Ok(args) => panic!("no error, got {}", args.nodes.iter().map(render).collect::<Vec<_>>().join("; ")),
        Err(e) => format!("{}: {}", e.line, e.message),
    }
}

#[test]
fn plain_records() {
    assert_eq!(read_str("a\nb c d"), "a; b(c d)");
}

#[test]
fn nodes() {
    assert_eq!(read_str("a b:c:d e(f g(h)) i()"), "a(b(c(d)) e(f g(h)) i)");
    assert_eq!(read_str("a (b)\na : b"), "a(b); a(b)");
    assert_eq!(error("a(b):c"), "1: unexpected :");
    assert_eq!(read_str("a b(c:d \"e f\")"), "a(b(c(d) \"e f\"))");
}

#[test]
fn record_as_single_node() {
    assert_eq!(read_str("a(b c)\na:b"), "a(b c); a(b)");
}

#[test]
fn single_node_record_takes_nothing_more() {
    assert_eq!(error("a(b) c"), "1: unexpected c");
    assert_eq!(error("a:b c"), "1: unexpected c");
    assert_eq!(error("a:b | c"), "1: unexpected |");
    assert_eq!(error("a:b\n    c"), "1: a record written as a single node can't have indented lines");
}

#[test]
fn segments() {
    assert_eq!(read_str("a b | c d | e || f g"), "a(b c(d) e \"f g\")");
    assert_eq!(read_str("a || b || c | d"), "a(b c d)");
    assert_eq!(read_str("a | b:c | d(e) | f"), "a(b(c) d(e) f)");
}

#[test]
fn segment_single_node_takes_nothing_more() {
    assert_eq!(error("a | b(c) d"), "1: unexpected d");
}

#[test]
fn empty_segment() {
    assert_eq!(error("a | | b"), "1: unexpected |");
    assert_eq!(error("a |"), "1: unexpected end of line");
}

#[test]
fn children() {
    assert_eq!(read_str("a\n    b\n        c\n    d e\nf"), "a(b(c) d(e)); f");
}

#[test]
fn continuation_lines() {
    assert_eq!(read_str("a b\n    | c\n    || d | e\n    f\n        g"), "a(b c d e f(g))");
}

#[test]
fn continuation_after_child() {
    assert_eq!(error("a\n    b\n    | c"), "3: unexpected |");
}

#[test]
fn line_starting_with_record_string() {
    assert_eq!(error("|| a"), "1: unexpected || a");
}

#[test]
fn indented_first_line() {
    assert_eq!(error("    a"), "1: unexpected indentation");
}

#[test]
fn unclosed_and_stray_parentheses() {
    assert_eq!(error("a(b"), "1: unexpected end of line");
    assert_eq!(error("a b)"), "1: unexpected )");
    assert_eq!(error("a:"), "1: unexpected end of line");
}

#[test]
fn dictionary_reading() {
    assert_eq!(
        read_str("kanji 上 part:2\n    kun-group\n        reading #core | pri-spell 上がる | *あ*がる || to rise\n            variant rare:上る"),
        "kanji(上 part(2) kun-group(reading(#core pri-spell(上がる) *あ*がる \"to rise\" variant(rare(上る)))))"
    );
}

#[test]
fn line_numbers() {
    let args = read("a\n\n    b | c\n        d").unwrap();
    let a = &args.nodes[0];
    let b = &a.args[0];
    assert_eq!((a.line, b.line, b.args[0].line, b.args[1].line), (1, 3, 3, 4));
}

#[test]
fn args_cursor() {
    let mut root = read("a x #f y:z w").unwrap();
    let result = root.take_list(|name, args| {
        assert_eq!(name, "a");
        assert_eq!(args.take_string()?, "x");
        assert!(!args.take_flag("#g"));
        assert!(args.take_flag("#f"));
        let y = args.take_optional("y", |args| args.take_string())?;
        let missing = args.take_optional("y", |args| args.take_string())?;
        Ok((y, missing, args.take_string()?))
    });
    assert_eq!(result.unwrap(), (Some("z".into()), None, "w".into()));
    assert!(root.is_empty());
}

#[test]
fn args_leftover_and_missing() {
    let mut root = read("a x y").unwrap();
    let e = root.take_list(|_, args| args.take_string()).unwrap_err();
    assert_eq!(e.message, "unexpected y");
    let mut root = read("a").unwrap();
    let e = root.take_list(|_, args| args.take_string()).unwrap_err();
    assert_eq!(e.message, "missing item in a");
}

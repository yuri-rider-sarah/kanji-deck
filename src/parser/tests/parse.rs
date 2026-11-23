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

/// Parses the text and takes all its top-level records.
fn parse_records(text: &str) -> Result<Vec<Node>, Vec<Error>> {
    parse(text, |root| Ok(root.nodes.drain(..).collect()))
}

/// The top-level records, rendered as `a(b c)` and separated by `; `.
fn read_str(text: &str) -> String {
    parse_records(text).unwrap().iter().map(render).collect::<Vec<_>>().join("; ")
}

/// The single syntax error in the text, as `line:column: message`.
fn error(text: &str) -> String {
    match parse_records(text) {
        Ok(records) => {
            panic!("no error, got {}", records.iter().map(render).collect::<Vec<_>>().join("; "))
        }
        Err(errors) => {
            let [e] = errors.as_slice() else { panic!("expected one error, got {errors:?}") };
            format!("{}:{}: {}", e.line, e.column.expect("parser errors have a column"), e.message)
        }
    }
}

/// The messages of the errors, in order.
fn messages(errors: &[Error]) -> Vec<&str> {
    errors.iter().map(|e| e.message.as_str()).collect()
}

#[test]
fn plain_records() {
    assert_eq!(read_str("a\nb c d"), "a; b(c d)");
}

#[test]
fn nodes() {
    assert_eq!(read_str("a b:c:d e(f g(h)) i()"), "a(b(c(d)) e(f g(h)) i)");
    assert_eq!(read_str("a (b)\na : b"), "a(b); a(b)");
    assert_eq!(error("a(b):c"), "1:5: unexpected :");
    assert_eq!(read_str("a b(c:d \"e f\")"), "a(b(c(d) \"e f\"))");
}

#[test]
fn record_as_single_node() {
    assert_eq!(read_str("a(b c)\na:b"), "a(b c); a(b)");
}

#[test]
fn single_node_record_takes_nothing_more() {
    assert_eq!(error("a(b) c"), "1:6: unexpected string");
    assert_eq!(error("a:b c"), "1:5: unexpected string");
    assert_eq!(error("a:b | c"), "1:5: unexpected |");
    assert_eq!(error("a:b\n    c"), "2:5: unexpected indentation");
}

#[test]
fn segments() {
    assert_eq!(read_str("a b | c d | e || f g"), "a(b c(d) e \"f g\")");
    assert_eq!(read_str("a || b || c | d"), "a(b c d)");
    assert_eq!(read_str("a | b:c | d(e) | f"), "a(b(c) d(e) f)");
}

#[test]
fn segment_single_node_takes_nothing_more() {
    assert_eq!(error("a | b(c) d"), "1:10: unexpected string");
}

#[test]
fn empty_segment() {
    assert_eq!(error("a | | b"), "1:5: unexpected |");
    assert_eq!(error("a |"), "1:4: unexpected end of line");
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
    assert_eq!(error("a\n    b\n    | c"), "3:5: unexpected |");
}

#[test]
fn line_starting_with_record_string() {
    assert_eq!(error("|| a"), "1:1: unexpected record string");
}

#[test]
fn indented_first_line() {
    assert_eq!(error("    a"), "1:5: unexpected indentation");
}

#[test]
fn unclosed_and_stray_parentheses() {
    assert_eq!(error("a(b"), "1:4: unexpected end of line");
    assert_eq!(error("a b)"), "1:4: unexpected )");
    assert_eq!(error("a:"), "1:3: unexpected end of line");
}

#[test]
fn dictionary_reading() {
    assert_eq!(
        read_str(
            "kanji 上 part:2\n    kun-group\n        reading #core | pri-spell 上がる | *あ*がる || to rise\n            variant rare:上る"
        ),
        "kanji(上 part(2) kun-group(reading(#core pri-spell(上がる) *あ*がる \"to rise\" variant(rare(上る)))))"
    );
}

#[test]
fn positions() {
    let records = parse_records("a\n\n    b | c\n        d").unwrap();
    let a = &records[0];
    let b = &a.args[0];
    let positions = [a.pos, b.pos, b.args[0].pos, b.args[1].pos]
        .map(|pos| format!("{}:{}", pos.line, pos.column));
    assert_eq!(positions, ["1:1", "3:5", "3:9", "4:9"]);
}

#[test]
fn args_cursor() {
    let result = parse("a x #f y:z w", |root| {
        Ok(root.take_one(|name, args| {
            assert_eq!(name, "a");
            assert_eq!(args.take_string()?, "x");
            assert!(!args.take_flag("#g"));
            assert!(args.take_flag("#f"));
            let y = args.take_optional_by_name("y", |args| args.take_string())?;
            let missing = args.take_optional_by_name("y", |args| args.take_string())?;
            Ok((y, missing, args.take_string()?))
        })?)
    });
    assert_eq!(result.unwrap(), (Some("z".to_string()), None, "w".to_string()));
}

#[test]
fn args_leftover_and_missing() {
    let errors = parse("a x y", |root| Ok(root.take_one(|_, args| args.take_string())?));
    assert_eq!(messages(&errors.unwrap_err()), ["unexpected y"]);
    let errors = parse("a", |root| Ok(root.take_one(|_, args| args.take_string())?));
    assert_eq!(messages(&errors.unwrap_err()), ["expected more arguments"]);
}

#[test]
fn args_many_by_names_collecting_errors() {
    let parsers: [(&str, fn(&mut Args) -> Result<String>); 2] =
        [("a", |args| args.take_string()), ("b", |_| Ok("b".to_string()))];
    let errors = parse("a x\nb\na x y\nb z\na x\nc", |root| {
        let errors = root.take_many_by_names_collecting_errors(&parsers).unwrap_err();
        assert_eq!(root.take_string().unwrap(), "c");
        Err::<(), _>(errors)
    });
    assert_eq!(messages(&errors.unwrap_err()), ["unexpected y", "unexpected z"]);
    let values = parse("a x\nb", |root| root.take_many_by_names_collecting_errors(&parsers));
    assert_eq!(values.unwrap(), ["x", "b"]);
}

#[test]
fn leftover_records() {
    let errors = parse("a\nb", |root| Ok(root.take_string()?));
    assert_eq!(messages(&errors.unwrap_err()), ["unexpected b"]);
    // The leftover error comes after the errors of the callback.
    let errors = parse("a x\nb", |root| Ok(root.take_string()?));
    assert_eq!(messages(&errors.unwrap_err()), ["unexpected x", "unexpected b"]);
}

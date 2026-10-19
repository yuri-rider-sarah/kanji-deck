//! Converts the tree of nodes from `parser` into dictionary entries.
//! Only the basic structure is checked here; the contents are validated by `check`.
//! Errors are collected per entry, so all broken entries are reported at once.

use super::*;
use crate::parser::Args;

pub fn convert(mut root: Args) -> Result<Vec<Entry>, Vec<Error>> {
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    let mut last_part_no = None;
    loop {
        let mut parse_entry = |head: String, args: &mut Args| {
            let (entry_type, part_no) = match head.as_str() {
                "kanji" => {
                    let name = args.take_string()?;
                    let mut chars = name.chars();
                    let (Some(kanji), None) = (chars.next(), chars.next()) else {
                        return Err(Error::new(
                            args.pos(),
                            format!("{name} is not a single character"),
                        ));
                    };
                    (EntryType::Kanji(kanji), parse_part_no(args)?)
                }
                "word" => {
                    let headword = args.take_string()?;
                    let part_no = parse_part_no(args)?;
                    let introduced_kanji = match args
                        .take_optional_by_name("introduces", |args| args.take_string())?
                    {
                        Some(introduced_kanji) => introduced_kanji.chars().collect(),
                        None => Vec::new(),
                    };
                    (EntryType::Word { headword, introduced_kanji }, part_no)
                }
                _ => return Err(Error::unexpected_node(args.pos(), &head)),
            };
            let part = parse_part(args)?;
            match part_no {
                None | Some(1) => {
                    entries.push(Entry { line: args.pos().line, entry_type, parts: vec![part] });
                }
                Some(_)
                    if matches!(&entry_type, EntryType::Word { introduced_kanji, .. }
                        if !introduced_kanji.is_empty()) =>
                {
                    return Err(Error::new(args.pos(), "introduces is only allowed in part:1"));
                }
                // A part number above 1 must follow the previous part of the same entry.
                Some(part_no) => match entries.last_mut() {
                    Some(entry)
                        if is_same_entry(&entry.entry_type, &entry_type)
                            && last_part_no == Some(part_no - 1) =>
                    {
                        entry.parts.push(part);
                    }
                    _ => {
                        return Err(Error::new(
                            args.pos(),
                            format!("part:{part_no} doesn't follow part:{}", part_no - 1),
                        ));
                    }
                },
            }
            last_part_no = part_no;
            Ok(())
        };
        match root.take_optional(&mut parse_entry) {
            Ok(Some(())) => (),
            Ok(None) => break,
            Err(e) => {
                last_part_no = None;
                errors.push(e);
            }
        }
    }
    if errors.is_empty() { Ok(entries) } else { Err(errors) }
}

/// Whether two entry types belong to the same entry: the same kanji, or the
/// same headword. Words only list introduced kanji in their first part.
fn is_same_entry(first: &EntryType, other: &EntryType) -> bool {
    match (first, other) {
        (EntryType::Kanji(first), EntryType::Kanji(other)) => first == other,
        (EntryType::Word { headword: first, .. }, EntryType::Word { headword: other, .. }) => {
            first == other
        }
        _ => false,
    }
}

/// Parses the optional part number of an entry, `part:N` with N above 0.
fn parse_part_no(args: &mut Args) -> Result<Option<usize>> {
    args.take_optional_by_name("part", |args| {
        args.take_string()?
            .parse()
            .ok()
            .filter(|&n| n > 0)
            .ok_or_else(|| Error::new(args.pos(), "bad part number"))
    })
}

/// Parses the groups of a part, which must be in the order on, kun.
fn parse_part(args: &mut Args) -> Result<EntryPart> {
    Ok(EntryPart {
        on: args.take_many_by_name("on-group", parse_group)?,
        kun: args.take_many_by_name("kun-group", parse_group)?,
    })
}

fn parse_group(args: &mut Args) -> Result<ReadingGroup> {
    args.take_many_by_name("reading", parse_reading)
}

fn parse_reading(args: &mut Args) -> Result<Reading> {
    let core = args.take_flag("#core");
    let regular = args.take_flag("#regular");
    let origin = if args.take_flag("#asian") {
        Some(ReadingOrigin::Asian)
    } else if args.take_flag("#european") {
        Some(ReadingOrigin::European)
    } else {
        None
    };
    Ok(Reading {
        line: args.pos().line,
        core,
        regular,
        origin,
        spelling: args.take_one(parse_spelling)?,
        kana: args.take_one(parse_kana)?,
        definition: args.take_string()?,
        variants: args.take_many_by_name("variant", |args| args.take_one(freq_word))?,
    })
}

fn parse_kana(kana: String, args: &mut Args) -> Result<Kana> {
    if !kana.contains('*') {
        return Ok(Kana::Unsplit(kana));
    }
    match kana.split('*').collect::<Vec<_>>()[..] {
        [pre, mid, post] => {
            Ok(Kana::Split { pre: pre.to_string(), mid: mid.to_string(), post: post.to_string() })
        }
        _ => Err(Error::new(args.pos(), "kana must have zero or two asterisks")),
    }
}

fn parse_spelling(head: String, args: &mut Args) -> Result<Spelling> {
    match head.as_str() {
        "pri-spell" => Ok(Spelling::Primary(args.take_string()?)),
        "pri-kanji-spell" => Ok(Spelling::PrimaryKanji {
            main: args.take_string()?,
            described: args.take_one(freq_word)?,
        }),
        "sec-spell" => Ok(Spelling::Secondary {
            main: args.take_string()?,
            described: args.take_one(freq_word)?,
        }),
        "sec-kanji-spell" => Ok(Spelling::SecondaryKanji {
            main: args.take_string()?,
            main_kanji: args.take_one(freq_word)?,
            described: args.take_one(freq_word)?,
        }),
        _ => Err(Error::unexpected_node(args.pos(), &head)),
    }
}

fn freq_word(head: String, args: &mut Args) -> Result<FreqWord> {
    let freq = match head.as_str() {
        "common" => Freq::Common,
        "rare" => Freq::Rare,
        _ => return Err(Error::unexpected_node(args.pos(), &head)),
    };
    Ok(FreqWord { freq, word: args.take_string()? })
}

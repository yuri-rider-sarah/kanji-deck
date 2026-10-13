//! The second phase of parsing the dictionary: converting the tree of nodes
//! into entries. Only the basic structure is checked here; the contents are
//! validated by `check`. Errors are collected per entry, so all broken entries
//! are reported at once.

use super::read::{Args, read};
use super::*;

pub fn parse(text: &str) -> Result<Vec<Entry>, Vec<Error>> {
    let mut root = read(text).map_err(|e| vec![e])?;
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    let mut last_part_no = None;
    loop {
        let mut parse_entry = |head: String, args: &mut Args| match head.as_str() {
            "kanji" => {
                let name = args.take_string()?;
                let mut chars = name.chars();
                let (Some(kanji), None) = (chars.next(), chars.next()) else {
                    return Err(Error::new(args.get_pos(), format!("{name} is not a single character")));
                };
                let part_no = args.take_optional_by_name("part", |args| {
                    args.take_string()?.parse().ok().filter(|&n| n > 0).ok_or_else(|| Error::new(args.get_pos(), "bad part number"))
                })?;
                let data = KanjiPart {
                    on: args.take_many_by_name("on-group", parse_group)?,
                    kun: args.take_many_by_name("kun-group", parse_group)?,
                    combined: args.take_many_by_name("combined-group", parse_group)?,
                };
                match part_no {
                    None | Some(1) => {
                        entries.push(Entry::Kanji(KanjiEntry { pos: args.get_pos(), kanji, parts: vec![data] }));
                    },
                    Some(part_no) => {
                        // If there was a part number above 1, it must follow the part number in the last entry.
                        match entries.last_mut() {
                            Some(Entry::Kanji(entry)) if entry.kanji == kanji && last_part_no == Some(part_no - 1) => {
                                entry.parts.push(data);
                            },
                            _ => {
                                return Err(Error { pos: args.get_pos(), message: format!("part:{part_no} doesn't follow part:{}", part_no - 1) });
                            },
                        }
                    },
                }
                last_part_no = part_no;
                Ok(())
            }
            "word" => {
                let headword = args.take_string()?;
                let readings = args.take_many_by_name("group", parse_group)?;
                entries.push(Entry::Word(WordEntry { pos: args.get_pos(), headword, readings }));
                last_part_no = None;
                Ok(())
            }
            _ => Err(Error::unexpected_node(args.get_pos(), &head)),
        };
        match root.take_optional(&mut parse_entry) {
            Ok(Some(())) => (),
            Ok(None) => break,
            Err(e) => {
                last_part_no = None;
                errors.push(e);
            },
        }
    }
    if errors.is_empty() { Ok(entries) } else { Err(errors) }
}

fn parse_group(args: &mut Args) -> Result<ReadingGroup> {
    args.take_many_by_name("reading", parse_reading)
}

fn parse_reading(args: &mut Args) -> Result<Reading> {
    let core = args.take_flag("#core");
    let origin = if args.take_flag("#asian") {
        Some(ReadingOrigin::Asian)
    } else if args.take_flag("#european") {
        Some(ReadingOrigin::European)
    } else {
        None
    };
    Ok(Reading {
        pos: args.get_pos(),
        core,
        origin,
        spelling: args.take_one(parse_spelling)?,
        kana: args.take_one(parse_kana)?,
        definition: args.take_string()?,
        variants: args.take_many_by_name("variant", |args| args.take_one(freq_word))?,
    })
}

/// Kana like `*あ*がる`, where the part between asterisks belongs to the kanji.
fn parse_kana(kana: String, args: &mut Args) -> Result<Kana> {
    if !kana.contains('*') {
        return Ok(Kana::Unsplit(kana));
    }
    match kana.split('*').collect::<Vec<_>>()[..] {
        [pre, mid, post] => Ok(Kana::Split { pre: pre.into(), mid: mid.into(), post: post.into() }),
        _ => Err(Error::new(args.get_pos(), "kana must have zero or two asterisks")),
    }
}

fn parse_spelling(head: String, args: &mut Args) -> Result<Spelling> {
    match head.as_str() {
        "pri-spell" => Ok(Spelling::Primary(args.take_string()?)),
        "pri-kanji-spell" => Ok(Spelling::PrimaryKanji { main: args.take_string()?, kanji: args.take_one(freq_word)? }),
        "sec-spell" => Ok(Spelling::Secondary { main: args.take_string()?, described: args.take_one(freq_word)? }),
        "sec-kanji-spell" => Ok(Spelling::SecondaryKanji {
            main: args.take_string()?,
            kanji: args.take_one(freq_word)?,
            described: args.take_one(freq_word)?,
        }),
        _ => Err(Error::unexpected_node(args.get_pos(), &head)),
    }
}

fn freq_word(head: String, args: &mut Args) -> Result<FreqWord> {
    let freq = match head.as_str() {
        "common" => Freq::Common,
        "rare" => Freq::Rare,
        _ => return Err(Error::unexpected_node(args.get_pos(), &head)),
    };
    Ok(FreqWord { freq, word: args.take_string()? })
}

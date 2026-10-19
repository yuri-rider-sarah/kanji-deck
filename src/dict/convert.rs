//! Converts the tree of nodes from `parser` into dictionary entries.
//! Only the basic structure is checked here; the contents are validated by `check`.
//! Errors are collected per entry, so all broken entries are reported at once.

use super::*;
use crate::parser::{Args, Pos};

pub fn convert(mut root: Args) -> Result<Vec<Entry>, Vec<Error>> {
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    let mut last_part_no = None;
    loop {
        let mut parse_entry = |head: String, args: &mut Args| {
            let part_no = match head.as_str() {
                "kanji" => parse_kanji_entry(args, &mut entries, last_part_no)?,
                "word" => parse_word_entry(args, &mut entries, last_part_no)?,
                _ => return Err(Error::unexpected_node(args.pos(), &head)),
            };
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

/// Parses a part of a kanji entry, and adds it to `entries` as a new entry or
/// as the next part of the last one, whose part number is `last_part_no`.
/// Returns the part number of the new part.
fn parse_kanji_entry(
    args: &mut Args,
    entries: &mut Vec<Entry>,
    last_part_no: Option<usize>,
) -> Result<Option<usize>> {
    let name = args.take_string()?;
    let mut chars = name.chars();
    let (Some(kanji), None) = (chars.next(), chars.next()) else {
        return Err(Error::new(args.pos(), format!("{name} is not a single character")));
    };
    let part_no = parse_part_no(args)?;
    let part = parse_part(args)?;
    match part_no {
        None | Some(1) => {
            entries.push(Entry::Kanji(KanjiEntry {
                line: args.pos().line,
                kanji,
                parts: vec![part],
            }));
        }
        Some(part_no) => match entries.last_mut() {
            Some(Entry::Kanji(last))
                if last.kanji == kanji && last_part_no == Some(part_no - 1) =>
            {
                last.parts.push(part);
            }
            _ => return Err(part_order_error(args.pos(), part_no)),
        },
    }
    Ok(part_no)
}

/// Parses a part of a word entry, and adds it to `entries` as a new entry or
/// as the next part of the last one, whose part number is `last_part_no`.
/// Returns the part number of the new part.
fn parse_word_entry(
    args: &mut Args,
    entries: &mut Vec<Entry>,
    last_part_no: Option<usize>,
) -> Result<Option<usize>> {
    let headword = args.take_string()?;
    let part_no = parse_part_no(args)?;
    let introduced_kanji: Vec<char> =
        match args.take_optional_by_name("introduces", |args| args.take_string())? {
            Some(introduced_kanji) => introduced_kanji.chars().collect(),
            None => Vec::new(),
        };
    let part = parse_part(args)?;
    match part_no {
        None | Some(1) => {
            entries.push(Entry::Word(WordEntry {
                line: args.pos().line,
                headword,
                introduced_kanji,
                parts: vec![part],
            }));
        }
        Some(_) if !introduced_kanji.is_empty() => {
            return Err(Error::new(args.pos(), "introduces is only allowed in part:1"));
        }
        Some(part_no) => match entries.last_mut() {
            Some(Entry::Word(last))
                if last.headword == headword && last_part_no == Some(part_no - 1) =>
            {
                last.parts.push(part);
            }
            _ => return Err(part_order_error(args.pos(), part_no)),
        },
    }
    Ok(part_no)
}

/// The error for a part number above 1 that doesn't follow the previous part
/// of the same entry.
fn part_order_error(pos: Pos, part_no: usize) -> Error {
    Error::new(pos, format!("part:{part_no} doesn't follow part:{}", part_no - 1))
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

/// Parses the groups of a part, which must be in the order on, kun, asian,
/// european.
fn parse_part<K: ParseKana>(args: &mut Args) -> Result<EntryPart<K>> {
    Ok(EntryPart {
        on: args.take_many_by_name("on-group", parse_group)?,
        kun: args.take_many_by_name("kun-group", parse_group)?,
        asian: args.take_many_by_name("asian-group", parse_group)?,
        european: args.take_many_by_name("european-group", parse_group)?,
    })
}

fn parse_group<K: ParseKana>(args: &mut Args) -> Result<ReadingGroup<K>> {
    args.take_many_by_name("reading", parse_reading)
}

fn parse_reading<K: ParseKana>(args: &mut Args) -> Result<Reading<K>> {
    let core = args.take_flag("#core");
    let regular = args.take_flag("#regular");
    Ok(Reading {
        line: args.pos().line,
        core,
        regular,
        spelling: args.take_one(parse_spelling)?,
        kana: args.take_one(K::parse_kana)?,
        definition: args.take_string()?,
        variants: args.take_many_by_name("variant", |args| args.take_one(freq_word))?,
    })
}

/// Kana of a reading, parsed differently for kanji and word entries.
trait ParseKana: Sized {
    fn parse_kana(kana: String, args: &mut Args) -> Result<Self>;
}

impl ParseKana for SplitKana {
    fn parse_kana(kana: String, args: &mut Args) -> Result<SplitKana> {
        match kana.split('*').collect::<Vec<_>>()[..] {
            [pre, mid, post] => {
                Ok(SplitKana { pre: pre.to_string(), mid: mid.to_string(), post: post.to_string() })
            }
            _ => Err(Error::new(args.pos(), "kana in kanji entries must have two asterisks")),
        }
    }
}

impl ParseKana for UnsplitKana {
    fn parse_kana(kana: String, args: &mut Args) -> Result<UnsplitKana> {
        if kana.contains('*') {
            return Err(Error::new(args.pos(), "kana in word entries can't have asterisks"));
        }
        Ok(UnsplitKana(kana))
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

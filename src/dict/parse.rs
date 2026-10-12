//! The second phase of parsing the dictionary: converting the nodes from
//! `read` into entries. Only the structure is checked here; the contents are
//! validated by `check`. Errors are collected per entry, so all broken entries
//! are reported at once.

use super::read::{Args, read};
use super::*;

/// An entry as written in the file, where each part of a kanji is separate.
enum Item {
    Kanji { pos: Pos, kanji: String, part: Option<usize>, data: KanjiPart },
    Word(WordEntry),
}

pub fn parse(text: &str) -> Result<Vec<Entry>, Vec<Error>> {
    let mut root = read(text).map_err(|e| vec![e])?;
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    while !root.is_empty() {
        match root.take_list(item) {
            Ok(Item::Word(entry)) => entries.push(Entry::Word(entry)),
            Ok(Item::Kanji { pos, kanji, part, data }) => {
                // Parts of a kanji come one after another, numbered from 1.
                let previous = match entries.last_mut() {
                    Some(Entry::Kanji(entry)) if entry.kanji == kanji => Some(entry),
                    _ => None,
                };
                match (part, previous) {
                    (None | Some(1), _) => entries.push(Entry::Kanji(KanjiEntry { pos, kanji, parts: vec![data] })),
                    (Some(n), Some(entry)) if entry.parts.len() == n - 1 => entry.parts.push(data),
                    (Some(n), _) => errors.push(Error { pos, message: format!("part:{n} doesn't follow part:{}", n - 1) }),
                }
            }
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() { Ok(entries) } else { Err(errors) }
}

fn item(head: &str, args: &mut Args) -> Result<Item> {
    match head {
        "kanji" => {
            let kanji = args.take_string()?;
            let part = match args.take_optional("part", |args| args.take_string())? {
                Some(part) => Some(part.parse().ok().filter(|&n| n > 0).ok_or_else(|| args.error("bad part number"))?),
                None => None,
            };
            let data = KanjiPart {
                on: args.take_many("on-group", group)?,
                kun: args.take_many("kun-group", group)?,
                combined: args.take_many("combined-group", group)?,
            };
            Ok(Item::Kanji { pos: args.pos, kanji, part, data })
        }
        "word" => {
            let headword = args.take_string()?;
            let readings = args.take_many("group", group)?;
            if readings.is_empty() {
                return Err(args.error("word without readings"));
            }
            Ok(Item::Word(WordEntry { pos: args.pos, headword, readings }))
        }
        _ => Err(args.unexpected()),
    }
}

fn group(args: &mut Args) -> Result<ReadingGroup> {
    let readings = args.take_many("reading", reading)?;
    if readings.is_empty() {
        return Err(args.error("empty group"));
    }
    Ok(readings)
}

fn reading(args: &mut Args) -> Result<Reading> {
    let core = args.take_flag("#core");
    let origin = if args.take_flag("#asian") {
        Some(ReadingOrigin::Asian)
    } else if args.take_flag("#european") {
        Some(ReadingOrigin::European)
    } else {
        None
    };
    Ok(Reading {
        pos: args.pos,
        core,
        origin,
        spelling: args.take_list(spelling)?,
        kana: kana(args)?,
        definition: args.take_string()?,
        variants: args.take_many("variant", |args| args.take_list(freq_word))?,
    })
}

/// Kana like `*あ*がる`, where the part between asterisks belongs to the kanji.
fn kana(args: &mut Args) -> Result<Kana> {
    let kana = args.take_string()?;
    if !kana.contains('*') {
        return Ok(Kana::Unsplit(kana));
    }
    match kana.split('*').collect::<Vec<_>>()[..] {
        [pre, mid, post] => Ok(Kana::Split { pre: pre.into(), mid: mid.into(), post: post.into() }),
        _ => Err(args.error("kana must have zero or two asterisks")),
    }
}

fn spelling(head: &str, args: &mut Args) -> Result<Spelling> {
    match head {
        "pri-spell" => Ok(Spelling::Primary(args.take_string()?)),
        "pri-kanji-spell" => Ok(Spelling::PrimaryKanji { main: args.take_string()?, kanji: args.take_list(freq_word)? }),
        "sec-spell" => Ok(Spelling::Secondary { main: args.take_string()?, described: args.take_list(freq_word)? }),
        "sec-kanji-spell" => Ok(Spelling::SecondaryKanji {
            main: args.take_string()?,
            kanji: args.take_list(freq_word)?,
            described: args.take_list(freq_word)?,
        }),
        _ => Err(args.unexpected()),
    }
}

fn freq_word(head: &str, args: &mut Args) -> Result<FreqWord> {
    let freq = match head {
        "common" => Freq::Common,
        "rare" => Freq::Rare,
        _ => return Err(args.unexpected()),
    };
    Ok(FreqWord { freq, word: args.take_string()? })
}

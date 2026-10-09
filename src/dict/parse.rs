//! The second phase of parsing the dictionary: converting the tree from `read`
//! into entries. Only the structure is checked here; the contents are
//! validated by `check`. Errors are collected per entry, so all broken entries
//! are reported at once.

use super::read::{Args, Node, read};
use super::*;

pub fn parse(text: &str) -> Result<Vec<Entry>, Vec<Error>> {
    let nodes = read(text).map_err(|e| vec![e])?;
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    for node in nodes {
        match entry(node) {
            Ok(e) => entries.push(e),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() { Ok(entries) } else { Err(errors) }
}

fn entry(node: Node) -> Result<Entry, Error> {
    node.take_list(|head, args| match head {
        "kanji" => {
            let kanji = args.take_atom()?;
            let parts = vec![part(String::new(), args)?];
            Ok(Entry::Kanji(KanjiEntry { line: args.line, kanji, parts }))
        }
        "kanji-split" => {
            let kanji = args.take_atom()?;
            let parts = args.take_many("part", |args| part(args.take_string()?, args))?;
            if parts.is_empty() {
                return Err(args.error("kanji-split without parts"));
            }
            Ok(Entry::Kanji(KanjiEntry { line: args.line, kanji, parts }))
        }
        "word" => Ok(Entry::Word(WordEntry { line: args.line, readings: groups(args)? })),
        _ => Err(args.unexpected()),
    })
}

/// The `on`, `kun` and `combined` lists of a kanji, each optional, in this order.
fn part(name: String, args: &mut Args) -> Result<KanjiPart, Error> {
    Ok(KanjiPart {
        name,
        on: args.take_optional("on", groups)?.unwrap_or_default(),
        kun: args.take_optional("kun", groups)?.unwrap_or_default(),
        combined: args.take_optional("combined", groups)?.unwrap_or_default(),
    })
}

const READINGS: &[&str] = &["primary-reading", "secondary-reading"];

/// Groups of readings, each written as a reading with the rest of the group
/// nested inside it.
fn groups(args: &mut Args) -> Result<Vec<Group>, Error> {
    let groups = args.take_many_with(READINGS, |head, args| {
        let mut group = vec![reading(head, args)?];
        group.extend(args.take_many_with(READINGS, |head, args| {
            let mut related = reading(head, args)?;
            related.attrs = attrs(args)?;
            Ok(related)
        })?);
        // The attributes of the first reading come after the rest of the group.
        group[0].attrs = attrs(args)?;
        Ok(group)
    })?;
    if groups.is_empty() {
        return Err(args.error("empty list of readings"));
    }
    Ok(groups)
}

/// A reading without its attributes, which come after the nested readings.
fn reading(head: &str, args: &mut Args) -> Result<Reading, Error> {
    Ok(Reading {
        line: args.line,
        primary: head == "primary-reading",
        spelling: args.take_list(spelling)?,
        kana: kana(args)?,
        definition: args.take_string()?,
        variants: args.take_many("variant", |args| args.take_list(freq_word))?,
        attrs: Vec::new(),
    })
}

fn attrs(args: &mut Args) -> Result<Vec<Attr>, Error> {
    args.take_many("attr", |args| match args.take_atom()?.as_str() {
        "asian" => Ok(Attr::Asian),
        "european" => Ok(Attr::European),
        _ => Err(args.error("unknown attribute")),
    })
}

/// Kana like `*あ*がる`, where the part between asterisks belongs to the kanji.
fn kana(args: &mut Args) -> Result<Kana, Error> {
    let kana = args.take_atom()?;
    if !kana.contains('*') {
        return Ok(Kana::Unsplit(kana));
    }
    match kana.split('*').collect::<Vec<_>>()[..] {
        [pre, mid, post] => Ok(Kana::Split { pre: pre.into(), mid: mid.into(), post: post.into() }),
        _ => Err(args.error("kana must have zero or two asterisks")),
    }
}

fn spelling(head: &str, args: &mut Args) -> Result<Spelling, Error> {
    match head {
        "primary-spelling" => Ok(Spelling::Primary(args.take_atom()?)),
        "primary-kanji-spelling" => {
            Ok(Spelling::PrimaryKanji { main: args.take_atom()?, kanji: args.take_list(freq_word)? })
        }
        "secondary-spelling" => {
            Ok(Spelling::Secondary { main: args.take_atom()?, described: args.take_list(freq_word)? })
        }
        "secondary-kanji-spelling" => Ok(Spelling::SecondaryKanji {
            main: args.take_atom()?,
            kanji: args.take_list(freq_word)?,
            described: args.take_list(freq_word)?,
        }),
        _ => Err(args.unexpected()),
    }
}

fn freq_word(head: &str, args: &mut Args) -> Result<FreqWord, Error> {
    let freq = match head {
        "common" => Freq::Common,
        "rare" => Freq::Rare,
        _ => return Err(args.unexpected()),
    };
    Ok(FreqWord { freq, word: args.take_atom()? })
}

//! Converts the tree of nodes from `parser` into dictionary entries.
//! Only the basic structure is checked here; the contents are validated by `check`.
//! Errors are collected per entry, so all broken entries are reported at once.

use super::*;
use crate::parser::{Args, unexpected_node_error};

/// A top-level node of the file that's an entry, or a part of a kanji entry.
enum EntryNode {
    KanjiPart { line: usize, kanji: char, part_no: Option<usize>, part: EntryPart<SplitKana> },
    Word(WordEntry),
}

/// Converts the top-level nodes of the file into the dictionary. Fails with all errors found.
pub fn convert(root: &mut Args) -> Result<Dictionary, Vec<Error>> {
    let mut errors = Vec::new();
    let entry_nodes = match root.take_many_by_names_collecting_errors(&[
        ("kanji", parse_kanji_part),
        ("word", parse_word_entry),
    ]) {
        Ok(entry_nodes) => entry_nodes,
        Err(node_errors) => {
            errors.extend(node_errors);
            Vec::new()
        }
    };
    let entries = match entries_from_nodes(entry_nodes) {
        Ok(entries) => entries,
        Err(entry_errors) => {
            errors.extend(entry_errors);
            Vec::new()
        }
    };
    let rendaku_suffixes = match root.take_optional_by_name("rendaku-suffixes", |args| {
        args.take_many_by_name("suffix", parse_rendaku_suffix)
    }) {
        Ok(rendaku_suffixes) => rendaku_suffixes.unwrap_or_default(),
        Err(error) => {
            errors.push(error);
            Vec::new()
        }
    };
    if errors.is_empty() { Ok(Dictionary { entries, rendaku_suffixes }) } else { Err(errors) }
}

/// Makes the entries from the entry nodes, adding each part of a kanji entry after the first
/// to the entry before it.
fn entries_from_nodes(entry_nodes: Vec<EntryNode>) -> Result<Vec<Entry>, Vec<Error>> {
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    // The part number of the last node, for merging kanji entries.
    let mut last_part_no = None;
    for entry_node in entry_nodes {
        match entry_node {
            EntryNode::KanjiPart { line, kanji, part_no: part_no @ (None | Some(1)), part } => {
                entries.push(Entry::Kanji(KanjiEntry { line, kanji, parts: vec![part] }));
                last_part_no = part_no;
            }
            EntryNode::KanjiPart { line, kanji, part_no: Some(part_no), part } => {
                match entries.last_mut() {
                    Some(Entry::Kanji(last))
                        if last.kanji == kanji && last_part_no == Some(part_no - 1) =>
                    {
                        last.parts.push(part);
                        last_part_no = Some(part_no);
                    }
                    _ => {
                        errors.push(Error::at_line(
                            line,
                            format!("part:{part_no} doesn't follow part:{}", part_no - 1),
                        ));
                        last_part_no = None;
                    }
                }
            }
            EntryNode::Word(entry) => {
                entries.push(Entry::Word(entry));
                last_part_no = None;
            }
        }
    }
    if errors.is_empty() { Ok(entries) } else { Err(errors) }
}

/// Parses a part of a kanji entry, which may be the whole entry.
fn parse_kanji_part(args: &mut Args) -> Result<EntryNode> {
    let line = args.pos().line;
    let name = args.take_string()?;
    let mut chars = name.chars();
    let (Some(kanji), None) = (chars.next(), chars.next()) else {
        return Err(Error::new(args.pos(), format!("{name} is not a single character")));
    };
    let part_no = parse_part_no(args)?;
    let part = parse_part(args)?;
    Ok(EntryNode::KanjiPart { line, kanji, part_no, part })
}

/// Parses a word entry, which unlike a kanji entry is never split into parts.
fn parse_word_entry(args: &mut Args) -> Result<EntryNode> {
    let headword = args.take_string()?;
    let introduced_kanji =
        match args.take_optional_by_name("introduces", |args| args.take_string())? {
            Some(introduced_kanji) => introduced_kanji.chars().collect(),
            None => Vec::new(),
        };
    let part = parse_part(args)?;
    Ok(EntryNode::Word(WordEntry { line: args.pos().line, headword, introduced_kanji, part }))
}

/// Parses a rendaku suffix, `suffix SPELLING KANA`.
fn parse_rendaku_suffix(args: &mut Args) -> Result<RendakuSuffix> {
    Ok(RendakuSuffix {
        line: args.pos().line,
        spelling: args.take_string()?,
        kana: args.take_string()?,
    })
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

/// A variant with its canonical spellings, as written in the file.
struct ParsedVariant {
    variant: FreqWord,
    canonical_spellings: Vec<String>,
}

fn parse_reading<K: ParseKana>(args: &mut Args) -> Result<Reading<K>> {
    let line = args.pos().line;
    let core = args.take_flag("#core");
    let regular = args.take_flag("#regular");
    let rendaku_after = args.take_flag("#rendaku-after");
    let spelling = args.take_one(parse_spelling)?;
    let kana = args.take_one(K::parse_kana)?;
    let definition = args.take_string()?;
    let mut canonicals = args.take_many_by_name("canonical", parse_canonical)?;
    let parsed_variants = args.take_many_by_name("variant", parse_variant)?;
    let part_of_speech = args.take_optional_by_name("part-of-speech", parse_part_of_speech)?;
    // Group the canonical spellings of the variants by the canonical form they belong to.
    let mut variants = Vec::new();
    for parsed_variant in parsed_variants {
        let variant_spelling = &parsed_variant.variant.word;
        if parsed_variant.canonical_spellings.len() != canonicals.len() {
            return Err(Error::at_line(
                line,
                format!(
                    "variant {variant_spelling} has {} canonical spellings, \
                     while its reading has {}",
                    parsed_variant.canonical_spellings.len(),
                    canonicals.len()
                ),
            ));
        }
        for (canonical, canonical_spelling) in
            canonicals.iter_mut().zip(parsed_variant.canonical_spellings)
        {
            canonical.variant_spellings.push(canonical_spelling);
        }
        variants.push(parsed_variant.variant);
    }
    Ok(Reading {
        line,
        core,
        regular,
        rendaku_after,
        spelling,
        kana,
        definition,
        canonicals,
        variants,
        part_of_speech,
    })
}

/// Parses a canonical spelling and kana of a reading, `canonical SPELLING KANA`.
/// The variant spellings are added once the variants are parsed.
fn parse_canonical<K: ParseKana>(args: &mut Args) -> Result<Canonical<K>> {
    Ok(Canonical {
        spelling: args.take_string()?,
        kana: args.take_one(K::parse_kana)?,
        variant_spellings: Vec::new(),
    })
}

/// Parses a variant, `variant FREQ:W`, with `canonical SPELLING` children.
fn parse_variant(args: &mut Args) -> Result<ParsedVariant> {
    Ok(ParsedVariant {
        variant: args.take_one(freq_word)?,
        canonical_spellings: args.take_many_by_name("canonical", |args| args.take_string())?,
    })
}

/// Parses the part of speech of a reading, `part-of-speech P`.
fn parse_part_of_speech(args: &mut Args) -> Result<PartOfSpeech> {
    let name = args.take_string()?;
    match name.as_str() {
        "verb-ichidan" => Ok(PartOfSpeech::IchidanVerb),
        "verb-godan" => Ok(PartOfSpeech::GodanVerb),
        "verb-kuru" => Ok(PartOfSpeech::KuruVerb),
        "verb-suru" => Ok(PartOfSpeech::SuruVerb),
        "adjective" => Ok(PartOfSpeech::Adjective),
        "other" => Ok(PartOfSpeech::Other),
        "noun" => Ok(PartOfSpeech::Noun),
        _ => Err(Error::new(args.pos(), format!("unknown part of speech: {name}"))),
    }
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
        _ => Err(unexpected_node_error(args.pos(), &head)),
    }
}

fn freq_word(head: String, args: &mut Args) -> Result<FreqWord> {
    let freq = match head.as_str() {
        "common" => Freq::Common,
        "rare" => Freq::Rare,
        _ => return Err(unexpected_node_error(args.pos(), &head)),
    };
    Ok(FreqWord { freq, word: args.take_string()? })
}

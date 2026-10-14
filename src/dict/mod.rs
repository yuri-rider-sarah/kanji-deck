//! The dictionary data model, with `load` to read and check a dictionary from
//! the nodes of its file and `serialize` to write it back as text.

mod check;
mod convert;
mod serialize;

pub use serialize::serialize;

use crate::error::{Error, Result};
use crate::parser::Args;
use std::ops::Index;

/// Reads the dictionary from the top-level nodes of its file, and checks its
/// contents. Fails with all errors found.
pub fn load(nodes: Args) -> Result<Vec<Entry>, Vec<Error>> {
    let entries = convert::convert(nodes)?;
    check::check(&entries)?;
    Ok(entries)
}

/// The relative frequency of a spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Freq {
    Rare,
    Common,
}

/// A spelling along with its frequency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreqWord {
    pub freq: Freq,
    pub word: String,
}

/// The given spellings of a word, in order from most to least frequent.
#[derive(Debug, Clone)]
pub enum Spelling {
    Primary(String),
    PrimaryKanji(String, FreqWord),
    Secondary(String, FreqWord),
    SecondaryKanji(String, FreqWord, FreqWord),
}

impl Spelling {
    /// The most common spelling of the word.
    pub fn main(&self) -> &str {
        match self {
            Spelling::Primary(main)
            | Spelling::PrimaryKanji(main, _)
            | Spelling::Secondary(main, _)
            | Spelling::SecondaryKanji(main, _, _) => main,
        }
    }

    /// The spelling of the word described in the entry.
    pub fn described(&self) -> &str {
        match self {
            Spelling::Primary(described) => described,
            Spelling::PrimaryKanji(_, described)
            | Spelling::Secondary(_, described)
            | Spelling::SecondaryKanji(_, _, described) => &described.word,
        }
    }

    /// The frequency of the described spelling.
    pub fn described_freq(&self) -> Freq {
        match self {
            Spelling::Primary(_) => Freq::Common,
            Spelling::PrimaryKanji(_, described)
            | Spelling::Secondary(_, described)
            | Spelling::SecondaryKanji(_, _, described) => described.freq,
        }
    }

    /// All spellings in the `Spelling` as a list, in order.
    pub fn words(&self) -> Vec<&str> {
        match self {
            Spelling::Primary(main) => vec![main],
            Spelling::PrimaryKanji(main, described)
            | Spelling::Secondary(main, described) => vec![main, &described.word],
            Spelling::SecondaryKanji(main, main_kanji, described) => vec![main, &main_kanji.word, &described.word],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingType {
    On,
    Kun,
    Combined,
}

impl ReadingType {
    /// All types, in the order they're written in a kanji entry.
    pub const ALL: [ReadingType; 3] = [ReadingType::On, ReadingType::Kun, ReadingType::Combined];
}

/// The kana used to spell a word
#[derive(Debug, Clone)]
pub enum Kana {
    /// A reading with the middle part marked as written by a given kanji.
    Split { pre: String, mid: String, post: String },
    /// A reading without no section dinstinguished.
    Unsplit(String),
}

impl Kana {
    /// The kana joined into a single string.
    pub fn joined(&self) -> String {
        match self {
            Kana::Split { pre, mid, post } => format!("{pre}{mid}{post}"),
            Kana::Unsplit(kana) => kana.clone(),
        }
    }
}

/// The origin of a reading, for more recent loanwords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingOrigin {
    Asian,
    European,
}

/// A specific example of a kanji reading.
#[derive(Debug, Clone)]
pub struct Reading {
    pub line: usize,
    pub core: bool,
    pub spelling: Spelling,
    pub kana: Kana,
    pub definition: String,
    pub origin: Option<ReadingOrigin>,
    pub variants: Vec<FreqWord>,
}

/// A group of related reading examples.
pub type ReadingGroup = Vec<Reading>;

/// One part of a kanji entry. Most kanji only have a single part.
#[derive(Debug, Clone)]
pub struct KanjiPart {
    pub on: Vec<ReadingGroup>,
    pub kun: Vec<ReadingGroup>,
    pub combined: Vec<ReadingGroup>,
}

impl KanjiPart {
    /// All readings combined into a single list.
    pub fn readings(&self) -> impl Iterator<Item = &Reading> {
        self.on.iter().chain(&self.kun).chain(&self.combined).flatten()
    }
}

impl Index<ReadingType> for KanjiPart {
    type Output = Vec<ReadingGroup>;

    /// The groups of readings for a given type.
    fn index(&self, reading_type: ReadingType) -> &Vec<ReadingGroup> {
        match reading_type {
            ReadingType::On => &self.on,
            ReadingType::Kun => &self.kun,
            ReadingType::Combined => &self.combined,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KanjiEntry {
    pub line: usize,
    pub kanji: char,
    pub parts: Vec<KanjiPart>,
}

#[derive(Debug, Clone)]
pub struct WordEntry {
    pub line: usize,
    pub headword: String,
    pub readings: Vec<ReadingGroup>,
}

#[derive(Debug, Clone)]
pub enum Entry {
    Kanji(KanjiEntry),
    Word(WordEntry),
}

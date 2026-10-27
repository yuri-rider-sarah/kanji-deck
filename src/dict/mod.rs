//! The dictionary data model, with `load` to read and check a dictionary
//! from the nodes of its file and `serialize` to write it back as text.

mod check;
mod convert;
mod serialize;

pub use serialize::serialize;

use crate::error::{Error, Result};
use crate::parser::Args;
use std::fmt;
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
    PrimaryKanji { main: String, described: FreqWord },
    Secondary { main: String, described: FreqWord },
    SecondaryKanji { main: String, main_kanji: FreqWord, described: FreqWord },
}

impl Spelling {
    /// The most common spelling of the word.
    pub fn main(&self) -> &str {
        match self {
            Spelling::Primary(main)
            | Spelling::PrimaryKanji { main, .. }
            | Spelling::Secondary { main, .. }
            | Spelling::SecondaryKanji { main, .. } => main,
        }
    }

    /// The spelling of the word described in the entry.
    pub fn described(&self) -> &str {
        match self {
            Spelling::Primary(described) => described,
            Spelling::PrimaryKanji { described, .. }
            | Spelling::Secondary { described, .. }
            | Spelling::SecondaryKanji { described, .. } => &described.word,
        }
    }

    /// The frequency of the described spelling.
    pub fn described_freq(&self) -> Freq {
        match self {
            Spelling::Primary(_) => Freq::Common,
            Spelling::PrimaryKanji { described, .. }
            | Spelling::Secondary { described, .. }
            | Spelling::SecondaryKanji { described, .. } => described.freq,
        }
    }

    /// All spellings in the `Spelling` as a list, in order.
    pub fn words(&self) -> Vec<&str> {
        match self {
            Spelling::Primary(main) => vec![main],
            Spelling::PrimaryKanji { main, described }
            | Spelling::Secondary { main, described } => {
                vec![main, &described.word]
            }
            Spelling::SecondaryKanji { main, main_kanji, described } => {
                vec![main, &main_kanji.word, &described.word]
            }
        }
    }
}

/// The kind of a reading, which decides the group it's in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingType {
    On,
    Kun,
    Asian,
    European,
}

impl ReadingType {
    /// All types, in the order they're written in an entry.
    pub const ALL: [ReadingType; 4] =
        [ReadingType::On, ReadingType::Kun, ReadingType::Asian, ReadingType::European];
}

impl fmt::Display for ReadingType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReadingType::On => write!(f, "on"),
            ReadingType::Kun => write!(f, "kun"),
            ReadingType::Asian => write!(f, "asian"),
            ReadingType::European => write!(f, "european"),
        }
    }
}

/// The kana used to spell the example word of a reading.
pub trait ReadingKana {
    /// The kana as a single string.
    fn joined(&self) -> String;
}

/// The kana of a reading in a kanji entry, with the middle part marked as
/// written by the kanji.
#[derive(Debug, Clone)]
pub struct SplitKana {
    pub pre: String,
    pub mid: String,
    pub post: String,
}

impl ReadingKana for SplitKana {
    fn joined(&self) -> String {
        format!("{}{}{}", self.pre, self.mid, self.post)
    }
}

/// The kana of a reading in a word entry, which all belongs to the word.
#[derive(Debug, Clone)]
pub struct UnsplitKana(pub String);

impl ReadingKana for UnsplitKana {
    fn joined(&self) -> String {
        self.0.clone()
    }
}

/// The spelling and kana that decomposition uses for a reading instead of its example,
/// for examples where the provided word would give the wrong reading.
#[derive(Debug, Clone)]
pub struct Canonical<K> {
    pub spelling: String,
    pub kana: K,
}

/// A less common spelling of the example word of a reading.
#[derive(Debug, Clone)]
pub struct Variant {
    pub spelling: FreqWord,
    pub canonical: Option<String>,
}

impl Variant {
    /// The spelling that decomposition uses: the canonical spelling if given,
    /// otherwise the variant's own.
    pub fn canonical_spelling(&self) -> &str {
        match &self.canonical {
            Some(canonical) => canonical,
            None => &self.spelling.word,
        }
    }
}

/// The part of speech of a reading's example word, overriding the one extracted from another source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartOfSpeech {
    IchidanVerb,
    GodanVerb,
    Adjective,
    Other,
    Noun,
}

/// A specific example of a kanji or word reading, with the kana `K` dependent
/// on the kind of entry it's in.
#[derive(Debug, Clone)]
pub struct Reading<K> {
    pub line: usize,
    pub core: bool,
    pub regular: bool,
    pub spelling: Spelling,
    pub kana: K,
    pub definition: String,
    pub canonical: Option<Canonical<K>>,
    pub variants: Vec<Variant>,
    pub part_of_speech: Option<PartOfSpeech>,
}

impl<K> Reading<K> {
    /// The spelling that decomposition uses: the canonical spelling if given,
    /// otherwise the example spelling.
    pub fn canonical_spelling(&self) -> &str {
        match &self.canonical {
            Some(canonical) => &canonical.spelling,
            None => self.spelling.described(),
        }
    }

    /// The kana that decomposition uses: the canonical kana if given,
    /// otherwise the reading's own.
    pub fn canonical_kana(&self) -> &K {
        match &self.canonical {
            Some(canonical) => &canonical.kana,
            None => &self.kana,
        }
    }
}

/// A group of related reading examples.
pub type ReadingGroup<K> = Vec<Reading<K>>;

/// The readings of a word entry, or one part of a kanji entry. Most kanji
/// entries only have a single part.
#[derive(Debug, Clone)]
pub struct EntryPart<K> {
    pub on: Vec<ReadingGroup<K>>,
    pub kun: Vec<ReadingGroup<K>>,
    pub asian: Vec<ReadingGroup<K>>,
    pub european: Vec<ReadingGroup<K>>,
}

impl<K> EntryPart<K> {
    /// All readings in a single list, in order.
    pub fn readings(&self) -> impl Iterator<Item = &Reading<K>> {
        ReadingType::ALL.into_iter().flat_map(|reading_type| &self[reading_type]).flatten()
    }
}

impl<K> Index<ReadingType> for EntryPart<K> {
    type Output = Vec<ReadingGroup<K>>;

    /// The groups of readings for a given type.
    fn index(&self, reading_type: ReadingType) -> &Vec<ReadingGroup<K>> {
        match reading_type {
            ReadingType::On => &self.on,
            ReadingType::Kun => &self.kun,
            ReadingType::Asian => &self.asian,
            ReadingType::European => &self.european,
        }
    }
}

/// The entry of a kanji, split into one or more parts, each with its own card.
#[derive(Debug, Clone)]
pub struct KanjiEntry {
    pub line: usize,
    pub kanji: char,
    pub parts: Vec<EntryPart<SplitKana>>,
}

/// The entry of a word taught outside a specific kanji, on a single card.
#[derive(Debug, Clone)]
pub struct WordEntry {
    pub line: usize,
    pub headword: String,
    pub introduced_kanji: Vec<char>,
    pub part: EntryPart<UnsplitKana>,
}

/// An entry of the dictionary.
#[derive(Debug, Clone)]
pub enum Entry {
    Kanji(KanjiEntry),
    Word(WordEntry),
}

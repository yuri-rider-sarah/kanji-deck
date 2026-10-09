//! The dictionary data model. These are plain data types: `parse` only checks
//! the structure of the file, and `check` validates the contents.

pub mod check;
pub mod parse;
pub mod read;

use std::fmt;
use unicode_script::{Script, UnicodeScript};

#[derive(Debug, Clone)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

pub fn is_han(c: char) -> bool {
    c.script() == Script::Han
}

pub fn is_hiragana(c: char) -> bool {
    c.script() == Script::Hiragana
}

pub fn is_katakana(c: char) -> bool {
    c.script() == Script::Katakana || c == 'ー'
}

pub fn has_han(word: &str) -> bool {
    word.chars().any(is_han)
}

/// Ordered so that `Rare < Common`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Freq {
    Rare,
    Common,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreqWord {
    pub freq: Freq,
    pub word: String,
}

/// The spellings of a word. `main` is the most common spelling, `kanji` the
/// most common one using kanji if that's different, and `described` the one
/// given as an example of the reading if it's neither of these.
#[derive(Debug, Clone)]
pub enum Spelling {
    Primary(String),
    PrimaryKanji { main: String, kanji: FreqWord },
    Secondary { main: String, described: FreqWord },
    SecondaryKanji { main: String, kanji: FreqWord, described: FreqWord },
}

impl Spelling {
    pub fn main(&self) -> &str {
        match self {
            Spelling::Primary(main)
            | Spelling::PrimaryKanji { main, .. }
            | Spelling::Secondary { main, .. }
            | Spelling::SecondaryKanji { main, .. } => main,
        }
    }

    /// The spelling given as an example of the reading.
    pub fn described(&self) -> &str {
        match self {
            Spelling::Primary(main) => main,
            Spelling::PrimaryKanji { kanji, .. } => &kanji.word,
            Spelling::Secondary { described, .. } | Spelling::SecondaryKanji { described, .. } => &described.word,
        }
    }

    pub fn words(&self) -> Vec<&str> {
        match self {
            Spelling::Primary(main) => vec![main],
            Spelling::PrimaryKanji { main, kanji } => vec![main, &kanji.word],
            Spelling::Secondary { main, described } => vec![main, &described.word],
            Spelling::SecondaryKanji { main, kanji, described } => vec![main, &kanji.word, &described.word],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    On,
    Kun,
    Combined,
    Word,
}

/// The reading of a word. On and kun readings mark which part of it belongs
/// to the kanji.
#[derive(Debug, Clone)]
pub enum Kana {
    Split { pre: String, mid: String, post: String },
    Unsplit(String),
}

impl Kana {
    pub fn joined(&self) -> String {
        match self {
            Kana::Split { pre, mid, post } => format!("{pre}{mid}{post}"),
            Kana::Unsplit(kana) => kana.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attr {
    Asian,
    European,
}

#[derive(Debug, Clone)]
pub struct Reading {
    pub line: usize,
    pub primary: bool,
    pub spelling: Spelling,
    pub kana: Kana,
    pub definition: String,
    pub attrs: Vec<Attr>,
    pub variants: Vec<FreqWord>,
}

impl Reading {
    /// All spellings including variants, without repetitions.
    pub fn words(&self) -> Vec<&str> {
        let mut words = Vec::new();
        for word in self.spelling.words().into_iter().chain(self.variants.iter().map(|v| v.word.as_str())) {
            if !words.contains(&word) {
                words.push(word);
            }
        }
        words
    }
}

/// Readings shown together: related readings, or examples of the same reading.
pub type Group = Vec<Reading>;

#[derive(Debug, Clone)]
pub struct KanjiPart {
    /// Empty if the kanji isn't split into parts.
    pub name: String,
    pub on: Vec<Group>,
    pub kun: Vec<Group>,
    pub combined: Vec<Group>,
}

impl KanjiPart {
    pub fn sections(&self) -> [(Section, &[Group]); 3] {
        [(Section::On, &self.on), (Section::Kun, &self.kun), (Section::Combined, &self.combined)]
    }

    pub fn readings(&self) -> impl Iterator<Item = &Reading> {
        self.on.iter().chain(&self.kun).chain(&self.combined).flatten()
    }
}

#[derive(Debug, Clone)]
pub struct KanjiEntry {
    pub line: usize,
    pub kanji: String,
    pub parts: Vec<KanjiPart>,
}

#[derive(Debug, Clone)]
pub struct WordEntry {
    pub line: usize,
    pub readings: Vec<Group>,
}

impl WordEntry {
    /// The word the entry is about: the example spelling of its first reading.
    pub fn headword(&self) -> &str {
        self.readings[0][0].spelling.described()
    }
}

#[derive(Debug, Clone)]
pub enum Entry {
    Kanji(KanjiEntry),
    Word(WordEntry),
}

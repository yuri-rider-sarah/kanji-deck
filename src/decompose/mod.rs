//! Decomposition of words into the readings of the dictionary.

mod extract;

pub use extract::extract_readings;

/// A word to decompose: a spelling and its kana, converted to hiragana.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Word {
    pub spelling: String,
    pub kana: String,
}

/// A part of speech whose okurigana decomposition handles differently from the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartOfSpeech {
    IchidanVerb,
    GodanVerb,
    Adjective,
}

/// A reading used in word decomposition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reading {
    /// What the reading is written with: a kanji for readings of kanji entries, and the
    /// spelling of the word for word entries, without the okurigana of kun readings.
    pub spelling: String,
    /// The kana of the reading in hiragana, without the okurigana of kun readings.
    pub kana: String,
    pub reading_type: ReadingType,
}

/// The type of a reading, which determines where in a word it may occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadingType {
    /// An on reading, including recent Asian borrowings.
    On,
    /// A kun reading without okurigana.
    BareKun,
    /// The stem of an i-adjective whose okurigana is just い.
    KunAdjective,
    /// A kun reading followed by okurigana that starts in the given way.
    KunWithOkurigana(OkuriganaStart),
    /// A borrowing from a European language.
    Foreign,
}

/// How the okurigana after a kun reading starts: mostly the consonant of its first kana,
/// which is the kana's column in the kana table, with dakuten and handakuten giving rows of their own.
/// あ, い, え, お, っ, and ん have values of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OkuriganaStart {
    K,
    G,
    S,
    Z,
    T,
    D,
    N,
    H,
    B,
    P,
    M,
    Y,
    R,
    W,
    A,
    I,
    E,
    O,
    /// っ.
    SmallTsu,
    /// ん.
    MoraicN,
}

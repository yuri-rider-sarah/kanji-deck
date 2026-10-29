//! Prints the words from UniDic's lexicon that can't be decomposed into the readings of the
//! dictionary, one `spelling kana` line each. Words without kanji, or with kanji or other
//! characters that the readings can't cover, are left out.

use kanji::decompose;
use kanji::dict::Entry;
use kanji::file_error::{FileAction, FileError, report_errors};
use kanji::japanese::{is_japanese, is_kanji};
use kanji::{dict, parser, unidic};
use std::collections::HashSet;
use std::process::ExitCode;

/// The dictionary, relative to the repo root.
const DICTIONARY_PATH: &str = "dictionary";

fn main() -> ExitCode {
    report_errors(run())
}

/// Reads the dictionary and the lexicon, and prints the words that can't be decomposed.
fn run() -> Result<(), Vec<FileError>> {
    const LEXICON_PATH: &str = "data/unidic/lex.csv";
    let text = std::fs::read_to_string(DICTIONARY_PATH)
        .map_err(|error| FileError::io(DICTIONARY_PATH, FileAction::Read, error))?;
    let nodes =
        parser::parse(&text).map_err(|error| FileError::contents(DICTIONARY_PATH, error))?;
    let entries =
        dict::load(nodes).map_err(|errors| FileError::contents_errors(DICTIONARY_PATH, errors))?;
    let lexicon = std::fs::read_to_string(LEXICON_PATH)
        .map_err(|error| FileError::io(LEXICON_PATH, FileAction::Read, error))?;
    let (words, parts_of_speech) =
        unidic::read_lexicon(&lexicon).map_err(|error| FileError::contents(LEXICON_PATH, error))?;
    let readings = decompose::extract_readings(&entries, &parts_of_speech)
        .map_err(|errors| FileError::contents_errors(DICTIONARY_PATH, errors))?;
    let known_kanji: HashSet<char> = entries
        .iter()
        .flat_map(|entry| match entry {
            Entry::Kanji(entry) => std::slice::from_ref(&entry.kanji),
            Entry::Word(entry) => entry.introduced_kanji.as_slice(),
        })
        .copied()
        .collect();
    let reading_index = decompose::index_readings(&readings);
    for word in &words {
        if is_word_to_decompose(&word.spelling, &known_kanji)
            && !decompose::can_decompose_word(word, &reading_index)
        {
            println!("{} {}", word.spelling, word.kana);
        }
    }
    Ok(())
}

/// Whether a word is one to decompose: it has a kanji, all its kanji are introduced in the
/// dictionary, and it has no characters other than kanji, kana, ヶ, ヵ and 々 after a kanji.
fn is_word_to_decompose(spelling: &str, known_kanji: &HashSet<char>) -> bool {
    let mut has_kanji = false;
    let mut previous_c = None;
    for c in spelling.chars() {
        if is_kanji(c) {
            if !known_kanji.contains(&c) {
                return false;
            }
            has_kanji = true;
        } else if c == '々' {
            if !previous_c.is_some_and(is_kanji) {
                return false;
            }
        } else if !is_japanese(c) {
            return false;
        }
        previous_c = Some(c);
    }
    has_kanji
}

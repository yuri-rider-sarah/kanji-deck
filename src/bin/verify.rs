//! Prints the readings extracted from the dictionary for decomposition, one
//! `spelling<TAB>kana<TAB>type` line each, with the parts of speech from UniDic's lexicon.
//!
//! This will become the check of which words the dictionary's readings don't cover.

use kanji::decompose::{self, OkuriganaStart, ReadingType};
use kanji::file_error::{FileAction, FileError, report_errors};
use kanji::{dict, parser, unidic};
use std::process::ExitCode;

/// The dictionary, relative to the repo root.
const DICTIONARY_PATH: &str = "dictionary";

fn main() -> ExitCode {
    report_errors(run())
}

/// Reads the dictionary and the lexicon, and prints the readings extracted from the dictionary.
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
    let (_words, parts_of_speech) =
        unidic::read_lexicon(&lexicon).map_err(|error| FileError::contents(LEXICON_PATH, error))?;
    let readings = decompose::extract_readings(&entries, &parts_of_speech)
        .map_err(|errors| FileError::contents_errors(DICTIONARY_PATH, errors))?;
    for reading in &readings {
        println!(
            "{}\t{}\t{}",
            reading.spelling,
            reading.kana,
            description_from_reading_type(reading.reading_type)
        );
    }
    Ok(())
}

/// The type of a reading as shown in the list, with how its okurigana starts
/// for kun readings with okurigana (`kun .k`).
fn description_from_reading_type(reading_type: ReadingType) -> String {
    match reading_type {
        ReadingType::On => "on".to_string(),
        ReadingType::BareKun => "kun".to_string(),
        ReadingType::KunAdjective => "kun adjective".to_string(),
        ReadingType::KunWithOkurigana(okurigana_start) => {
            format!("kun .{}", name_from_okurigana_start(okurigana_start))
        }
        ReadingType::Foreign => "foreign".to_string(),
    }
}

/// How okurigana starts, as a consonant in romaji, or the kana itself where it has no
/// consonant of its own.
fn name_from_okurigana_start(okurigana_start: OkuriganaStart) -> &'static str {
    match okurigana_start {
        OkuriganaStart::K => "k",
        OkuriganaStart::S => "s",
        OkuriganaStart::T => "t",
        OkuriganaStart::N => "n",
        OkuriganaStart::H => "h",
        OkuriganaStart::M => "m",
        OkuriganaStart::Y => "y",
        OkuriganaStart::R => "r",
        OkuriganaStart::W => "w",
        OkuriganaStart::G => "g",
        OkuriganaStart::Z => "z",
        OkuriganaStart::D => "d",
        OkuriganaStart::B => "b",
        OkuriganaStart::P => "p",
        OkuriganaStart::A => "あ",
        OkuriganaStart::I => "い",
        OkuriganaStart::E => "え",
        OkuriganaStart::O => "お",
        OkuriganaStart::SmallTsu => "っ",
        OkuriganaStart::MoraicN => "ん",
    }
}

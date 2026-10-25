//! Prints the words to decompose from UniDic's lexicon, one `spelling<TAB>kana` pair per line.
//!
//! This will become the check of which words the dictionary's readings don't cover.

use kanji::file_error::{FileAction, FileError, report_errors};
use kanji::unidic;
use std::process::ExitCode;

fn main() -> ExitCode {
    report_errors(run())
}

/// Reads the lexicon and prints its words.
fn run() -> Result<(), Vec<FileError>> {
    const LEXICON_PATH: &str = "data/unidic/lex.csv";
    let text = std::fs::read_to_string(LEXICON_PATH)
        .map_err(|error| FileError::io(LEXICON_PATH, FileAction::Read, error))?;
    let (words, _parts_of_speech) =
        unidic::read_lexicon(&text).map_err(|error| FileError::contents(LEXICON_PATH, error))?;
    for word in &words {
        println!("{}\t{}", word.spelling, word.kana);
    }
    Ok(())
}

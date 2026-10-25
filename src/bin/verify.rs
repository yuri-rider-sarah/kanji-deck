//! Prints the words to decompose from UniDic's lexicon, one `spelling<TAB>kana` pair per line.
//!
//! This will become the check of which words the dictionary's readings don't cover.

use kanji::unidic;
use std::process::ExitCode;

fn main() -> ExitCode {
    const LEXICON_PATH: &str = "data/unidic/lex.csv";
    let text = match std::fs::read_to_string(LEXICON_PATH) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Can't read {LEXICON_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (words, _parts_of_speech) = match unidic::read_lexicon(&text) {
        Ok(lexicon) => lexicon,
        Err(error) => {
            eprintln!("Error: {error}");
            return ExitCode::FAILURE;
        }
    };
    for word in &words {
        println!("{}\t{}", word.spelling, word.kana);
    }
    ExitCode::SUCCESS
}

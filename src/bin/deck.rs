//! Generates the Anki deck from the dictionary, written to `target/kanji.apkg`.
//!
//! Also checks that serializing the parsed dictionary gives back the same file.
//! If it doesn't, the serialized version is saved for comparison.

use kanji::{deck, dict, parser};
use std::io::ErrorKind;
use std::process::ExitCode;
use std::time::SystemTime;

fn main() -> ExitCode {
    let text = match std::fs::read_to_string("dictionary") {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Can't read dictionary: {e}");
            return ExitCode::FAILURE;
        }
    };
    let entries = parser::parse(&text).map_err(|e| vec![e]).and_then(dict::load);
    let errors = match entries {
        Ok(entries) => {
            check_serializer(&text, &entries);
            for warning in deck::check(&entries) {
                eprintln!("Warning: {warning}");
            }
            return write_package(&entries);
        }
        Err(errors) => errors,
    };
    for error in &errors {
        eprintln!("Error: {error}");
    }
    eprintln!("{} errors, deck not generated", errors.len());
    ExitCode::FAILURE
}

/// Warns if the serialized entries differ from the original text,
/// saving them to `target/dictionary_serialized`.
/// Otherwise removes that file if it's left over from a previous run.
fn check_serializer(text: &str, entries: &[dict::Entry]) {
    const PATH: &str = "target/dictionary_serialized";
    let serialized = dict::serialize(entries);
    if serialized != text {
        eprintln!(
            "Warning: the serialized dictionary doesn't match the original, saved it to {PATH}"
        );
        if let Err(e) = std::fs::write(PATH, serialized) {
            eprintln!("Can't write {PATH}: {e}");
        }
    } else if let Err(e) = std::fs::remove_file(PATH)
        && e.kind() != ErrorKind::NotFound
    {
        eprintln!("Can't remove {PATH}: {e}");
    }
}

/// Writes the entries as an Anki package to `target/kanji.apkg`, with the styling
/// and the font from `assets`.
fn write_package(entries: &[dict::Entry]) -> ExitCode {
    const CSS_PATH: &str = "assets/style.css";
    const FONT_PATH: &str = "assets/_NotoSansJP.ttf";
    const PACKAGE_PATH: &str = "target/kanji.apkg";
    let css = match std::fs::read_to_string(CSS_PATH) {
        Ok(css) => css,
        Err(e) => {
            eprintln!("Can't read {CSS_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let font = match std::fs::read(FONT_PATH) {
        Ok(font) => font,
        Err(e) => {
            eprintln!("Can't read {FONT_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let package = deck::apkg_from_entries(entries, &css, &font, SystemTime::now());
    if let Err(e) = std::fs::write(PACKAGE_PATH, package) {
        eprintln!("Can't write {PACKAGE_PATH}: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

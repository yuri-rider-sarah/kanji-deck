//! Generates the Anki deck from the dictionary, written to `target/kanji.apkg`.
//!
//! Also checks that serializing the parsed dictionary gives back the same file.
//! If it doesn't, the serialized version is saved for comparison.

use kanji::file_error::{FileAction, FileError, report_errors, report_warning};
use kanji::{deck, dict, parser};
use std::io::ErrorKind;
use std::process::ExitCode;
use std::time::SystemTime;

/// The dictionary, relative to the repo root.
const DICTIONARY_PATH: &str = "dictionary";

fn main() -> ExitCode {
    report_errors(run())
}

/// Reads and checks the dictionary, warns about problems that don't stop generation,
/// and writes the package.
fn run() -> Result<(), Vec<FileError>> {
    let text = std::fs::read_to_string(DICTIONARY_PATH)
        .map_err(|error| FileError::io(DICTIONARY_PATH, FileAction::Read, error))?;
    let nodes =
        parser::parse(&text).map_err(|error| FileError::contents(DICTIONARY_PATH, error))?;
    let entries =
        dict::load(nodes).map_err(|errors| FileError::contents_errors(DICTIONARY_PATH, errors))?;
    check_serializer(&text, &entries);
    for warning in deck::check(&entries) {
        report_warning(&FileError::contents(DICTIONARY_PATH, warning));
    }
    write_package(&entries)?;
    Ok(())
}

/// Warns if the serialized entries differ from the original text,
/// saving them to `target/dictionary_serialized`.
/// Otherwise removes that file if it's left over from a previous run.
fn check_serializer(text: &str, entries: &[dict::Entry]) {
    const PATH: &str = "target/dictionary_serialized";
    let serialized = dict::serialize(entries);
    if serialized != text {
        report_warning(&FileError::whole_file(
            DICTIONARY_PATH,
            format!("serializing doesn't give back the same text, saved the result to {PATH}"),
        ));
        if let Err(error) = std::fs::write(PATH, serialized) {
            report_warning(&FileError::io(PATH, FileAction::Write, error));
        }
    } else if let Err(error) = std::fs::remove_file(PATH)
        && error.kind() != ErrorKind::NotFound
    {
        report_warning(&FileError::io(PATH, FileAction::Remove, error));
    }
}

/// Writes the entries as an Anki package to `target/kanji.apkg`, with the styling
/// and the font from `assets`.
fn write_package(entries: &[dict::Entry]) -> Result<(), FileError> {
    const CSS_PATH: &str = "assets/style.css";
    const FONT_PATH: &str = "assets/_NotoSansJP.ttf";
    const PACKAGE_PATH: &str = "target/kanji.apkg";
    let css = std::fs::read_to_string(CSS_PATH)
        .map_err(|error| FileError::io(CSS_PATH, FileAction::Read, error))?;
    let font = std::fs::read(FONT_PATH)
        .map_err(|error| FileError::io(FONT_PATH, FileAction::Read, error))?;
    let package = deck::apkg_from_entries(entries, &css, &font, SystemTime::now());
    std::fs::write(PACKAGE_PATH, package)
        .map_err(|error| FileError::io(PACKAGE_PATH, FileAction::Write, error))
}

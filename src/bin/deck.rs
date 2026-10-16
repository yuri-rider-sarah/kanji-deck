//! Generates the Anki deck from the dictionary. With `--json`, prints the
//! card data instead of passing it to generate.py.
//!
//! Also checks that serializing the parsed dictionary gives back the same
//! file. If it doesn't, the serialized version is saved for comparison.

use kanji::{deck, dict, parser};
use std::io::{ErrorKind, Write};
use std::process::{Command, ExitCode, Stdio};

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
            return output(&deck::json_from_entries(&entries));
        }
        Err(errors) => errors,
    };
    for error in &errors {
        eprintln!("Error: {error}");
    }
    eprintln!("{} errors, deck not generated", errors.len());
    ExitCode::FAILURE
}

/// Warns if the serialized entries differ from the original text, saving
/// them to `dictionary_serialized`. Otherwise removes that file if it's left
/// over from a previous run.
fn check_serializer(text: &str, entries: &[dict::Entry]) {
    const PATH: &str = "dictionary_serialized";
    let serialized = dict::serialize(entries);
    if serialized != text {
        eprintln!("Warning: the serialized dictionary doesn't match the original, saved it to {PATH}");
        if let Err(e) = std::fs::write(PATH, serialized) {
            eprintln!("Can't write {PATH}: {e}");
        }
    } else if let Err(e) = std::fs::remove_file(PATH)
        && e.kind() != ErrorKind::NotFound
    {
        eprintln!("Can't remove {PATH}: {e}");
    }
}

fn output(json: &str) -> ExitCode {
    if std::env::args().any(|arg| arg == "--json") {
        print!("{json}");
        return ExitCode::SUCCESS;
    }
    let mut child = Command::new("./generate.py").stdin(Stdio::piped()).spawn().expect("can't run generate.py");
    child.stdin.take().unwrap().write_all(json.as_bytes()).expect("can't write to generate.py");
    match child.wait() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

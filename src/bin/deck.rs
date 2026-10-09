//! Generates the Anki deck from the dictionary. With `--json`, prints the
//! card data instead of passing it to generate.py.

use kanji::{deck, dict};
use std::io::Write;
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    let text = match std::fs::read_to_string("dictionary") {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Can't read dictionary: {e}");
            return ExitCode::FAILURE;
        }
    };
    let errors = match dict::parse::parse(&text) {
        Ok(entries) => {
            let errors = dict::check::check(&entries);
            if errors.is_empty() {
                return output(&deck::json(&entries));
            }
            errors
        }
        Err(errors) => errors,
    };
    for error in &errors {
        eprintln!("{error}");
    }
    eprintln!("{} errors, deck not generated", errors.len());
    ExitCode::FAILURE
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

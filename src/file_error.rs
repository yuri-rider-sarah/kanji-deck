//! Errors and warnings in the files the executables read and write, and reporting them.
//! Other modules report errors only by position, so the executables add the file's path.

use crate::error::Error;
use std::fmt;
use std::process::ExitCode;

/// An error or warning in a file, with the path it was reported for. Paths are fixed by
/// the executables, so they're static.
#[derive(Debug)]
pub enum FileError {
    /// The file couldn't be read, written or removed.
    Io { path: &'static str, action: FileAction, error: std::io::Error },
    /// The file's contents are wrong, at the position a module reported.
    Contents { path: &'static str, error: Error },
    /// A problem with the file as a whole, without a position.
    WholeFile { path: &'static str, message: String },
}

/// What an executable tried to do with a file when it failed.
#[derive(Debug, Clone, Copy)]
pub enum FileAction {
    Read,
    Write,
    Remove,
}

impl FileError {
    /// A failure to access a file.
    pub fn io(path: &'static str, action: FileAction, error: std::io::Error) -> FileError {
        FileError::Io { path, action, error }
    }

    /// An error in the contents of a file.
    pub fn contents(path: &'static str, error: Error) -> FileError {
        FileError::Contents { path, error }
    }

    /// A problem with a file as a whole.
    pub fn whole_file(path: &'static str, message: impl Into<String>) -> FileError {
        FileError::WholeFile { path, message: message.into() }
    }
}

/// A single error as the list of errors that `run` functions return, so `?` works on both.
impl From<FileError> for Vec<FileError> {
    fn from(error: FileError) -> Vec<FileError> {
        vec![error]
    }
}

impl fmt::Display for FileAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileAction::Read => write!(f, "read"),
            FileAction::Write => write!(f, "write"),
            FileAction::Remove => write!(f, "remove"),
        }
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileError::Io { path, action, error } => {
                write!(f, "{path}: failed to {action}: {error}")
            }
            FileError::Contents { path, error } => match error.column {
                Some(column) => write!(f, "{path}:{}:{column}: {}", error.line, error.message),
                None => write!(f, "{path}:{}: {}", error.line, error.message),
            },
            FileError::WholeFile { path, message } => write!(f, "{path}: {message}"),
        }
    }
}

/// Prints the errors of an executable's `run` function and returns the exit code for the result.
#[must_use]
pub fn report_errors(result: Result<(), Vec<FileError>>) -> ExitCode {
    let errors = match result {
        Ok(()) => return ExitCode::SUCCESS,
        Err(errors) => errors,
    };
    for error in &errors {
        eprintln!("Error: {error}");
    }
    ExitCode::FAILURE
}

/// Prints a problem that doesn't stop the executable.
pub fn report_warning(warning: &FileError) {
    eprintln!("Warning: {warning}");
}

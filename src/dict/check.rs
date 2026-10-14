//! Validation of the dictionary contents.

use super::*;
use crate::japanese::*;
use std::collections::HashSet;

pub(super) fn check(entries: &[Entry]) -> Vec<Error> {
    let mut errors = Vec::new();
    // Kanji introduced so far.
    let mut known_kanji = HashSet::new();
    // Word entries introduced so far.
    let mut known_words = HashSet::new();
    for entry in entries {
        match entry {
            Entry::Kanji(entry) => check_kanji(entry, &mut known_kanji, &mut errors),
            Entry::Word(entry) => check_word(entry, &known_kanji, &mut known_words, &mut errors),
        }
    }
    errors
}

fn check_kanji(entry: &KanjiEntry, known_kanji: &mut HashSet<char>, errors: &mut Vec<Error>) {
    let kanji = entry.kanji;
    if !known_kanji.insert(kanji) {
        errors.push(Error::at_line(entry.line, format!("duplicate entry for kanji {kanji}")));
    }
    if !is_kanji(kanji) {
        errors.push(Error::at_line(entry.line, format!("{kanji} is not a kanji")));
    }
    if !entry.parts.iter().flat_map(KanjiPart::readings).any(|r| r.core) {
        errors.push(Error::at_line(entry.line, format!("{kanji} has no core reading")));
    }
    for part in &entry.parts {
        if part.readings().next().is_none() {
            errors.push(Error::at_line(entry.line, format!("{kanji} has a part without readings")));
            continue;
        }
        for reading_type in ReadingType::ALL {
            check_groups(&part[reading_type], entry.line, errors);
            for reading in part[reading_type].iter().flatten() {
                check_reading(reading, Some(reading_type), Some(kanji), errors);
            }
        }
    }
}

fn check_word<'a>(entry: &'a WordEntry, known_kanji: &HashSet<char>, known_words: &mut HashSet<&'a str>, errors: &mut Vec<Error>) {
    match entry.headword.chars().count() {
        0 => errors.push(Error::at_line(entry.line, "word with empty headword")),
        1 => errors.push(Error::at_line(entry.line, format!("word {} has headword with only one character", entry.headword))),
        _ => (),
    }
    if !known_words.insert(&entry.headword) {
        errors.push(Error::at_line(entry.line, format!("duplicate entry for word {}", entry.headword)));
    }
    let Some(first) = entry.readings.iter().flatten().next() else {
        errors.push(Error::at_line(entry.line, "word without readings"));
        return;
    };
    if !first.core {
        errors.push(Error::at_line(entry.line, "word entry doesn't start with a core reading"));
    }
    if entry.headword != first.spelling.described() {
        errors.push(Error::at_line(entry.line,
            format!("word {} doesn't match its first example {}", entry.headword, first.spelling.described()),
        ));
    }
    check_groups(&entry.readings, entry.line, errors);
    for reading in entry.readings.iter().flatten() {
        check_reading(reading, None, None, errors);
        for word in reading.words() {
            for c in word.chars().filter(|&c| is_kanji(c) && !known_kanji.contains(&c)) {
                errors.push(Error::at_line(reading.line, format!("{word} contains {c}, which hasn't been introduced yet")));
            }
        }
    }
}

/// Checks that none of the groups is empty. Groups have no line, so errors
/// are reported at the entry's.
fn check_groups(groups: &[ReadingGroup], line: usize, errors: &mut Vec<Error>) {
    if groups.iter().any(Vec::is_empty) {
        errors.push(Error::at_line(line, "empty group"));
    }
}

/// Checks a reading of `kanji`. Both the reading type and the kanji are `None`
/// for word entries.
fn check_reading(reading: &Reading, reading_type: Option<ReadingType>, kanji: Option<char>, errors: &mut Vec<Error>) {
    let line = reading.line;
    if reading.definition.trim().is_empty() {
        errors.push(Error::at_line(line, "empty definition"));
    }
    if !kana_fits(&reading.kana, reading_type) {
        let readings = match reading_type {
            Some(reading_type) => format!("{reading_type:?} readings"),
            None => "word readings".to_string(),
        };
        errors.push(Error::at_line(line, format!("{} is not valid kana for {readings}", reading.kana.joined())));
    }
    for word in reading.words() {
        if word.is_empty() {
            errors.push(Error::at_line(line, "empty word"));
        } else if !word.chars().all(is_japanese) {
            errors.push(Error::at_line(line, format!("{word} is not a valid word")));
        }
    }
    check_spelling(&reading.spelling, line, errors);
    // Combined readings are vestigial, and are often for words with kanji
    // that are only introduced later.
    if reading_type == Some(ReadingType::Combined) {
        return;
    }
    if let Some(kanji) = kanji {
        let examples = std::iter::once(reading.spelling.described()).chain(reading.variants.iter().map(|v| v.word.as_str()));
        for word in examples {
            if !word.contains(kanji) {
                errors.push(Error::at_line(line, format!("{word} doesn't contain {kanji}")));
            }
        }
    }
    let kana = katakana_to_hiragana(&reading.kana.joined());
    for word in reading.words() {
        if !hiragana_matches(&katakana_to_hiragana(word), &kana) {
            errors.push(Error::at_line(line, format!("{word} doesn't match its reading {}", reading.kana.joined())));
        }
    }
}

fn check_spelling(spelling: &Spelling, line: usize, errors: &mut Vec<Error>) {
    let words = spelling.words();
    if (1..words.len()).any(|i| words[..i].contains(&words[i])) {
        errors.push(Error::at_line(line, "spelling contains a repeated word"));
    }
    if !has_kanji(spelling.described()) {
        errors.push(Error::at_line(line, format!("example spelling {} contains no kanji", spelling.described())));
    }
    if let Spelling::SecondaryKanji(_, main_kanji, described) = spelling {
        if !has_kanji(&main_kanji.word) {
            errors.push(Error::at_line(line, format!("kanji spelling {} contains no kanji", main_kanji.word)));
        }
        if described.freq > main_kanji.freq {
            errors.push(Error::at_line(line, "described spelling is marked as more common than the main kanji spelling"));
        }
    }
}

/// Whether the kana has the right form for the reading type, which is `None`
/// for word entries.
fn kana_fits(kana: &Kana, reading_type: Option<ReadingType>) -> bool {
    match (kana, reading_type) {
        (Kana::Split { pre, mid, post }, Some(ReadingType::On)) => {
            !mid.is_empty() && [pre, mid, post].iter().all(|s| s.chars().all(is_katakana))
        }
        (Kana::Split { pre, mid, post }, Some(ReadingType::Kun)) => {
            !mid.is_empty() && [pre, mid, post].iter().all(|s| s.chars().all(is_hiragana))
        }
        (Kana::Unsplit(kana), Some(ReadingType::Combined) | None) => {
            !kana.is_empty() && (kana.chars().all(is_hiragana) || kana.chars().all(is_katakana))
        }
        _ => false,
    }
}

/// Whether the kana at the start and end of a word matches its reading. Both
/// should have their katakana converted to hiragana.
fn hiragana_matches(word: &str, kana: &str) -> bool {
    if word.chars().all(is_hiragana) {
        return word == kana;
    }
    let pre: String = word.chars().take_while(|&c| is_hiragana(c)).collect();
    let post: String = word.chars().rev().take_while(|&c| is_hiragana(c)).collect::<Vec<_>>().into_iter().rev().collect();
    kana.len() > pre.len() + post.len() && kana.starts_with(&pre) && kana.ends_with(&post)
}

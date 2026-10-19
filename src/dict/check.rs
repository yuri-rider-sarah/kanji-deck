//! Validation of the dictionary contents.

use super::*;
use crate::japanese::*;
use std::collections::{HashMap, HashSet};

/// How a kanji is introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntroducedBy {
    /// Its own kanji entry.
    KanjiEntry,
    /// One or more word entries, as the kanji has no readings of its own.
    WordEntries,
}

pub fn check(entries: &[Entry]) -> Result<(), Vec<Error>> {
    let mut errors = Vec::new();
    // Every kanji with a kanji entry or introduced by a word entry.
    let mut all_kanji = HashMap::new();
    for entry in entries {
        match &entry.entry_type {
            EntryType::Kanji(kanji) => {
                if all_kanji.insert(*kanji, IntroducedBy::KanjiEntry)
                    == Some(IntroducedBy::KanjiEntry)
                {
                    errors.push(Error::at_line(
                        entry.line,
                        format!("duplicate entry for kanji {kanji}"),
                    ));
                }
            }
            EntryType::Word { introduced_kanji, .. } => {
                for &kanji in introduced_kanji {
                    // A kanji entry takes precedence, so that words introducing it are reported.
                    all_kanji.entry(kanji).or_insert(IntroducedBy::WordEntries);
                }
            }
        }
    }
    // Kanji introduced so far.
    let mut known_kanji = HashSet::new();
    // Word entries introduced so far.
    let mut known_words = HashSet::new();
    for entry in entries {
        match &entry.entry_type {
            EntryType::Kanji(kanji) => {
                known_kanji.insert(*kanji);
                check_kanji(entry, *kanji, &all_kanji, &mut errors);
            }
            EntryType::Word { headword, introduced_kanji } => {
                check_introduced_kanji(entry, headword, introduced_kanji, &all_kanji, &mut errors);
                known_kanji.extend(introduced_kanji);
                check_word(
                    entry,
                    headword,
                    &all_kanji,
                    &known_kanji,
                    &mut known_words,
                    &mut errors,
                );
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Checks the entry of `kanji`. `all_kanji` has every kanji in the dictionary
/// and how it's introduced.
fn check_kanji(
    entry: &Entry,
    kanji: char,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    if !is_kanji(kanji) {
        errors.push(Error::at_line(entry.line, format!("{kanji} is not a kanji")));
    }
    if !entry.parts.iter().flat_map(EntryPart::readings).any(|r| r.core) {
        errors.push(Error::at_line(entry.line, format!("{kanji} has no core reading")));
    }
    for part in &entry.parts {
        check_part(entry, part, all_kanji, errors);
        for reading in part.readings() {
            if reading.regular {
                errors
                    .push(Error::at_line(reading.line, "#regular is only allowed in word entries"));
            }
        }
    }
}

/// Checks the kanji introduced by the word `headword`. A kanji can be
/// introduced by several words, but not if it has a kanji entry.
fn check_introduced_kanji(
    entry: &Entry,
    headword: &str,
    introduced_kanji: &[char],
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    for &kanji in introduced_kanji {
        if !is_kanji(kanji) {
            errors.push(Error::at_line(entry.line, format!("{kanji} is not a kanji")));
        }
        if !headword.contains(kanji) {
            errors.push(Error::at_line(
                entry.line,
                format!("word {headword} introduces {kanji}, which isn't in its headword"),
            ));
        }
        if all_kanji.get(&kanji) == Some(&IntroducedBy::KanjiEntry) {
            errors.push(Error::at_line(
                entry.line,
                format!("word {headword} introduces {kanji}, which has a kanji entry"),
            ));
        }
    }
}

/// Checks the entry of the word `headword`. `all_kanji` has every kanji in the
/// dictionary and how it's introduced, and `known_kanji` are those introduced
/// up to and including this entry.
fn check_word<'a>(
    entry: &Entry,
    headword: &'a str,
    all_kanji: &HashMap<char, IntroducedBy>,
    known_kanji: &HashSet<char>,
    known_words: &mut HashSet<&'a str>,
    errors: &mut Vec<Error>,
) {
    match headword.chars().count() {
        0 => errors.push(Error::at_line(entry.line, "word with empty headword")),
        1 => errors.push(Error::at_line(
            entry.line,
            format!("word {headword} has headword with only one character"),
        )),
        _ => (),
    }
    if !known_words.insert(headword) {
        errors.push(Error::at_line(entry.line, format!("duplicate entry for word {headword}")));
    }
    let Some(first) = entry.parts.iter().flat_map(EntryPart::readings).next() else {
        errors.push(Error::at_line(entry.line, "word without readings"));
        return;
    };
    if !first.core {
        errors.push(Error::at_line(entry.line, "word entry doesn't start with a core reading"));
    }
    if !entry.parts.iter().flat_map(EntryPart::readings).any(|r| r.spelling.described() == headword)
    {
        errors.push(Error::at_line(
            entry.line,
            format!("word {headword} isn't the example spelling of any of its readings"),
        ));
    }
    for part in &entry.parts {
        check_part(entry, part, all_kanji, errors);
        for group in part.on.iter().chain(&part.kun) {
            // Empty groups are reported by `check_part`.
            if let Some(first) = group.first()
                && group.iter().all(|reading| reading.regular)
            {
                errors.push(Error::at_line(first.line, "group with only #regular readings"));
            }
        }
        for reading in part.readings() {
            for word in unique_words(reading) {
                // Kanji that no entry introduces are reported by `check_reading`.
                for c in word
                    .chars()
                    .filter(|&c| all_kanji.contains_key(&c) && !known_kanji.contains(&c))
                {
                    errors.push(Error::at_line(
                        reading.line,
                        format!("{word} contains {c}, which hasn't been introduced yet"),
                    ));
                }
            }
        }
    }
}

/// Checks the groups and readings in a part of an entry.
fn check_part(
    entry: &Entry,
    part: &EntryPart,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    if part.readings().next().is_none() {
        errors.push(Error::at_line(
            entry.line,
            format!("{} has a part without readings", entry.entry_type.key()),
        ));
        return;
    }
    let kanji = match entry.entry_type {
        EntryType::Kanji(kanji) => Some(kanji),
        EntryType::Word { .. } => None,
    };
    for reading_type in ReadingType::ALL {
        check_groups(&part[reading_type], entry.line, errors);
        for reading in part[reading_type].iter().flatten() {
            check_reading(reading, reading_type, kanji, all_kanji, errors);
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

/// Checks a reading of `kanji`, which is `None` for word entries.
fn check_reading(
    reading: &Reading,
    reading_type: ReadingType,
    kanji: Option<char>,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    let line = reading.line;
    if reading.definition.trim().is_empty() {
        errors.push(Error::at_line(line, "empty definition"));
    }
    let (kana_fits, kind_of_readings) = match kanji {
        Some(_) => (kanji_kana_fits(&reading.kana, reading_type), "readings"),
        None => (word_kana_fits(&reading.kana, reading_type, reading.regular), "word readings"),
    };
    if !kana_fits {
        errors.push(Error::at_line(
            line,
            format!(
                "{} is not valid kana for {reading_type} {kind_of_readings}",
                reading.kana.joined()
            ),
        ));
    }
    for word in unique_words(reading) {
        if word.is_empty() {
            errors.push(Error::at_line(line, "empty word"));
        } else if !word.chars().all(is_japanese) {
            errors.push(Error::at_line(line, format!("{word} is not a valid word")));
        }
        for c in word.chars().filter(|&c| is_kanji(c) && !all_kanji.contains_key(&c)) {
            errors.push(Error::at_line(
                line,
                format!("{word} contains {c}, which no entry introduces"),
            ));
        }
    }
    check_spelling(&reading.spelling, line, errors);
    if let Some(kanji) = kanji {
        let examples = std::iter::once(reading.spelling.described())
            .chain(reading.variants.iter().map(|v| v.word.as_str()));
        for word in examples {
            if !word.contains(kanji) {
                errors.push(Error::at_line(line, format!("{word} doesn't contain {kanji}")));
            }
        }
    }
    let kana = katakana_to_hiragana(&reading.kana.joined());
    for word in unique_words(reading) {
        if !hiragana_matches(&katakana_to_hiragana(word), &kana) {
            errors.push(Error::at_line(
                line,
                format!("{word} doesn't match its reading {}", reading.kana.joined()),
            ));
        }
    }
}

fn check_spelling(spelling: &Spelling, line: usize, errors: &mut Vec<Error>) {
    let words = spelling.words();
    if (1..words.len()).any(|i| words[..i].contains(&words[i])) {
        errors.push(Error::at_line(line, "spelling contains a repeated word"));
    }
    if !has_kanji(spelling.described()) {
        errors.push(Error::at_line(
            line,
            format!("example spelling {} contains no kanji", spelling.described()),
        ));
    }
    if let Spelling::SecondaryKanji { main_kanji, described, .. } = spelling {
        if !has_kanji(&main_kanji.word) {
            errors.push(Error::at_line(
                line,
                format!("kanji spelling {} contains no kanji", main_kanji.word),
            ));
        }
        if described.freq > main_kanji.freq {
            errors.push(Error::at_line(
                line,
                "described spelling is marked as more common than the main kanji spelling",
            ));
        }
    }
}

/// Whether the kana has the right form for a reading of the given type in a
/// kanji entry.
fn kanji_kana_fits(kana: &Kana, reading_type: ReadingType) -> bool {
    match (kana, reading_type) {
        (Kana::Split { pre, mid, post }, ReadingType::On) => {
            !mid.is_empty() && [pre, mid, post].iter().all(|s| s.chars().all(is_katakana))
        }
        (Kana::Split { pre, mid, post }, ReadingType::Kun) => {
            !mid.is_empty() && [pre, mid, post].iter().all(|s| s.chars().all(is_hiragana))
        }
        (Kana::Unsplit(_), _) => false,
    }
}

/// Whether the kana has the right form for a reading of the given type in a
/// word entry: unsplit, in katakana for on readings and hiragana for kun
/// readings. Regular readings can be in either script.
fn word_kana_fits(kana: &Kana, reading_type: ReadingType, regular: bool) -> bool {
    let kana = match kana {
        Kana::Split { .. } => return false,
        Kana::Unsplit(kana) => kana,
    };
    let in_katakana = !kana.is_empty() && kana.chars().all(is_katakana);
    let in_hiragana = !kana.is_empty() && kana.chars().all(is_hiragana);
    if regular {
        return in_katakana || in_hiragana;
    }
    match reading_type {
        ReadingType::On => in_katakana,
        ReadingType::Kun => in_hiragana,
    }
}

/// Whether the kana at the start and end of a word matches its reading. Both
/// should have their katakana converted to hiragana.
fn hiragana_matches(word: &str, kana: &str) -> bool {
    if word.chars().all(is_hiragana) {
        return word == kana;
    }
    let pre: String = word.chars().take_while(|&c| is_hiragana(c)).collect();
    let post: String = word
        .chars()
        .rev()
        .take_while(|&c| is_hiragana(c))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    kana.len() > pre.len() + post.len() && kana.starts_with(&pre) && kana.ends_with(&post)
}

/// All spellings in a reading including variants, without repetitions.
fn unique_words(reading: &Reading) -> Vec<&str> {
    let mut words = Vec::new();
    for word in
        reading.spelling.words().into_iter().chain(reading.variants.iter().map(|v| v.word.as_str()))
    {
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

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
        match entry {
            Entry::Kanji(entry) => {
                if all_kanji.insert(entry.kanji, IntroducedBy::KanjiEntry)
                    == Some(IntroducedBy::KanjiEntry)
                {
                    errors.push(Error::at_line(
                        entry.line,
                        format!("duplicate entry for kanji {}", entry.kanji),
                    ));
                }
            }
            Entry::Word(entry) => {
                for &kanji in &entry.introduced_kanji {
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
        match entry {
            Entry::Kanji(entry) => {
                known_kanji.insert(entry.kanji);
                check_kanji(entry, &all_kanji, &mut errors);
            }
            Entry::Word(entry) => {
                check_introduced_kanji(entry, &all_kanji, &mut errors);
                known_kanji.extend(&entry.introduced_kanji);
                check_word(entry, &all_kanji, &known_kanji, &mut known_words, &mut errors);
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Checks a kanji entry. `all_kanji` has every kanji in the dictionary and how
/// it's introduced.
fn check_kanji(
    entry: &KanjiEntry,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    let kanji = entry.kanji;
    if !is_kanji(kanji) {
        errors.push(Error::at_line(entry.line, format!("{kanji} is not a kanji")));
    }
    if !entry.parts.iter().flat_map(EntryPart::readings).any(|r| r.core) {
        errors.push(Error::at_line(entry.line, format!("{kanji} has no core reading")));
    }
    for part in &entry.parts {
        check_part(part, &kanji.to_string(), entry.line, all_kanji, errors);
        for reading in part.readings() {
            if reading.regular {
                errors
                    .push(Error::at_line(reading.line, "#regular is only allowed in word entries"));
            }
            let examples = std::iter::once(reading.spelling.described())
                .chain(reading.variants.iter().map(|v| v.word.as_str()));
            for word in examples {
                if !word.contains(kanji) {
                    errors.push(Error::at_line(
                        reading.line,
                        format!("{word} doesn't contain {kanji}"),
                    ));
                }
            }
        }
    }
}

/// Checks the kanji introduced by a word entry. A kanji can be introduced by
/// several words, but not if it has a kanji entry.
fn check_introduced_kanji(
    entry: &WordEntry,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    let headword = &entry.headword;
    for &kanji in &entry.introduced_kanji {
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

/// Checks a word entry. `all_kanji` has every kanji in the dictionary and how
/// it's introduced, and `known_kanji` are those introduced up to and including
/// this entry.
fn check_word<'a>(
    entry: &'a WordEntry,
    all_kanji: &HashMap<char, IntroducedBy>,
    known_kanji: &HashSet<char>,
    known_words: &mut HashSet<&'a str>,
    errors: &mut Vec<Error>,
) {
    let headword = &entry.headword;
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
        check_part(part, headword, entry.line, all_kanji, errors);
        for reading_type in ReadingType::ALL {
            for group in &part[reading_type] {
                // Empty groups are reported by `check_part`.
                if let Some(first) = group.first()
                    && group.iter().all(|reading| reading.regular)
                {
                    errors.push(Error::at_line(first.line, "group with only #regular readings"));
                }
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

/// Checks the groups and readings in a part of the entry named `name` (the
/// kanji or headword) at `line`.
fn check_part<K: CheckKana>(
    part: &EntryPart<K>,
    name: &str,
    line: usize,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    if part.readings().next().is_none() {
        errors.push(Error::at_line(line, format!("{name} has a part without readings")));
        return;
    }
    for reading_type in ReadingType::ALL {
        check_groups(&part[reading_type], line, errors);
        for reading in part[reading_type].iter().flatten() {
            check_reading(reading, reading_type, all_kanji, errors);
        }
    }
}

/// Checks that none of the groups is empty. Groups have no line, so errors
/// are reported at the entry's.
fn check_groups<K>(groups: &[ReadingGroup<K>], line: usize, errors: &mut Vec<Error>) {
    if groups.iter().any(Vec::is_empty) {
        errors.push(Error::at_line(line, "empty group"));
    }
}

/// Checks a reading of the given type in a kanji or word entry.
fn check_reading<K: CheckKana>(
    reading: &Reading<K>,
    reading_type: ReadingType,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    let line = reading.line;
    if reading.definition.trim().is_empty() {
        errors.push(Error::at_line(line, "empty definition"));
    }
    if !reading.kana.kana_fits(reading_type, reading.regular) {
        errors.push(Error::at_line(
            line,
            format!(
                "{} is not valid kana for {reading_type} {}",
                reading.kana.joined(),
                K::KIND_OF_READINGS
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

/// Kana of a reading, whose form is checked differently for kanji and word
/// entries.
trait CheckKana: ReadingKana {
    /// What readings with this kana are called in errors.
    const KIND_OF_READINGS: &str;

    /// Whether the kana has the right form for a reading of the given type.
    fn kana_fits(&self, reading_type: ReadingType, regular: bool) -> bool;
}

impl CheckKana for SplitKana {
    const KIND_OF_READINGS: &str = "readings";

    /// Whether the kana is in hiragana for kun readings and katakana otherwise,
    /// with a non-empty part for the kanji. Regular readings are reported
    /// separately, as they're not allowed in kanji entries.
    fn kana_fits(&self, reading_type: ReadingType, _regular: bool) -> bool {
        let is_in_script = match reading_type {
            ReadingType::On | ReadingType::Asian | ReadingType::European => is_katakana,
            ReadingType::Kun => is_hiragana,
        };
        !self.mid.is_empty()
            && [&self.pre, &self.mid, &self.post].iter().all(|s| s.chars().all(is_in_script))
    }
}

impl CheckKana for UnsplitKana {
    const KIND_OF_READINGS: &str = "word readings";

    /// Whether the kana is in hiragana for kun readings and katakana otherwise.
    /// Regular readings can be in either script.
    fn kana_fits(&self, reading_type: ReadingType, regular: bool) -> bool {
        let kana = &self.0;
        let in_katakana = !kana.is_empty() && kana.chars().all(is_katakana);
        let in_hiragana = !kana.is_empty() && kana.chars().all(is_hiragana);
        if regular {
            return in_katakana || in_hiragana;
        }
        match reading_type {
            ReadingType::On | ReadingType::Asian | ReadingType::European => in_katakana,
            ReadingType::Kun => in_hiragana,
        }
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
fn unique_words<K>(reading: &Reading<K>) -> Vec<&str> {
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

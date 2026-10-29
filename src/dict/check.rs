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
                // Check that no other kanji entry has the same kanji.
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
    // Check that the entry is for a kanji, not some other character.
    if !is_kanji(kanji) {
        errors.push(Error::at_line(entry.line, format!("{kanji} is not a kanji")));
    }
    // Check that at least one reading in the whole entry is marked as core.
    if !entry.parts.iter().flat_map(EntryPart::readings).any(|r| r.core) {
        errors.push(Error::at_line(entry.line, format!("{kanji} has no core reading")));
    }
    for part in &entry.parts {
        // Check that every part has at least one reading.
        if part.readings().next().is_none() {
            errors.push(Error::at_line(entry.line, format!("{kanji} has a part without readings")));
            continue;
        }
        check_part(part, entry.line, all_kanji, errors);
        for reading in part.readings() {
            // Check that no reading is marked as regular, as that's only for word entries.
            if reading.regular {
                errors
                    .push(Error::at_line(reading.line, "#regular is only allowed in word entries"));
            }
            // Check that the example spelling, all variants and the canonical spellings
            // are written with the kanji. The other spellings may be in kana.
            let examples = std::iter::once(reading.spelling.described())
                .chain(reading.variants.iter().map(|v| v.spelling.word.as_str()))
                .chain(canonical_spellings(reading));
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
        // Check that the introduced character is a kanji.
        if !is_kanji(kanji) {
            errors.push(Error::at_line(entry.line, format!("{kanji} is not a kanji")));
        }
        // Check that the introduced kanji appears in the headword.
        if !headword.contains(kanji) {
            errors.push(Error::at_line(
                entry.line,
                format!("word {headword} introduces {kanji}, which isn't in its headword"),
            ));
        }
        // Check that the introduced kanji doesn't also have its own kanji entry.
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
    // Check that the headword has at least two characters,
    // as single kanji belong in kanji entries.
    match headword.chars().count() {
        0 => errors.push(Error::at_line(entry.line, "word with empty headword")),
        1 => errors.push(Error::at_line(
            entry.line,
            format!("word {headword} has headword with only one character"),
        )),
        _ => (),
    }
    // Check that no earlier word entry has the same headword.
    if !known_words.insert(headword) {
        errors.push(Error::at_line(entry.line, format!("duplicate entry for word {headword}")));
    }
    // Check that the word has at least one reading.
    let Some(first) = entry.part.readings().next() else {
        errors.push(Error::at_line(entry.line, "word without readings"));
        return;
    };
    // Check that the first reading is marked as core.
    if !first.core {
        errors.push(Error::at_line(entry.line, "word entry doesn't start with a core reading"));
    }
    // Check that the headword is the example spelling of one of the readings.
    if !entry.part.readings().any(|r| r.spelling.described() == headword) {
        errors.push(Error::at_line(
            entry.line,
            format!("word {headword} isn't the example spelling of any of its readings"),
        ));
    }
    check_part(&entry.part, entry.line, all_kanji, errors);
    for reading_type in ReadingType::ALL {
        for group in &entry.part[reading_type] {
            // Check that every group has a reading that isn't marked as regular.
            // Empty groups are reported by `check_part`.
            if let Some(first) = group.first()
                && group.iter().all(|reading| reading.regular)
            {
                errors.push(Error::at_line(first.line, "group with only #regular readings"));
            }
        }
    }
    for reading in entry.part.readings() {
        for word in unique_words(reading).into_iter().chain(canonical_spellings(reading)) {
            for c in word.chars() {
                // Check that every kanji in the word has already been introduced.
                // Kanji that no entry introduces are reported by `check_reading`.
                if all_kanji.contains_key(&c) && !known_kanji.contains(&c) {
                    errors.push(Error::at_line(
                        reading.line,
                        format!("{word} contains {c}, which hasn't been introduced yet"),
                    ));
                }
            }
        }
    }
}

/// Checks the groups and readings in a non-empty part of an entry at `line`.
fn check_part<K: CheckKana>(
    part: &EntryPart<K>,
    line: usize,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    for reading_type in ReadingType::ALL {
        // Check that no group is empty.
        // Groups have no line, so the error is reported at the entry's.
        if part[reading_type].iter().any(Vec::is_empty) {
            errors.push(Error::at_line(line, "empty group"));
        }
        for reading in part[reading_type].iter().flatten() {
            check_reading(reading, reading_type, all_kanji, errors);
        }
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
    // Check that the reading has a definition.
    if reading.definition.trim().is_empty() {
        errors.push(Error::at_line(line, "empty definition"));
    }
    check_spelling(&reading.spelling, line, errors);
    check_words_and_kana(
        &unique_words(reading),
        &reading.kana,
        reading_type,
        reading.regular,
        line,
        all_kanji,
        errors,
    );
    // Check that only kun readings have variants, as other readings have no okurigana
    // that variants could differ in.
    if reading_type != ReadingType::Kun && !reading.variants.is_empty() {
        errors.push(Error::at_line(line, "only kun readings can have variants"));
    }
    for variant in &reading.variants {
        // Check that either the reading and all its variants have a canonical spelling,
        // or none of them do.
        match (&reading.canonical, &variant.canonical) {
            (Some(_), None) => errors.push(Error::at_line(
                line,
                format!(
                    "variant {} has no canonical spelling, but its reading has one",
                    variant.spelling.word
                ),
            )),
            (None, Some(_)) => errors.push(Error::at_line(
                line,
                format!(
                    "variant {} has a canonical spelling, but its reading doesn't",
                    variant.spelling.word
                ),
            )),
            (Some(_), Some(_)) | (None, None) => (),
        }
    }
    if let Some(canonical) = &reading.canonical {
        check_written_with_kanji(&canonical.spelling, "canonical", line, errors);
        check_words_and_kana(
            &canonical_spellings(reading),
            &canonical.kana,
            reading_type,
            reading.regular,
            line,
            all_kanji,
            errors,
        );
    }
}

/// Checks the kana of a reading at `line` of the given type, and the words read with it:
/// the reading's own spellings, or its canonical ones with the canonical kana.
fn check_words_and_kana<K: CheckKana>(
    words: &[&str],
    kana: &K,
    reading_type: ReadingType,
    regular: bool,
    line: usize,
    all_kanji: &HashMap<char, IntroducedBy>,
    errors: &mut Vec<Error>,
) {
    let joined_kana = kana.joined();
    // Check that the kana is in the right script for the type of reading.
    if !kana.kana_fits(reading_type, regular) {
        errors.push(Error::at_line(
            line,
            format!("{joined_kana} is not valid kana for {reading_type} {}", K::KIND_OF_READINGS),
        ));
    }
    let kana_in_hiragana = katakana_to_hiragana_in_str(&joined_kana);
    for &word in words {
        // Check that the word isn't empty and is made up only of Japanese characters.
        if word.is_empty() {
            errors.push(Error::at_line(line, "empty word"));
        } else if !word.chars().all(is_japanese) {
            errors.push(Error::at_line(line, format!("{word} is not a valid word")));
        }
        for c in word.chars() {
            // Check that every kanji in the word has an entry or is introduced by a word entry.
            if is_kanji(c) && !all_kanji.contains_key(&c) {
                errors.push(Error::at_line(
                    line,
                    format!("{word} contains {c}, which no entry introduces"),
                ));
            }
        }
        // Check that the kana at the start and end of the word matches the reading.
        if !hiragana_matches(&katakana_to_hiragana_in_str(word), &kana_in_hiragana) {
            errors.push(Error::at_line(
                line,
                format!("{word} doesn't match its reading {joined_kana}"),
            ));
        }
    }
}

/// Checks that a spelling of the named kind (example, kanji, canonical) is written with kanji.
fn check_written_with_kanji(
    word: &str,
    kind_of_spelling: &str,
    line: usize,
    errors: &mut Vec<Error>,
) {
    if !has_kanji(word) {
        errors.push(Error::at_line(
            line,
            format!("{kind_of_spelling} spelling {word} contains no kanji"),
        ));
    }
}

/// Checks the spellings of a reading at `line`.
fn check_spelling(spelling: &Spelling, line: usize, errors: &mut Vec<Error>) {
    let words = spelling.words();
    // Check that the spellings are all different, as equal ones are marked by the spelling tag.
    let distinct_words: HashSet<&str> = words.iter().copied().collect();
    if distinct_words.len() < words.len() {
        errors.push(Error::at_line(line, "spelling contains a repeated word"));
    }
    check_written_with_kanji(spelling.described(), "example", line, errors);
    if let Spelling::SecondaryKanji { main_kanji, described, .. } = spelling {
        check_written_with_kanji(&main_kanji.word, "kanji", line, errors);
        // Check that the example spelling isn't more common than the main kanji spelling.
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

/// Whether the kana at the start and end of a word matches its reading, leaving
/// at least one kana for the rest of the word. Both should have their katakana
/// converted to hiragana.
fn hiragana_matches(word: &str, kana: &str) -> bool {
    if word.chars().all(is_hiragana) {
        return word == kana;
    }
    let word_without_pre = word.trim_start_matches(is_hiragana);
    let pre = &word[..word.len() - word_without_pre.len()];
    let word_without_post = word.trim_end_matches(is_hiragana);
    let post = &word[word_without_post.len()..];
    kana.len() > pre.len() + post.len() && kana.starts_with(pre) && kana.ends_with(post)
}

/// All spellings in a reading including variants, without repetitions.
fn unique_words<K>(reading: &Reading<K>) -> Vec<&str> {
    let mut words = Vec::new();
    for word in reading
        .spelling
        .words()
        .into_iter()
        .chain(reading.variants.iter().map(|v| v.spelling.word.as_str()))
    {
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

/// The canonical spellings of a reading and its variants, empty if it has none.
fn canonical_spellings<K>(reading: &Reading<K>) -> Vec<&str> {
    let variant_canonical_spellings =
        reading.variants.iter().filter_map(|variant| variant.canonical.as_deref());
    reading
        .canonical
        .iter()
        .map(|canonical| canonical.spelling.as_str())
        .chain(variant_canonical_spellings)
        .collect()
}

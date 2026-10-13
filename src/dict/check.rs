//! Validation of the dictionary contents.

use super::*;
use crate::japanese::*;
use std::collections::{HashMap, HashSet};

pub fn check(entries: &[Entry]) -> Vec<Error> {
    let mut errors = Vec::new();
    // Kanji introduced so far.
    let mut known = HashSet::new();
    // Names of entries, which are used as note IDs in the deck.
    let mut names = HashSet::new();
    // Tables of kanji cards with the kanji hidden, to find identical cards.
    let mut tables = HashMap::new();
    for entry in entries {
        let (pos, name) = match entry {
            Entry::Kanji(entry) => (entry.pos, entry.kanji.clone()),
            Entry::Word(entry) => (entry.pos, entry.headword.clone()),
        };
        if !names.insert(name.clone()) {
            errors.push(Error::new(pos, format!("duplicate entry for {name}")));
        }
        match entry {
            Entry::Kanji(entry) => check_kanji(entry, &mut known, &mut tables, &mut errors),
            Entry::Word(entry) => check_word(entry, &known, &mut errors),
        }
    }
    errors
}

fn check_kanji<'a>(entry: &'a KanjiEntry, known: &mut HashSet<char>, tables: &mut HashMap<String, &'a str>, errors: &mut Vec<Error>) {
    let kanji = &entry.kanji;
    let mut chars = kanji.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if is_kanji(c) => {
            known.insert(c);
        }
        _ => errors.push(Error::new(entry.pos, format!("{kanji} is not a single kanji"))),
    }
    if !entry.parts.iter().flat_map(KanjiPart::readings).any(|r| r.core) {
        errors.push(Error::new(entry.pos, format!("{kanji} has no core reading")));
    }
    for part in &entry.parts {
        if part.readings().next().is_none() {
            errors.push(Error::new(entry.pos, format!("{kanji} has a part without readings")));
            continue;
        }
        if let Some(other) = tables.insert(hidden_table(part, kanji), kanji) {
            errors.push(Error::new(entry.pos, format!("{other} and {kanji} have identical cards")));
        }
        for (section, groups) in part.sections() {
            check_groups(groups, entry.pos, errors);
            for reading in groups.iter().flatten() {
                check_reading(reading, section, kanji, errors);
            }
        }
    }
}

fn check_word(entry: &WordEntry, known: &HashSet<char>, errors: &mut Vec<Error>) {
    let Some(first) = entry.readings.iter().flatten().next() else {
        errors.push(Error::new(entry.pos, "word without readings"));
        return;
    };
    if !first.core {
        errors.push(Error::new(entry.pos, "word entry doesn't start with a core reading"));
    }
    if entry.headword != first.spelling.described() {
        errors.push(Error::new(
            entry.pos,
            format!("word {} doesn't match its first example {}", entry.headword, first.spelling.described()),
        ));
    }
    check_groups(&entry.readings, entry.pos, errors);
    for reading in entry.readings.iter().flatten() {
        check_reading(reading, Section::Word, "", errors);
        for word in reading.words() {
            for c in word.chars().filter(|&c| is_kanji(c) && !known.contains(&c)) {
                errors.push(Error::new(reading.pos, format!("{word} contains {c}, which hasn't been introduced yet")));
            }
        }
    }
}

/// Checks that none of the groups is empty. Groups have no position, so
/// errors are reported at the entry's.
fn check_groups(groups: &[ReadingGroup], pos: Pos, errors: &mut Vec<Error>) {
    if groups.iter().any(Vec::is_empty) {
        errors.push(Error::new(pos, "empty group"));
    }
}

/// Checks a reading of `kanji`, which is empty for word entries.
fn check_reading(reading: &Reading, section: Section, kanji: &str, errors: &mut Vec<Error>) {
    let pos = reading.pos;
    if reading.definition.trim().is_empty() {
        errors.push(Error::new(pos, "empty definition"));
    }
    if !kana_fits(&reading.kana, section) {
        errors.push(Error::new(pos, format!("{} is not valid kana for {section:?} readings", reading.kana.joined())));
    }
    for word in reading.words() {
        if word.is_empty() {
            errors.push(Error::new(pos, "empty word"));
        } else if !word.chars().all(|c| c.is_alphanumeric() || c == '_') {
            errors.push(Error::new(pos, format!("{word} is not a valid word")));
        }
    }
    check_spelling(&reading.spelling, pos, errors);
    // Combined readings are vestigial, and are often for words with kanji
    // that are only introduced later.
    if section == Section::Combined {
        return;
    }
    if !kanji.is_empty() {
        let examples = std::iter::once(reading.spelling.described()).chain(reading.variants.iter().map(|v| v.word.as_str()));
        for word in examples {
            if !word.contains(kanji) {
                errors.push(Error::new(pos, format!("{word} doesn't contain {kanji}")));
            }
        }
    }
    let kana = to_hiragana(&reading.kana.joined());
    for word in reading.words() {
        if !kana_matches(&to_hiragana(word), &kana) {
            errors.push(Error::new(pos, format!("{word} doesn't match its reading {}", reading.kana.joined())));
        }
    }
}

fn check_spelling(spelling: &Spelling, pos: Pos, errors: &mut Vec<Error>) {
    let words = spelling.words();
    if (1..words.len()).any(|i| words[..i].contains(&words[i])) {
        errors.push(Error::new(pos, "spelling contains a repeated word"));
    }
    if !has_kanji(spelling.described()) {
        errors.push(Error::new(pos, format!("example spelling {} contains no kanji", spelling.described())));
    }
    if let Spelling::SecondaryKanji { kanji, described, .. } = spelling {
        if !has_kanji(&kanji.word) {
            errors.push(Error::new(pos, format!("kanji spelling {} contains no kanji", kanji.word)));
        }
        if described.freq > kanji.freq {
            errors.push(Error::new(pos, format!("{} is marked as more common than {}", described.word, kanji.word)));
        }
    }
}

fn kana_fits(kana: &Kana, section: Section) -> bool {
    let all = |text: &str, f: fn(char) -> bool| text.chars().all(f);
    match (kana, section) {
        (Kana::Split { pre, mid, post }, Section::On) => {
            !mid.is_empty() && [pre, mid, post].iter().all(|s| all(s, is_katakana))
        }
        (Kana::Split { pre, mid, post }, Section::Kun) => {
            !mid.is_empty() && [pre, mid, post].iter().all(|s| all(s, is_hiragana))
        }
        (Kana::Unsplit(kana), Section::Combined | Section::Word) => {
            !kana.is_empty() && (all(kana, is_hiragana) || all(kana, is_katakana))
        }
        _ => false,
    }
}

/// Whether the kana at the start and end of a word matches its reading. Both
/// are in hiragana.
fn kana_matches(word: &str, kana: &str) -> bool {
    let is_kana = |c: &char| is_hiragana(*c) || is_katakana(*c);
    if word.chars().all(|c| is_kana(&c)) {
        return word == kana;
    }
    let pre: String = word.chars().take_while(is_kana).collect();
    let post: String = word.chars().rev().take_while(is_kana).collect::<Vec<_>>().into_iter().rev().collect();
    kana.len() > pre.len() + post.len() && kana.starts_with(&pre) && kana.ends_with(&post)
}

/// The contents of a kanji card with the kanji hidden.
fn hidden_table(part: &KanjiPart, kanji: &str) -> String {
    part.readings()
        .map(|r| {
            let words: Vec<String> = r.spelling.words().iter().map(|w| w.replace(kanji, "[]")).collect();
            format!("{}\n{}\n{}", words.join("\n"), r.kana.joined(), r.definition)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

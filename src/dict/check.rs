//! Validation of the dictionary contents.

use super::*;
use std::collections::{HashMap, HashSet};

pub fn check(entries: &[Entry]) -> Vec<Error> {
    let mut checker = Checker {
        errors: Vec::new(),
        known: HashSet::from(['々']),
        names: HashSet::new(),
        tables: HashMap::new(),
    };
    for entry in entries {
        match entry {
            Entry::Kanji(entry) => checker.check_kanji(entry),
            Entry::Word(entry) => checker.check_word(entry),
        }
    }
    checker.errors
}

struct Checker<'a> {
    errors: Vec<Error>,
    /// Kanji introduced so far.
    known: HashSet<char>,
    /// Names of entries, which are used as note IDs in the deck.
    names: HashSet<&'a str>,
    /// Tables of kanji cards with the kanji hidden, to find identical cards.
    tables: HashMap<String, &'a str>,
}

impl<'a> Checker<'a> {
    fn error(&mut self, line: usize, message: String) {
        self.errors.push(Error { line, message });
    }

    fn check_kanji(&mut self, entry: &'a KanjiEntry) {
        let kanji = &entry.kanji;
        let mut chars = kanji.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if is_han(c) => {
                self.known.insert(c);
            }
            _ => self.error(entry.line, format!("{kanji} is not a single kanji")),
        }
        if !self.names.insert(kanji) {
            self.error(entry.line, format!("duplicate entry for {kanji}"));
        }
        if !entry.parts.iter().flat_map(KanjiPart::readings).any(|r| r.primary) {
            self.error(entry.line, format!("{kanji} has no primary reading"));
        }
        for part in &entry.parts {
            if let Some(other) = self.tables.insert(hidden_table(part, kanji), kanji) {
                self.error(entry.line, format!("{other} and {kanji} have identical cards"));
            }
            for (section, readings) in part.sections() {
                for reading in readings.iter().flatten() {
                    self.check_reading(reading, section, kanji);
                }
            }
        }
    }

    fn check_word(&mut self, entry: &'a WordEntry) {
        let first = &entry.readings[0][0];
        if !first.primary {
            self.error(entry.line, "word entry doesn't start with a primary reading".into());
        }
        if entry.headword != first.spelling.described() {
            self.error(entry.line, format!("word {} doesn't match its first example {}", entry.headword, first.spelling.described()));
        }
        if !self.names.insert(&entry.headword) {
            self.error(entry.line, format!("duplicate entry for {}", entry.headword));
        }
        for reading in entry.readings.iter().flatten() {
            self.check_reading(reading, Section::Word, "");
            for word in reading.words() {
                let unknown: Vec<char> = word.chars().filter(|&c| is_han(c) && !self.known.contains(&c)).collect();
                for c in unknown {
                    self.error(reading.line, format!("{word} contains {c}, which hasn't been introduced yet"));
                }
            }
        }
    }

    /// Checks a reading of `kanji`, which is empty for word entries.
    fn check_reading(&mut self, reading: &Reading, section: Section, kanji: &str) {
        let line = reading.line;
        if reading.definition.trim().is_empty() {
            self.error(line, "empty definition".into());
        }
        if !kana_fits(&reading.kana, section) {
            self.error(line, format!("{} is not valid kana for {section:?} readings", reading.kana.joined()));
        }
        for word in reading.words() {
            if !word.chars().all(|c| c.is_alphanumeric() || c == '_') {
                self.error(line, format!("{word} is not a valid word"));
            }
        }
        self.check_spelling(&reading.spelling, line);
        // Combined readings are vestigial, and are often for words with kanji
        // that are only introduced later.
        if section == Section::Combined {
            return;
        }
        if !kanji.is_empty() {
            let examples = std::iter::once(reading.spelling.described()).chain(reading.variants.iter().map(|v| v.word.as_str()));
            for word in examples {
                if !word.contains(kanji) {
                    self.error(line, format!("{word} doesn't contain {kanji}"));
                }
            }
        }
        let kana = to_hiragana(&reading.kana.joined());
        for word in reading.words() {
            if !kana_matches(&to_hiragana(word), &kana) {
                self.error(line, format!("{word} doesn't match its reading {}", reading.kana.joined()));
            }
        }
    }

    fn check_spelling(&mut self, spelling: &Spelling, line: usize) {
        let words = spelling.words();
        if (1..words.len()).any(|i| words[..i].contains(&words[i])) {
            self.error(line, "spelling contains a repeated word".into());
        }
        if !has_han(spelling.described()) {
            self.error(line, format!("example spelling {} contains no kanji", spelling.described()));
        }
        if let Spelling::SecondaryKanji { kanji, described, .. } = spelling {
            if !has_han(&kanji.word) {
                self.error(line, format!("kanji spelling {} contains no kanji", kanji.word));
            }
            if described.freq > kanji.freq {
                self.error(line, format!("{} is marked as more common than {}", described.word, kanji.word));
            }
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

/// Whether the kana at the start and end of a word matches its reading.
fn kana_matches(word: &str, kana: &str) -> bool {
    let is_kana = |c: &char| !is_han(*c) && *c != 'ゖ';
    if word.chars().all(|c| is_kana(&c)) {
        return word == kana;
    }
    let pre: String = word.chars().take_while(is_kana).collect();
    let post: String = word.chars().rev().take_while(is_kana).collect::<Vec<_>>().into_iter().rev().collect();
    kana.len() > pre.len() + post.len() && kana.starts_with(&pre) && kana.ends_with(&post)
}

fn to_hiragana(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'ァ'..='ヶ' => char::from_u32(c as u32 - 0x60).unwrap(),
            c => c,
        })
        .collect()
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

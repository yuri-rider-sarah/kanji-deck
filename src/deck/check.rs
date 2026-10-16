//! Finds kanji cards too similar to tell apart with the kanji hidden.

use crate::dict::*;
use crate::error::Error;
use std::collections::HashMap;
use std::fmt;

/// What a reading on a kanji card shows that matters for telling cards apart:
/// its words with the kanji hidden (as `None`), its kana and its definition.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct HiddenReading<'a> {
    words: Vec<Vec<Option<char>>>,
    kana: String,
    definition: &'a str,
}

#[derive(Clone, Copy)]
struct KanjiPartName {
    kanji: char,
    part: Option<usize>,
}

impl fmt::Display for KanjiPartName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.part {
            Some(part) => write!(f, "{} (part {part})", self.kanji),
            None => write!(f, "{}", self.kanji),
        }
    }
}

/// The hidden readings of a card, sorted so that the order of the readings
/// doesn't matter.
fn hidden_readings_from_part<'a>(part: &'a KanjiPart, kanji: char) -> Vec<HiddenReading<'a>> {
    let mut readings: Vec<_> = part.readings()
        .map(|reading| HiddenReading {
            words: reading.spelling.words().iter().map(|word|
                   word.chars().map(|c| (c != kanji).then_some(c)).collect()
                ).collect(),
            kana: reading.kana.joined(),
            definition: &reading.definition,
        })
        .collect();
    readings.sort();
    readings
}

pub fn check(entries: &[Entry]) -> Vec<Error> {
    let mut errors = Vec::new();
    let mut cards = HashMap::new();
    for entry in entries {
        if let Entry::Kanji(entry) = entry {
            for (i, part) in entry.parts.iter().enumerate() {
                // Parts are numbered from 1, but only if there's more than one.
                let name = KanjiPartName {
                    kanji: entry.kanji,
                    part: if entry.parts.len() > 1 { Some(i + 1) } else { None },
                };
                if let Some(other) = cards.insert(hidden_readings_from_part(part, entry.kanji), name) {
                    errors.push(Error::at_line(entry.line, format!("{other} and {name} have cards too similar to tell apart")));
                }
            }
        }
    }
    errors
}

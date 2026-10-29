//! Decomposes words into the readings extracted from the dictionary.

use super::*;
use crate::japanese::*;
use std::collections::HashMap;
use std::str::Chars;

/// A reading along with its normalized spelling.
struct NormalizedReading<'a> {
    reading: &'a Reading,
    spelling: String,
}

/// The readings that words are decomposed into, indexed for matching them.
/// They are grouped by the first character of their normalized spelling.
pub struct ReadingIndex<'a> {
    readings_by_first_char: HashMap<char, Vec<NormalizedReading<'a>>>,
}

/// The kana of a word, read in order, remembering the last kana read (`None` at the start).
#[derive(Clone)]
struct WordKana<'a> {
    rest: Chars<'a>,
    previous: Option<char>,
}

impl WordKana<'_> {
    /// Get the next kana.
    fn next(&mut self) -> Option<char> {
        let c = self.rest.next()?;
        self.previous = Some(c);
        Some(c)
    }

    /// Get the next kana without consuming it.
    fn peek(&self) -> Option<char> {
        self.rest.clone().next()
    }
}

/// Indexes the readings that words are to be decomposed into by grouping them
/// by the first character.
pub fn index_readings(readings: &[Reading]) -> ReadingIndex<'_> {
    let mut readings_by_first_char: HashMap<char, Vec<NormalizedReading>> = HashMap::new();
    for reading in readings {
        let Some(spelling) = normalize_spelling(&reading.spelling) else {
            continue;
        };
        let Some(first_char) = spelling.chars().next() else {
            continue;
        };
        readings_by_first_char
            .entry(first_char)
            .or_default()
            .push(NormalizedReading { spelling, reading });
    }
    ReadingIndex { readings_by_first_char }
}

/// Whether a word can be decomposed into the indexed readings.
pub fn can_decompose_word(word: &Word, reading_index: &ReadingIndex) -> bool {
    let Some(spelling) = normalize_spelling(&word.spelling) else {
        return false;
    };
    let kana = WordKana { rest: word.kana.chars(), previous: None };
    can_decompose(spelling.chars(), kana, &reading_index.readings_by_first_char)
}

/// Normalizes a spelling by converting katakana to hiragana and replacing each 々
/// with the kanji before it. `None` if a 々 doesn't come after a kanji.
fn normalize_spelling(spelling: &str) -> Option<String> {
    let mut normalized_spelling = String::new();
    for c in katakana_to_hiragana(spelling).chars() {
        if c == '々' {
            let previous = normalized_spelling.chars().next_back().filter(|&c| is_kanji(c))?;
            normalized_spelling.push(previous);
        } else {
            normalized_spelling.push(c);
        }
    }
    Some(normalized_spelling)
}

/// Whether the rest of a word's normalized spelling and the rest of its kana can be decomposed
/// into readings.
fn can_decompose(
    mut spelling: Chars,
    mut kana: WordKana,
    readings_by_first_char: &HashMap<char, Vec<NormalizedReading>>,
) -> bool {
    let Some(first_char) = spelling.clone().next() else {
        return kana.peek().is_none();
    };
    let Some(first_kana) = kana.peek() else {
        return false;
    };
    if is_hiragana(first_char) || matches!(first_char, 'ヶ' | 'ヵ' | 'ー') {
        if first_char == first_kana
            || matches!((first_char, first_kana), ('ヶ' | 'ヵ', 'か' | 'が' | 'こ'))
        {
            spelling.next();
            kana.next();
            return can_decompose(spelling, kana, readings_by_first_char);
        } else {
            return false;
        }
    }
    let Some(readings) = readings_by_first_char.get(&first_char) else {
        return false;
    };
    readings.iter().any(|reading| {
        let Some(spelling_rest) = spelling.as_str().strip_prefix(reading.spelling.as_str()) else {
            return false;
        };
        let mut kana_rest = kana.clone();
        match_reading(&mut kana_rest, reading.reading)
            && okurigana_fits(reading.reading.okurigana_start, spelling_rest.chars().next())
            && can_decompose(spelling_rest.chars(), kana_rest, readings_by_first_char)
    })
}

/// Try to match a reading's kana at the start of the given kana with the changes
/// the reading allows, advancing past it.
fn match_reading(word_kana: &mut WordKana, reading: &Reading) -> bool {
    let mut reading_kana = reading.kana.chars();
    let Some(first_reading_char) = reading_kana.next() else {
        return true;
    };
    let previous_kana = word_kana.previous;
    let Some(first_word_kana_char) = word_kana.next() else {
        return false;
    };
    let first_kana_matches = first_word_kana_char == first_reading_char
        || (reading.rendaku
            && previous_kana.is_some()
            && add_dakuten(first_reading_char) == Some(first_word_kana_char))
        || (reading.handakuten
            && matches!(previous_kana, Some('っ' | 'ん'))
            && add_handakuten(first_reading_char) == Some(first_word_kana_char));
    if !first_kana_matches {
        return false;
    }
    while let Some(reading_char) = reading_kana.next() {
        let Some(word_kana_char) = word_kana.next() else {
            return false;
        };
        let kana_matches = word_kana_char == reading_char
            || (reading.gemination
                && reading_kana.as_str().is_empty()
                && word_kana_char == 'っ'
                && geminates_before(reading_char, word_kana.peek()));
        if !kana_matches {
            return false;
        }
    }
    true
}

/// Whether the last kana of a reading may become っ before `next_kana`.
fn geminates_before(reading_kana: char, next_kana: Option<char>) -> bool {
    let next_start = next_kana.and_then(okurigana_start_from_kana);
    match reading_kana {
        'つ' | 'ち' => matches!(
            next_start,
            Some(OkuriganaStart::K | OkuriganaStart::S | OkuriganaStart::T | OkuriganaStart::P)
        ),
        'き' | 'く' => next_start == Some(OkuriganaStart::K),
        _ => false,
    }
}

/// Whether the character after a reading in a word's spelling (`None` at the end of the word)
/// fits the okurigana that has to follow the reading, if any.
/// い, え, and お have their own `OkuriganaStart` values, but they can also
/// be treated as if starting with 'w'.
fn okurigana_fits(okurigana_start: Option<OkuriganaStart>, c: Option<char>) -> bool {
    match (okurigana_start, c) {
        (None, _) => true,
        (Some(okurigana_start), Some(c)) => {
            okurigana_start_from_kana(c) == Some(okurigana_start)
                || (okurigana_start == OkuriganaStart::W && matches!(c, 'い' | 'え' | 'お'))
        }
        (Some(_), None) => false,
    }
}

/// Adds a dakuten to a hiragana character. `None` if there's no corresponding character.
fn add_dakuten(c: char) -> Option<char> {
    let with_dakuten = match c {
        'か' => 'が',
        'き' => 'ぎ',
        'く' => 'ぐ',
        'け' => 'げ',
        'こ' => 'ご',
        'さ' => 'ざ',
        'し' => 'じ',
        'す' => 'ず',
        'せ' => 'ぜ',
        'そ' => 'ぞ',
        'た' => 'だ',
        'ち' => 'ぢ',
        'つ' => 'づ',
        'て' => 'で',
        'と' => 'ど',
        'は' => 'ば',
        'ひ' => 'び',
        'ふ' => 'ぶ',
        'へ' => 'べ',
        'ほ' => 'ぼ',
        _ => return None,
    };
    Some(with_dakuten)
}

/// Adds a handakuten to a hiragana character. `None` if there's no corresponding character.
fn add_handakuten(c: char) -> Option<char> {
    let with_handakuten = match c {
        'は' => 'ぱ',
        'ひ' => 'ぴ',
        'ふ' => 'ぷ',
        'へ' => 'ぺ',
        'ほ' => 'ぽ',
        _ => return None,
    };
    Some(with_handakuten)
}

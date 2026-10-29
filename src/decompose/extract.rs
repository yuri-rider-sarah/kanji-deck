//! Extracts the readings that words are decomposed into from the entries of the dictionary.

use super::*;
use crate::dict::{self, Entry, ReadingKana, SplitKana, UnsplitKana};
use crate::error::{Error, Result};
use crate::japanese::*;
use std::collections::HashMap;

/// A spelling of a kun reading, split into the part read with the reading and its okurigana.
struct KunSplit {
    /// The part of the spelling the reading belongs to: the kanji in kanji entries,
    /// and the rest of the spelling without its okurigana in word entries.
    main_spelling: String,
    /// The kana of the main part, in hiragana.
    main_kana: String,
    /// The kana after the main part of the spelling, in hiragana.
    okurigana: String,
}

/// Extracts the readings of all entries, in the order of the dictionary. A reading can be
/// extracted more than once, from different readings of the dictionary. Kun readings get
/// their part of speech from the dictionary if it's given there, otherwise from
/// `parts_of_speech`. Fails with all errors found.
pub fn extract_readings(
    entries: &[Entry],
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>, Vec<Error>> {
    // The readings extracted from each reading of the dictionary, or the error for it.
    let mut results = Vec::new();
    for entry in entries {
        match entry {
            Entry::Kanji(entry) => {
                for part in &entry.parts {
                    for reading_type in dict::ReadingType::ALL {
                        for reading in part[reading_type].iter().flatten() {
                            results.push(readings_from_kanji_reading(
                                reading,
                                reading_type,
                                entry.kanji,
                                parts_of_speech,
                            ));
                        }
                    }
                }
            }
            Entry::Word(entry) => {
                for reading_type in dict::ReadingType::ALL {
                    for reading in entry.part[reading_type].iter().flatten() {
                        results.push(readings_from_word_reading(
                            reading,
                            reading_type,
                            parts_of_speech,
                        ));
                    }
                }
            }
        }
    }
    let mut readings = Vec::new();
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(extracted_readings) => readings.extend(extracted_readings),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() { Ok(readings) } else { Err(errors) }
}

/// The readings from a reading of a kanji entry, of the given type.
fn readings_from_kanji_reading(
    reading: &dict::Reading<SplitKana>,
    reading_type: dict::ReadingType,
    kanji: char,
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>> {
    match reading_type {
        dict::ReadingType::On => Ok(vec![on_reading_from_kanji(reading, kanji)]),
        dict::ReadingType::Asian | dict::ReadingType::European => Ok(vec![foreign_reading(
            kanji.to_string(),
            katakana_to_hiragana_in_str(&reading.canonical_kana().mid),
        )]),
        dict::ReadingType::Kun => {
            let (reading_split, variant_splits) = kun_splits_from_kanji_reading(reading, kanji)?;
            readings_from_kun_splits(reading_split, variant_splits, reading, parts_of_speech)
        }
    }
}

/// The readings from a reading of a word entry, of the given type.
fn readings_from_word_reading(
    reading: &dict::Reading<UnsplitKana>,
    reading_type: dict::ReadingType,
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>> {
    match reading_type {
        dict::ReadingType::On => {
            let (spelling, kana) = non_kun_spelling_and_kana_from_word(reading)?;
            Ok(vec![on_reading(spelling, kana)])
        }
        dict::ReadingType::Asian | dict::ReadingType::European => {
            let (spelling, kana) = non_kun_spelling_and_kana_from_word(reading)?;
            Ok(vec![foreign_reading(spelling, kana)])
        }
        dict::ReadingType::Kun => {
            let (reading_split, variant_splits) = kun_splits_from_word_reading(reading)?;
            readings_from_kun_splits(reading_split, variant_splits, reading, parts_of_speech)
        }
    }
}

/// The reading of a kanji from an on reading of its entry.
fn on_reading_from_kanji(reading: &dict::Reading<SplitKana>, kanji: char) -> Reading {
    let example_kana = reading.canonical_kana();
    let kanji_kana = katakana_to_hiragana_in_str(&example_kana.mid);
    let kana_before = katakana_to_hiragana_in_str(&example_kana.pre);
    let kana = if kana_before.ends_with(['ん', 'っ']) {
        remove_initial_handakuten(&kanji_kana)
    } else {
        kanji_kana
    };
    on_reading(kanji.to_string(), kana)
}

/// Removes the handakuten from the first character of hiragana, if it has one.
fn remove_initial_handakuten(kana: &str) -> String {
    let mut chars = kana.chars();
    let first_char = match chars.next() {
        Some('ぱ') => 'は',
        Some('ぴ') => 'ひ',
        Some('ぷ') => 'ふ',
        Some('ぺ') => 'へ',
        Some('ぽ') => 'ほ',
        Some(c) => c,
        None => return String::new(),
    };
    format!("{first_char}{}", chars.as_str())
}

/// The spelling and kana of a word's reading from an on, Asian or European reading of its entry.
fn non_kun_spelling_and_kana_from_word(
    reading: &dict::Reading<UnsplitKana>,
) -> Result<(String, String)> {
    let kana = katakana_to_hiragana_in_str(&reading.canonical_kana().joined());
    let (spelling, kana) = strip_initial_kana(reading.canonical_spelling(), &kana, reading.line)?;
    Ok((spelling.to_string(), kana.to_string()))
}

/// An on reading.
fn on_reading(spelling: String, kana: String) -> Reading {
    Reading {
        spelling,
        kana,
        rendaku: false,
        handakuten: true,
        gemination: true,
        okurigana_start: None,
    }
}

/// A reading of a borrowing, whose kana doesn't change in words.
fn foreign_reading(spelling: String, kana: String) -> Reading {
    Reading {
        spelling,
        kana,
        rendaku: false,
        handakuten: false,
        gemination: false,
        okurigana_start: None,
    }
}

/// A kun reading, followed by okurigana starting in the given way, or by anything if `None`.
fn kun_reading(spelling: String, kana: String, okurigana_start: Option<OkuriganaStart>) -> Reading {
    Reading { spelling, kana, rendaku: true, handakuten: false, gemination: false, okurigana_start }
}

/// Splits the spelling of a kun reading of `kanji`, and those of its variants.
/// The okurigana is the kana directly after the kanji.
fn kun_splits_from_kanji_reading(
    reading: &dict::Reading<SplitKana>,
    kanji: char,
) -> Result<(KunSplit, Vec<KunSplit>)> {
    let okurigana = okurigana_after_kanji(reading.canonical_spelling(), kanji, reading.line)?;
    let reading_split = KunSplit {
        main_spelling: kanji.to_string(),
        main_kana: reading.canonical_kana().mid.clone(),
        okurigana,
    };
    let variant_splits = reading
        .variants
        .iter()
        .map(|variant| {
            let variant_spelling = variant.canonical_spelling();
            let variant_okurigana = okurigana_after_kanji(variant_spelling, kanji, reading.line)?;
            variant_split(
                &reading_split,
                kanji.to_string(),
                variant_okurigana,
                variant_spelling,
                reading.line,
            )
        })
        .collect::<Result<_>>()?;
    Ok((reading_split, variant_splits))
}

/// Splits the spelling of a kun reading of a word, and those of its variants.
/// The okurigana is all kana at the end of the spelling.
fn kun_splits_from_word_reading(
    reading: &dict::Reading<UnsplitKana>,
) -> Result<(KunSplit, Vec<KunSplit>)> {
    let spelling = reading.canonical_spelling();
    let (main_spelling, okurigana) = split_okurigana_from_end(spelling, reading.line)?;
    let kana = katakana_to_hiragana_in_str(&reading.canonical_kana().joined());
    let Some(main_kana) = kana.strip_suffix(&okurigana) else {
        return Err(Error::at_line(
            reading.line,
            format!("okurigana {okurigana} of {spelling} isn't the end of its kana {kana}"),
        ));
    };
    let reading_split = KunSplit { main_spelling, main_kana: main_kana.to_string(), okurigana };
    let variant_splits = reading
        .variants
        .iter()
        .map(|variant| {
            let variant_spelling = variant.canonical_spelling();
            let (variant_main_spelling, variant_okurigana) =
                split_okurigana_from_end(variant_spelling, reading.line)?;
            variant_split(
                &reading_split,
                variant_main_spelling,
                variant_okurigana,
                variant_spelling,
                reading.line,
            )
        })
        .collect::<Result<_>>()?;
    Ok((reading_split, variant_splits))
}

/// Splits the spelling of a variant of a reading with the split `reading_split`, given the
/// variant's main spelling and okurigana. The okurigana of the variant has to be the end of
/// the reading's main kana and okurigana, and the rest of them is the variant's main kana.
fn variant_split(
    reading_split: &KunSplit,
    main_spelling: String,
    okurigana: String,
    variant_spelling: &str,
    line: usize,
) -> Result<KunSplit> {
    let reading_kana = reading_split.main_kana.clone() + &reading_split.okurigana;
    match reading_kana.strip_suffix(&okurigana) {
        Some(main_kana) => {
            Ok(KunSplit { main_spelling, main_kana: main_kana.to_string(), okurigana })
        }
        None => Err(Error::at_line(
            line,
            format!("okurigana {okurigana} of {variant_spelling} isn't the end of {reading_kana}"),
        )),
    }
}

/// The kana directly after `kanji` in a word, converted to hiragana. Fails if the kanji
/// isn't in the word exactly once, as it's then unclear where the okurigana is.
fn okurigana_after_kanji(word: &str, kanji: char, line: usize) -> Result<String> {
    let Some((_, after_kanji)) = word.split_once(kanji) else {
        return Err(Error::at_line(line, format!("{word} doesn't contain {kanji}")));
    };
    if after_kanji.contains(kanji) {
        return Err(Error::at_line(line, format!("{word} contains {kanji} more than once")));
    }
    let okurigana = after_kanji
        .chars()
        .take_while(|&c| is_okurigana_char(c))
        .map(katakana_to_hiragana)
        .collect();
    Ok(okurigana)
}

/// Splits a word into the part before the kana at its end, and that kana converted to
/// hiragana. Fails if the word is only kana, as there's then nothing for the reading.
fn split_okurigana_from_end(word: &str, line: usize) -> Result<(String, String)> {
    let main_spelling = word.trim_end_matches(is_okurigana_char);
    if main_spelling.is_empty() {
        return Err(Error::at_line(line, format!("{word} is written only in kana")));
    }
    let okurigana = katakana_to_hiragana_in_str(&word[main_spelling.len()..]);
    Ok((main_spelling.to_string(), okurigana))
}

/// Removes the kana at the start of a word's spelling from the spelling and its kana, as words
/// match kana in their spelling before trying readings (お洒落 gives 洒落 しゃれ).
fn strip_initial_kana<'a>(
    spelling: &'a str,
    kana: &'a str,
    line: usize,
) -> Result<(&'a str, &'a str)> {
    let main_spelling = spelling.trim_start_matches(|c| is_hiragana(c) || is_katakana(c));
    if main_spelling.is_empty() {
        return Err(Error::at_line(line, format!("{spelling} is written only in kana")));
    }
    let initial_kana =
        katakana_to_hiragana_in_str(&spelling[..spelling.len() - main_spelling.len()]);
    match kana.strip_prefix(initial_kana.as_str()) {
        Some(main_kana) => Ok((main_spelling, main_kana)),
        None => Err(Error::at_line(
            line,
            format!(
                "kana {initial_kana} at the start of {spelling} isn't the start of its kana {kana}"
            ),
        )),
    }
}

/// Whether a character can be part of okurigana: hiragana or katakana other than を,
/// which in modern Japanese is only used as a particle.
fn is_okurigana_char(c: char) -> bool {
    (is_hiragana(c) || is_katakana(c)) && !matches!(c, 'を' | 'ヲ')
}

/// The readings from the split spellings of a kun reading and its variants. The part of
/// speech is the one given in the dictionary, otherwise that of the reading's own spelling
/// in `parts_of_speech`, and applies to the variants too.
fn readings_from_kun_splits<K>(
    reading_split: KunSplit,
    variant_splits: Vec<KunSplit>,
    reading: &dict::Reading<K>,
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>> {
    let part_of_speech = match reading.part_of_speech {
        Some(part_of_speech) => part_of_speech_from_dictionary(part_of_speech),
        None => {
            let word = Word {
                spelling: reading_split.main_spelling.clone() + &reading_split.okurigana,
                kana: reading_split.main_kana.clone() + &reading_split.okurigana,
            };
            parts_of_speech.get(&word).copied()
        }
    };
    let readings_by_split: Vec<Vec<Reading>> = std::iter::once(reading_split)
        .chain(variant_splits)
        .map(|split| readings_from_kun_split(&split, part_of_speech, reading.line))
        .collect::<Result<_>>()?;
    Ok(readings_by_split.into_iter().flatten().collect())
}

/// The readings from one split spelling of a kun reading at `line` with the given part of
/// speech: a reading with okurigana starting like the split's, or with no okurigana
/// if the split doesn't have any or if it's the い of an i-adjective. For ichidan and godan verbs,
/// adds a reading with no okurigana for the continuative form (e.g. 受ける うける gives 受 うけ).
fn readings_from_kun_split(
    split: &KunSplit,
    part_of_speech: Option<PartOfSpeech>,
    line: usize,
) -> Result<Vec<Reading>> {
    let (main_spelling, main_kana) =
        strip_initial_kana(&split.main_spelling, &split.main_kana, line)?;
    let first_okurigana = match split.okurigana.chars().next() {
        Some(c) => c,
        None => {
            return Ok(vec![kun_reading(main_spelling.to_string(), main_kana.to_string(), None)]);
        }
    };
    if part_of_speech == Some(PartOfSpeech::Adjective) && split.okurigana == "い" {
        return Ok(vec![kun_reading(main_spelling.to_string(), main_kana.to_string(), None)]);
    }
    let Some(okurigana_start) = okurigana_start_from_kana(first_okurigana) else {
        return Err(Error::at_line(
            line,
            format!(
                "okurigana {} starts with {first_okurigana}, which okurigana can't start with",
                split.okurigana
            ),
        ));
    };
    let mut readings =
        vec![kun_reading(main_spelling.to_string(), main_kana.to_string(), Some(okurigana_start))];
    let kana_with_okurigana = main_kana.to_string() + &split.okurigana;
    let continuative_kana = match part_of_speech {
        Some(PartOfSpeech::IchidanVerb) => match kana_with_okurigana.strip_suffix('る') {
            Some(stem) => Some(stem.to_string()),
            None => {
                return Err(Error::at_line(
                    line,
                    format!("ichidan verb {kana_with_okurigana} doesn't end with る"),
                ));
            }
        },
        Some(PartOfSpeech::GodanVerb) => match godan_continuative_kana(&kana_with_okurigana) {
            Some(continuative_kana) => Some(continuative_kana),
            None => {
                return Err(Error::at_line(
                    line,
                    format!(
                        "godan verb {kana_with_okurigana} doesn't end with a kana of the u row"
                    ),
                ));
            }
        },
        Some(PartOfSpeech::Adjective) | None => None,
    };
    if let Some(continuative_kana) = continuative_kana {
        readings.push(kun_reading(main_spelling.to_string(), continuative_kana, None));
    }
    Ok(readings)
}

/// The kana of the continuative form (連用形) of a godan verb: its last kana changed from
/// the u row to the i row, `None` if it doesn't end with a u-row kana.
fn godan_continuative_kana(kana: &str) -> Option<String> {
    let mut chars = kana.chars();
    let last = chars.next_back()?;
    let continuative_last = match last {
        'う' => 'い',
        'く' => 'き',
        'ぐ' => 'ぎ',
        'す' => 'し',
        'つ' => 'ち',
        'ぬ' => 'に',
        'ぶ' => 'び',
        'む' => 'み',
        'る' => 'り',
        _ => return None,
    };
    Some(format!("{}{continuative_last}", chars.as_str()))
}

/// The part of speech given in the dictionary, or `None` for other parts of speech.
fn part_of_speech_from_dictionary(part_of_speech: dict::PartOfSpeech) -> Option<PartOfSpeech> {
    match part_of_speech {
        dict::PartOfSpeech::IchidanVerb => Some(PartOfSpeech::IchidanVerb),
        dict::PartOfSpeech::GodanVerb => Some(PartOfSpeech::GodanVerb),
        dict::PartOfSpeech::Adjective => Some(PartOfSpeech::Adjective),
        dict::PartOfSpeech::Other | dict::PartOfSpeech::Noun => None,
    }
}

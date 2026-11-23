//! Extracts the readings that words are decomposed into from the entries of the dictionary.

use super::*;
use crate::dict::{self, CanonicalForm, Entry, ReadingKana, SplitKana, UnsplitKana};
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

/// Extracts the readings of all entries, in the order of the dictionary, from the canonical
/// forms of their readings. A reading can be extracted more than once, from different
/// readings of the dictionary. Kun readings get their part of speech from the dictionary
/// if it's given there, otherwise from `parts_of_speech`. Fails with all errors found.
pub fn extract_readings(
    entries: &[Entry],
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>, Vec<Error>> {
    // The readings extracted from each canonical form of a reading of the dictionary,
    // or the error for it.
    let mut results = Vec::new();
    for entry in entries {
        match entry {
            Entry::Kanji(entry) => {
                for part in &entry.parts {
                    for reading_type in dict::ReadingType::ALL {
                        for reading in part[reading_type].iter().flatten() {
                            for form in reading.canonical_forms() {
                                results.push(readings_from_kanji_reading(
                                    reading,
                                    form,
                                    reading_type,
                                    entry.kanji,
                                    parts_of_speech,
                                ));
                            }
                        }
                    }
                }
            }
            Entry::Word(entry) => {
                for reading_type in dict::ReadingType::ALL {
                    for reading in entry.part[reading_type].iter().flatten() {
                        for form in reading.canonical_forms() {
                            results.push(readings_from_word_reading(
                                reading,
                                form,
                                reading_type,
                                parts_of_speech,
                            ));
                        }
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

/// The readings from a canonical form of a reading of a kanji entry, of the given type.
fn readings_from_kanji_reading(
    reading: &dict::Reading<SplitKana>,
    form: CanonicalForm<SplitKana>,
    reading_type: dict::ReadingType,
    kanji: char,
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>> {
    let mut readings = match reading_type {
        dict::ReadingType::On => vec![on_reading_from_kanji(form.kana, kanji)],
        dict::ReadingType::Asian | dict::ReadingType::European => {
            vec![foreign_reading(kanji.to_string(), katakana_to_hiragana_in_str(&form.kana.mid))]
        }
        dict::ReadingType::Kun => {
            let (reading_split, variant_splits) =
                kun_splits_from_kanji_reading(form, kanji, reading.line)?;
            readings_from_kun_splits(reading_split, variant_splits, reading, parts_of_speech)?
        }
    };
    for extracted_reading in &mut readings {
        extracted_reading.rendaku_after = reading.rendaku_after;
    }
    Ok(readings)
}

/// The readings from a canonical form of a reading of a word entry, of the given type.
fn readings_from_word_reading(
    reading: &dict::Reading<UnsplitKana>,
    form: CanonicalForm<UnsplitKana>,
    reading_type: dict::ReadingType,
    parts_of_speech: &HashMap<Word, PartOfSpeech>,
) -> Result<Vec<Reading>> {
    let mut readings = match reading_type {
        dict::ReadingType::On => {
            let (spelling, kana) = non_kun_spelling_and_kana_from_word(form, reading.line)?;
            vec![on_reading(spelling, kana)]
        }
        dict::ReadingType::Asian | dict::ReadingType::European => {
            let (spelling, kana) = non_kun_spelling_and_kana_from_word(form, reading.line)?;
            vec![foreign_reading(spelling, kana)]
        }
        dict::ReadingType::Kun => {
            let (reading_split, variant_splits) = kun_splits_from_word_reading(form, reading.line)?;
            readings_from_kun_splits(reading_split, variant_splits, reading, parts_of_speech)?
        }
    };
    for extracted_reading in &mut readings {
        extracted_reading.rendaku_after = reading.rendaku_after;
    }
    Ok(readings)
}

/// The reading of a kanji from the kana of an on reading of its entry.
fn on_reading_from_kanji(example_kana: &SplitKana, kanji: char) -> Reading {
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

/// The spelling and kana of a word's reading from a canonical form of an on, Asian or European
/// reading of its entry at `line`, without the kana at the start and end of the spelling.
fn non_kun_spelling_and_kana_from_word(
    form: CanonicalForm<UnsplitKana>,
    line: usize,
) -> Result<(String, String)> {
    let kana = katakana_to_hiragana_in_str(&form.kana.joined());
    let (spelling, kana) = strip_initial_kana(form.spelling, &kana, line)?;
    let (spelling, kana) = strip_final_kana(spelling, kana, line)?;
    Ok((spelling.to_string(), kana.to_string()))
}

/// An on reading.
fn on_reading(spelling: String, kana: String) -> Reading {
    Reading {
        spelling,
        kana,
        rendaku: Rendaku::Conditional,
        handakuten: true,
        gemination: true,
        rendaku_after: false,
        okurigana_start: None,
    }
}

/// A reading of a borrowing, whose kana doesn't change in words.
fn foreign_reading(spelling: String, kana: String) -> Reading {
    Reading {
        spelling,
        kana,
        rendaku: Rendaku::Never,
        handakuten: false,
        gemination: false,
        rendaku_after: false,
        okurigana_start: None,
    }
}

/// A kun reading, followed by okurigana starting in the given way, or by anything if `None`.
fn kun_reading(spelling: String, kana: String, okurigana_start: Option<OkuriganaStart>) -> Reading {
    Reading {
        spelling,
        kana,
        rendaku: Rendaku::Full,
        handakuten: true,
        gemination: false,
        rendaku_after: false,
        okurigana_start,
    }
}

/// Splits the spelling of a canonical form of a kun reading of `kanji`, and those of its
/// variants. The okurigana is the kana directly after the kanji.
fn kun_splits_from_kanji_reading(
    form: CanonicalForm<SplitKana>,
    kanji: char,
    line: usize,
) -> Result<(KunSplit, Vec<KunSplit>)> {
    let okurigana = find_kana_after_kanji(form.spelling, kanji, line)?;
    let main_kana = form.kana.mid.clone();
    let reading_kana = main_kana.clone() + &okurigana;
    let reading_split = KunSplit { main_spelling: kanji.to_string(), main_kana, okurigana };
    let variant_splits = form
        .variant_spellings
        .iter()
        .map(|&variant_spelling| {
            let variant_okurigana = find_kana_after_kanji(variant_spelling, kanji, line)?;
            variant_split(
                &reading_kana,
                kanji.to_string(),
                variant_okurigana,
                variant_spelling,
                line,
            )
        })
        .collect::<Result<_>>()?;
    Ok((reading_split, variant_splits))
}

/// Splits the spelling of a canonical form of a kun reading of a word, and those of its
/// variants. The okurigana is all kana at the end of the spelling.
fn kun_splits_from_word_reading(
    form: CanonicalForm<UnsplitKana>,
    line: usize,
) -> Result<(KunSplit, Vec<KunSplit>)> {
    let spelling = form.spelling;
    let (main_spelling, okurigana) = split_okurigana_from_end(spelling, line)?;
    let kana = katakana_to_hiragana_in_str(&form.kana.joined());
    let Some(main_kana) = kana.strip_suffix(&okurigana) else {
        return Err(Error::at_line(
            line,
            format!("okurigana {okurigana} of {spelling} isn't the end of its kana {kana}"),
        ));
    };
    let reading_split = KunSplit { main_spelling, main_kana: main_kana.to_string(), okurigana };
    let reading_kana = reading_split.main_kana.clone() + &reading_split.okurigana;
    let variant_splits = form
        .variant_spellings
        .iter()
        .map(|&variant_spelling| {
            let (variant_main_spelling, variant_okurigana) =
                split_okurigana_from_end(variant_spelling, line)?;
            variant_split(
                &reading_kana,
                variant_main_spelling,
                variant_okurigana,
                variant_spelling,
                line,
            )
        })
        .collect::<Result<_>>()?;
    Ok((reading_split, variant_splits))
}

/// Splits the spelling of a variant of a reading whose kana from the start of its main part
/// is `reading_kana`, given the variant's main spelling and okurigana. The okurigana of
/// the variant has to be the end of that kana, and the rest of it is the variant's main kana.
fn variant_split(
    reading_kana: &str,
    main_spelling: String,
    okurigana: String,
    variant_spelling: &str,
    line: usize,
) -> Result<KunSplit> {
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

/// The kana directly after `kanji` in a word, converted to hiragana. Fails if the
/// kanji isn't in the word exactly once, as it's then unclear where the okurigana is.
fn find_kana_after_kanji(word: &str, kanji: char, line: usize) -> Result<String> {
    let Some((_, after_kanji)) = word.split_once(kanji) else {
        return Err(Error::at_line(line, format!("{word} doesn't contain {kanji}")));
    };
    if after_kanji.contains(kanji) {
        return Err(Error::at_line(line, format!("{word} contains {kanji} more than once")));
    }
    let kana = after_kanji
        .chars()
        .take_while(|&c| is_hiragana(c) || is_katakana(c))
        .map(katakana_to_hiragana)
        .collect();
    Ok(kana)
}

/// Splits a word into the part before the kana at its end, and that kana converted to
/// hiragana. Fails if the word is only kana, as there's then nothing for the reading.
fn split_okurigana_from_end(word: &str, line: usize) -> Result<(String, String)> {
    let main_spelling = word.trim_end_matches(|c| is_hiragana(c) || is_katakana(c));
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

/// Removes the kana at the end of a word's spelling from the spelling and its kana, as
/// for on readings of kanji, so that the reading also matches in other words
/// (可愛い gives 可愛 かわい, also found in 可愛らしい).
fn strip_final_kana<'a>(
    spelling: &'a str,
    kana: &'a str,
    line: usize,
) -> Result<(&'a str, &'a str)> {
    let main_spelling = spelling.trim_end_matches(|c| is_hiragana(c) || is_katakana(c));
    let final_kana = katakana_to_hiragana_in_str(&spelling[main_spelling.len()..]);
    match kana.strip_suffix(final_kana.as_str()) {
        Some(main_kana) => Ok((main_spelling, main_kana)),
        None => Err(Error::at_line(
            line,
            format!("kana {final_kana} at the end of {spelling} isn't the end of its kana {kana}"),
        )),
    }
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
/// speech: a reading with okurigana starting like the split's, or with no okurigana if the
/// split doesn't have any or if it's the い or しい of an adjective (涼しい gives 涼 すず).
/// For ichidan and godan verbs, and for 来る and 為る, adds a reading with no okurigana for the
/// continuative form (e.g. 受ける うける gives 受 うけ).
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
    if part_of_speech == Some(PartOfSpeech::Adjective)
        && matches!(split.okurigana.as_str(), "い" | "しい")
    {
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
        Some(PartOfSpeech::KuruVerb) if main_kana == "く" && split.okurigana == "る" => {
            Some("き".to_string())
        }
        Some(PartOfSpeech::SuruVerb) if main_kana == "す" && split.okurigana == "る" => {
            Some("し".to_string())
        }
        Some(PartOfSpeech::KuruVerb | PartOfSpeech::SuruVerb | PartOfSpeech::Adjective) | None => {
            None
        }
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
        dict::PartOfSpeech::KuruVerb => Some(PartOfSpeech::KuruVerb),
        dict::PartOfSpeech::SuruVerb => Some(PartOfSpeech::SuruVerb),
        dict::PartOfSpeech::Adjective => Some(PartOfSpeech::Adjective),
        dict::PartOfSpeech::Other | dict::PartOfSpeech::Noun => None,
    }
}

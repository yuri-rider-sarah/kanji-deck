//! Reads the lexicon of UniDic (`lex.csv`): the words to decompose, and the parts of speech
//! that decomposition treats differently from the rest.

use crate::decompose::{PartOfSpeech, Word};
use crate::error::{Error, Result};
use crate::japanese::katakana_to_hiragana;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// The number of fields on each line of the lexicon.
const FIELD_COUNT: usize = 33;

/// The part of speech of a word according to one or more of its rows in the lexicon.
/// Verbs other than ichidan and godan (サ変, カ変 and classical verbs) are kept apart
/// from other parts of speech, as they give way to an ichidan or godan row of the same word.
/// Conflicting is a value of its own, so that once rows conflict, later rows can't resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexiconPartOfSpeech {
    IchidanVerb,
    GodanVerb,
    OtherVerb,
    Adjective,
    Other,
    Conflicting,
}

/// The fields of a line of the lexicon that decomposition needs, named as in UniDic's
/// documentation where it gives them a name.
struct Row<'a> {
    /// The main part of speech (`pos1`), such as 動詞 or 接尾辞.
    pos1: &'a str,
    /// The subcategory of the part of speech (`pos2`), such as 固有名詞 or 動詞的.
    pos2: &'a str,
    /// The conjugation type, such as 五段-ラ行, or `*` for words that don't conjugate.
    conjugation_type: &'a str,
    /// The conjugation form, such as 終止形-一般, or `*` for words that don't conjugate.
    conjugation_form: &'a str,
    /// The written form of this row (`orth`).
    spelling: &'a str,
    /// Whether rendaku applies to this row (`iForm`): 基本形 or `*` if not.
    initial_form: &'a str,
    /// Whether a change at the end of the word, such as gemination, applies to this row
    /// (`fForm`): 基本形 or `*` if not.
    final_form: &'a str,
    /// The kana of the written form in katakana, which follows historical kana usage
    /// if the written form does (`kana`).
    kana: &'a str,
    /// The kana of the word in modern kana usage (`form`).
    modern_kana: &'a str,
}

/// Reads the lexicon, returning every word to decompose in the order of the file, and the
/// parts of speech of the words that are ichidan or godan verbs, or adjectives. Words whose
/// rows conflict are left out of the parts of speech, like words of other parts of speech.
/// Words are spelling and kana pairs, so a spelling read in two ways gives two words.
pub fn read_lexicon(text: &str) -> Result<(Vec<Word>, HashMap<Word, PartOfSpeech>)> {
    let mut reader =
        csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(text.as_bytes());
    let mut record = csv::StringRecord::new();
    let mut words = Vec::new();
    let mut lexicon_parts_of_speech = HashMap::new();
    loop {
        // The line the next record starts on.
        let line_number = reader.position().line() as usize;
        match reader.read_record(&mut record) {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => return Err(Error::at_line(line_number, error.to_string())),
        }
        let row = read_row(&record, line_number)?;
        if !is_word_to_decompose(&row) {
            continue;
        }
        let word =
            Word { spelling: row.spelling.to_string(), kana: katakana_to_hiragana(row.kana) };
        let row_part_of_speech = row_part_of_speech(&row);
        match lexicon_parts_of_speech.entry(word) {
            Entry::Vacant(entry) => {
                words.push(entry.key().clone());
                entry.insert(row_part_of_speech);
            }
            Entry::Occupied(mut entry) => {
                entry.insert(merge_parts_of_speech(*entry.get(), row_part_of_speech));
            }
        }
    }
    let parts_of_speech = lexicon_parts_of_speech
        .into_iter()
        .filter_map(|(word, lexicon_part_of_speech)| {
            part_of_speech_from_lexicon(lexicon_part_of_speech)
                .map(|part_of_speech| (word, part_of_speech))
        })
        .collect();
    Ok((words, parts_of_speech))
}

/// Reads the fields of a record of the lexicon that decomposition needs.
fn read_row(record: &csv::StringRecord, line_number: usize) -> Result<Row<'_>> {
    if record.len() != FIELD_COUNT {
        return Err(Error::at_line(
            line_number,
            format!("expected {FIELD_COUNT} fields, found {}", record.len()),
        ));
    }
    Ok(Row {
        pos1: &record[4],
        pos2: &record[5],
        conjugation_type: &record[8],
        conjugation_form: &record[9],
        spelling: &record[12],
        initial_form: &record[18],
        final_form: &record[20],
        kana: &record[24],
        modern_kana: &record[26],
    })
}

/// Whether a row is a word to decompose. Excludes proper nouns, inflected forms,
/// rows for rendaku and other sound changes, and spellings in historical kana usage.
fn is_word_to_decompose(row: &Row) -> bool {
    row.pos2 != "固有名詞"
        && matches!(row.conjugation_form, "*" | "終止形-一般" | "ク語法")
        && matches!(row.initial_form, "*" | "基本形")
        && matches!(row.final_form, "*" | "基本形")
        && row.kana == row.modern_kana
}

/// The part of speech of a row. Verb-like and adjective-like suffixes (兼ねる, 易い)
/// count as verbs and adjectives.
fn row_part_of_speech(row: &Row) -> LexiconPartOfSpeech {
    let is_verb = (row.pos1 == "動詞" && row.conjugation_form == "終止形-一般")
        || (row.pos1 == "接尾辞" && row.pos2 == "動詞的");
    let is_adjective = (row.pos1 == "形容詞"
        && row.conjugation_type == "形容詞"
        && row.conjugation_form == "終止形-一般")
        || (row.pos1 == "接尾辞" && row.pos2 == "形容詞的");
    if is_verb {
        if row.conjugation_type.starts_with("上一段") || row.conjugation_type.starts_with("下一段")
        {
            LexiconPartOfSpeech::IchidanVerb
        } else if row.conjugation_type.starts_with("五段") {
            LexiconPartOfSpeech::GodanVerb
        } else {
            LexiconPartOfSpeech::OtherVerb
        }
    } else if is_adjective {
        LexiconPartOfSpeech::Adjective
    } else {
        LexiconPartOfSpeech::Other
    }
}

/// The part of speech of a word from the parts of speech of two sets of its rows.
/// Other verbs give way to ichidan and godan verbs (e.g. 言う also has a classical 四段 row),
/// and are the same as other parts of speech otherwise. The order rows are merged in
/// doesn't matter.
fn merge_parts_of_speech(
    first: LexiconPartOfSpeech,
    second: LexiconPartOfSpeech,
) -> LexiconPartOfSpeech {
    match (first, second) {
        (first, second) if first == second => first,
        (
            LexiconPartOfSpeech::OtherVerb,
            verb @ (LexiconPartOfSpeech::IchidanVerb | LexiconPartOfSpeech::GodanVerb),
        )
        | (
            verb @ (LexiconPartOfSpeech::IchidanVerb | LexiconPartOfSpeech::GodanVerb),
            LexiconPartOfSpeech::OtherVerb,
        ) => verb,
        (LexiconPartOfSpeech::OtherVerb, LexiconPartOfSpeech::Other)
        | (LexiconPartOfSpeech::Other, LexiconPartOfSpeech::OtherVerb) => {
            LexiconPartOfSpeech::Other
        }
        _ => LexiconPartOfSpeech::Conflicting,
    }
}

/// The part of speech of a word from the part of speech of all its rows, or `None` for
/// parts of speech without special handling, including other verbs, and for conflicting rows.
fn part_of_speech_from_lexicon(
    lexicon_part_of_speech: LexiconPartOfSpeech,
) -> Option<PartOfSpeech> {
    match lexicon_part_of_speech {
        LexiconPartOfSpeech::IchidanVerb => Some(PartOfSpeech::IchidanVerb),
        LexiconPartOfSpeech::GodanVerb => Some(PartOfSpeech::GodanVerb),
        LexiconPartOfSpeech::Adjective => Some(PartOfSpeech::Adjective),
        LexiconPartOfSpeech::OtherVerb
        | LexiconPartOfSpeech::Other
        | LexiconPartOfSpeech::Conflicting => None,
    }
}

//! Classification and conversion of Japanese characters.

use unicode_script::{Script, UnicodeScript};

/// Whether a character is a kanji. The iteration mark 々 is in the Han script,
/// but isn't a kanji.
pub fn is_kanji(c: char) -> bool {
    c.script() == Script::Han && c != '々'
}

pub fn has_kanji(text: &str) -> bool {
    text.chars().any(is_kanji)
}

pub fn is_hiragana(c: char) -> bool {
    c.script() == Script::Hiragana
}

/// Whether a character is katakana, including the long vowel mark ー. ヵ and ヶ
/// aren't counted: in words like 一ヵ月 and 一ヶ月 they're abbreviations of 箇,
/// not kana.
pub fn is_katakana(c: char) -> bool {
    (c.script() == Script::Katakana || c == 'ー') && !['ヵ', 'ヶ'].contains(&c)
}

/// Whether a character can be part of a Japanese word: a kanji, kana, or one
/// of 々, ヵ and ヶ, which count as neither.
pub fn is_japanese(c: char) -> bool {
    is_kanji(c) || is_hiragana(c) || is_katakana(c) || ['々', 'ヵ', 'ヶ'].contains(&c)
}

/// Converts katakana to hiragana, leaving other characters unchanged.
pub fn katakana_to_hiragana(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'ァ'..='ヴ' => char::from_u32(c as u32 - 0x60).unwrap(),
            c => c,
        })
        .collect()
}

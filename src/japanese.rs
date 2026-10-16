//! Classification and conversion of Japanese characters.

use unicode_script::{Script, UnicodeScript};

/// Whether a character is a kanji. Excludes the kanji iteration mark 々.
pub fn is_kanji(c: char) -> bool {
    c.script() == Script::Han && c != '々'
}

/// Whether a string contains a kanji.
pub fn has_kanji(text: &str) -> bool {
    text.chars().any(is_kanji)
}

/// Whether a character is hiragana.
/// Excludes ゖ and ゕ, as they are virtually unused.
pub fn is_hiragana(c: char) -> bool {
    c.script() == Script::Hiragana && !matches!(c, 'ゕ' | 'ゖ')
}

/// Whether a character is katakana, including the long vowel mark ー.
/// Excludes ヶ and ヵ, as semantically they are abbreviations of 箇, not kana.
pub fn is_katakana(c: char) -> bool {
    (c.script() == Script::Katakana || c == 'ー') && !matches!(c, 'ヵ' | 'ヶ')
}

/// Whether a character can be part of a Japanese word: a kanji or kana.
/// Includes the characters excluded by other functions: 々, ヶ, ヵ, ゖ, ゕ.
pub fn is_japanese(c: char) -> bool {
    [Script::Han, Script::Hiragana, Script::Katakana].contains(&c.script()) || c == 'ー'
}

/// Converts katakana to hiragana, leaving other characters unchanged.
/// Does not convert ヶ and ヵ.
pub fn katakana_to_hiragana(text: &str) -> String {
    text.chars()
        // The katakana unicode range is exactly 0x60 after the hiragana range.
        .map(|c| match c {
            'ァ'..='ヴ' => char::from_u32(c as u32 - 0x60).unwrap(),
            c => c,
        })
        .collect()
}

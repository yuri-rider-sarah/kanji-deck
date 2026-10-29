//! Classification and conversion of Japanese characters.

/// Whether a character is a kanji: a character from CJK Unified Ideographs,
/// its extensions A and B, or one of the twelve unified ideographs
/// in the CJK Compatibility Ideographs block. Excludes the kanji iteration mark 々.
pub fn is_kanji(c: char) -> bool {
    matches!(c,
        // CJK Unified Ideographs.
        '\u{4E00}'..='\u{9FFF}'
        // Extension A.
        | '\u{3400}'..='\u{4DBF}'
        // Extension B.
        | '\u{20000}'..='\u{2A6DF}'
        // The unified ideographs in CJK Compatibility Ideographs. The rest of the block
        // are duplicates of other kanji, kept for compatibility with older encodings.
        | '﨎' | '﨏' | '﨑' | '﨓' | '﨔' | '﨟' | '﨡' | '﨣' | '﨤' | '﨧' | '﨨' | '﨩'
    )
}

/// Whether a string contains a kanji.
pub fn has_kanji(text: &str) -> bool {
    text.chars().any(is_kanji)
}

/// Whether a character is hiragana, from ぁ to ゔ.
/// Excludes ゕ and ゖ, as they are virtually unused.
pub fn is_hiragana(c: char) -> bool {
    matches!(c, 'ぁ'..='ゔ')
}

/// Whether a character is katakana, from ァ to ヴ, or the long vowel mark ー.
/// Excludes ヵ and ヶ, as semantically they are abbreviations of 箇, not kana.
pub fn is_katakana(c: char) -> bool {
    matches!(c, 'ァ'..='ヴ' | 'ー')
}

/// Whether a character can be part of a Japanese word: a kanji, kana,
/// or one of 々, ヵ and ヶ, which are excluded by the other functions.
pub fn is_japanese(c: char) -> bool {
    is_kanji(c) || is_hiragana(c) || is_katakana(c) || matches!(c, '々' | 'ヵ' | 'ヶ')
}

/// Converts a katakana character to hiragana, leaving other characters unchanged.
/// Does not convert ヶ and ヵ.
pub fn katakana_to_hiragana(c: char) -> char {
    match c {
        // The katakana unicode range is exactly 0x60 after the hiragana range.
        'ァ'..='ヴ' => char::from_u32(c as u32 - 0x60).unwrap(),
        c => c,
    }
}

/// Converts katakana in a string to hiragana, leaving other characters unchanged.
/// Does not convert ヶ and ヵ.
pub fn katakana_to_hiragana_in_str(text: &str) -> String {
    text.chars().map(katakana_to_hiragana).collect()
}

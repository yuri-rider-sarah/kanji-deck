//! Writes entries back out in the dictionary format.

use super::*;

/// Formats a line indented by the given number of levels of 4 spaces.
macro_rules! indented {
    ($level:expr, $($arg:tt)*) => {
        format!("{}{}", " ".repeat(4 * $level), format_args!($($arg)*))
    };
}

pub fn serialize(entries: &[Entry]) -> String {
    let mut lines = Vec::new();
    for entry in entries {
        let head = match &entry.key {
            EntryKey::Kanji(kanji) => format!("kanji {kanji}"),
            EntryKey::Word(headword) => format!("word {headword}"),
        };
        serialize_parts(&mut lines, &head, &entry.parts);
    }
    lines.join("\n") + "\n"
}

/// Adds the lines of each part of an entry, starting with `head` and a part
/// number if there's more than one part.
fn serialize_parts(lines: &mut Vec<String>, head: &str, parts: &[EntryPart]) {
    for (i, part) in parts.iter().enumerate() {
        if parts.len() > 1 {
            lines.push(format!("{head} part:{}", i + 1));
        } else {
            lines.push(head.to_string());
        }
        for reading_type in ReadingType::ALL {
            for group in &part[reading_type] {
                lines.push(indented!(1, "{}-group", reading_type));
                for reading in group {
                    serialize_reading(lines, reading);
                }
            }
        }
    }
}

fn serialize_reading(lines: &mut Vec<String>, reading: &Reading) {
    let mut head = String::from("reading");
    if reading.core {
        head += " #core";
    }
    if reading.regular {
        head += " #regular";
    }
    match reading.origin {
        Some(ReadingOrigin::Asian) => head += " #asian",
        Some(ReadingOrigin::European) => head += " #european",
        None => {}
    }
    lines.push(indented!(
        2,
        "{head} | {} | {} || {}",
        serialize_spelling(&reading.spelling),
        serialize_kana(&reading.kana),
        reading.definition
    ));
    for variant in &reading.variants {
        lines.push(indented!(3, "variant {}", serialize_freq_word(variant)));
    }
}

fn serialize_freq_word(fw: &FreqWord) -> String {
    let freq = match fw.freq {
        Freq::Common => "common",
        Freq::Rare => "rare",
    };
    format!("{freq}:{}", fw.word)
}

fn serialize_spelling(spelling: &Spelling) -> String {
    match spelling {
        Spelling::Primary(main) => format!("pri-spell {main}"),
        Spelling::PrimaryKanji { main, described } => {
            format!("pri-kanji-spell {main} {}", serialize_freq_word(described))
        }
        Spelling::Secondary { main, described } => {
            format!("sec-spell {main} {}", serialize_freq_word(described))
        }
        Spelling::SecondaryKanji { main, main_kanji, described } => {
            format!(
                "sec-kanji-spell {main} {} {}",
                serialize_freq_word(main_kanji),
                serialize_freq_word(described)
            )
        }
    }
}

fn serialize_kana(kana: &Kana) -> String {
    match kana {
        Kana::Split { pre, mid, post } => format!("{pre}*{mid}*{post}"),
        Kana::Unsplit(kana) => kana.clone(),
    }
}

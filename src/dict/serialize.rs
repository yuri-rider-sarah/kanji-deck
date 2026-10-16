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
        match entry {
            Entry::Kanji(entry) => {
                for (i, part) in entry.parts.iter().enumerate() {
                    if entry.parts.len() > 1 {
                        lines.push(format!("kanji {} part:{}", entry.kanji, i + 1));
                    } else {
                        lines.push(format!("kanji {}", entry.kanji));
                    }
                    let heads = [(ReadingType::On, "on-group"), (ReadingType::Kun, "kun-group"), (ReadingType::Combined, "combined-group")];
                    for (reading_type, head) in heads {
                        for group in &part[reading_type] {
                            lines.push(indented!(1, "{head}"));
                            for reading in group {
                                serialize_reading(&mut lines, reading);
                            }
                        }
                    }
                }
            }
            Entry::Word(entry) => {
                lines.push(format!("word {}", entry.headword));
                for group in &entry.readings {
                    lines.push(indented!(1, "group"));
                    for reading in group {
                        serialize_reading(&mut lines, reading);
                    }
                }
            }
        }
    }
    lines.join("\n") + "\n"
}

fn serialize_reading(lines: &mut Vec<String>, reading: &Reading) {
    let mut head = String::from("reading");
    if reading.core {
        head += " #core";
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
        Spelling::PrimaryKanji { main, described } => format!("pri-kanji-spell {main} {}", serialize_freq_word(described)),
        Spelling::Secondary { main, described } => format!("sec-spell {main} {}", serialize_freq_word(described)),
        Spelling::SecondaryKanji { main, main_kanji, described } => {
            format!("sec-kanji-spell {main} {} {}", serialize_freq_word(main_kanji), serialize_freq_word(described))
        }
    }
}

fn serialize_kana(kana: &Kana) -> String {
    match kana {
        Kana::Split { pre, mid, post } => format!("{pre}*{mid}*{post}"),
        Kana::Unsplit(kana) => kana.clone(),
    }
}

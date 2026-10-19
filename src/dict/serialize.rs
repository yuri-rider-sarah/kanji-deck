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
        serialize_entry(&mut lines, entry);
    }
    lines.join("\n") + "\n"
}

/// Adds the lines of each part of an entry.
fn serialize_entry(lines: &mut Vec<String>, entry: &Entry) {
    match entry {
        Entry::Kanji(entry) => {
            let head = format!("kanji {}", entry.kanji);
            for (i, part) in entry.parts.iter().enumerate() {
                lines.push(serialize_part_head(&head, i, entry.parts.len()));
                serialize_part(lines, part);
            }
        }
        Entry::Word(entry) => {
            lines.push(format!("word {}", entry.headword));
            if !entry.introduced_kanji.is_empty() {
                let introduced_kanji: String = entry.introduced_kanji.iter().collect();
                lines.push(indented!(1, "introduces {introduced_kanji}"));
            }
            serialize_part(lines, &entry.part);
        }
    }
}

/// The first line of the part at `index` of an entry starting with `head`,
/// with a part number if there's more than one part.
fn serialize_part_head(head: &str, index: usize, part_count: usize) -> String {
    if part_count > 1 { format!("{head} part:{}", index + 1) } else { head.to_string() }
}

fn serialize_part<K: SerializeKana>(lines: &mut Vec<String>, part: &EntryPart<K>) {
    for reading_type in ReadingType::ALL {
        for group in &part[reading_type] {
            lines.push(indented!(1, "{}-group", reading_type));
            for reading in group {
                serialize_reading(lines, reading);
            }
        }
    }
}

fn serialize_reading<K: SerializeKana>(lines: &mut Vec<String>, reading: &Reading<K>) {
    let mut head = String::from("reading");
    if reading.core {
        head += " #core";
    }
    if reading.regular {
        head += " #regular";
    }
    lines.push(indented!(
        2,
        "{head} | {} | {} || {}",
        serialize_spelling(&reading.spelling),
        reading.kana.serialize_kana(),
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

/// Kana of a reading, serialized differently for kanji and word entries.
trait SerializeKana {
    fn serialize_kana(&self) -> String;
}

impl SerializeKana for SplitKana {
    fn serialize_kana(&self) -> String {
        format!("{}*{}*{}", self.pre, self.mid, self.post)
    }
}

impl SerializeKana for UnsplitKana {
    fn serialize_kana(&self) -> String {
        self.0.clone()
    }
}

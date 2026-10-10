//! Writes entries back out in the dictionary format.

use super::*;

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
                    let sections = [("on-group", &part.on), ("kun-group", &part.kun), ("combined-group", &part.combined)];
                    for (head, groups) in sections {
                        for group in groups {
                            lines.push(format!("    {head}"));
                            for reading in group {
                                write_reading(&mut lines, reading);
                            }
                        }
                    }
                }
            }
            Entry::Word(entry) => {
                lines.push(format!("word {}", entry.headword));
                for group in &entry.readings {
                    lines.push("    group".into());
                    for reading in group {
                        write_reading(&mut lines, reading);
                    }
                }
            }
        }
    }
    lines.join("\n") + "\n"
}

fn write_reading(lines: &mut Vec<String>, reading: &Reading) {
    let mut head = String::from("reading");
    if reading.primary {
        head += " #core";
    }
    for attr in &reading.attrs {
        head += match attr {
            Attr::Asian => " #asian",
            Attr::European => " #european",
        };
    }
    lines.push(format!(
        "        {head} | {} | {} || {}",
        spelling(&reading.spelling),
        kana(&reading.kana),
        reading.definition
    ));
    for variant in &reading.variants {
        lines.push(format!("            variant {}", freq_word(variant)));
    }
}

fn freq_word(fw: &FreqWord) -> String {
    let freq = match fw.freq {
        Freq::Common => "common",
        Freq::Rare => "rare",
    };
    format!("{freq}:{}", fw.word)
}

fn spelling(spelling: &Spelling) -> String {
    match spelling {
        Spelling::Primary(main) => format!("pri-spell {main}"),
        Spelling::PrimaryKanji { main, kanji } => format!("pri-kanji-spell {main} {}", freq_word(kanji)),
        Spelling::Secondary { main, described } => format!("sec-spell {main} {}", freq_word(described)),
        Spelling::SecondaryKanji { main, kanji, described } => {
            format!("sec-kanji-spell {main} {} {}", freq_word(kanji), freq_word(described))
        }
    }
}

fn kana(kana: &Kana) -> String {
    match kana {
        Kana::Split { pre, mid, post } => format!("{pre}*{mid}*{post}"),
        Kana::Unsplit(kana) => kana.clone(),
    }
}

//! Generates JSON for Anki deck which is passed to generate.py.
//! Each note contains the kanji, part number (if present), and the content,
//! which is HTML generated from dictionary entries.

use crate::dict::*;

/// Escapes special characters in a string so it can be inserted into HTML.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A word with the kanji highlighted, or the whole word if there's no kanji.
fn html_from_word(word: &str, kanji: Option<char>) -> String {
    match kanji {
        None => format!("<span class=\"kanji\">{}</span>", escape_html(word)),
        Some(kanji) => {
            let parts: Vec<String> = word.split(kanji).map(escape_html).collect();
            parts.join(&format!("<span class=\"kanji\">{kanji}</span>"))
        }
    }
}

fn html_from_freq_word(fw: &FreqWord, kanji: Option<char>) -> String {
    match fw.freq {
        Freq::Common => html_from_word(&fw.word, kanji),
        Freq::Rare => format!("<span class=\"rare\">{}</span>", html_from_word(&fw.word, kanji)),
    }
}

/// The spellings shown in addition to the main one.
fn html_from_other_spellings(spelling: &Spelling, kanji: Option<char>) -> String {
    match spelling {
        Spelling::Primary(_) => String::new(),
        Spelling::PrimaryKanji(_, described) | Spelling::Secondary(_, described) => {
            html_from_freq_word(described, kanji)
        }
        Spelling::SecondaryKanji(_, main_kanji, described) => {
            format!("{} / {}", html_from_freq_word(described, kanji), html_from_freq_word(main_kanji, kanji))
        }
    }
}

fn html_from_kana(kana: &Kana) -> String {
    match kana {
        Kana::Split { pre, mid, post } => {
            format!("{}<span class=\"kana\">{}</span>{}", escape_html(pre), escape_html(mid), escape_html(post))
        }
        Kana::Unsplit(kana) => format!("<span class=\"kana\">{}</span>", escape_html(kana)),
    }
}

fn html_from_variants(variants: &[FreqWord], under_primary: bool, kanji: Option<char>) -> String {
    let text = variants.iter().map(|v| html_from_freq_word(v, kanji)).collect::<Vec<_>>().join(", ");
    if under_primary {
        format!("<tr class=\"variant\"><td></td><td>~ {text}</td><td></td><td></td></tr>\n")
    } else {
        format!("<tr class=\"variant\"><td>~ {text}</td><td></td><td></td><td></td></tr>\n")
    }
}

/// A table row for a reading, and one for its variants if it has any. `main`
/// is set for the first reading in a group.
fn html_from_reading(reading: &Reading, main: bool, kanji: Option<char>) -> String {
    let mut classes = Vec::new();
    match reading.origin {
        Some(ReadingOrigin::Asian) => classes.push("asian"),
        Some(ReadingOrigin::European) => classes.push("european"),
        None => {}
    }
    if main {
        classes.push("main");
    }
    if reading.core {
        classes.push("core");
    }
    if reading.spelling.described_freq() == Freq::Rare {
        classes.push("rare-spelling");
    }
    let class_attr = if classes.is_empty() { String::new() } else { format!(" class=\"{}\"", classes.join(" ")) };
    let mut html = format!(
        "<tr{class_attr}><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
        html_from_other_spellings(&reading.spelling, kanji),
        html_from_word(reading.spelling.main(), kanji),
        html_from_kana(&reading.kana),
        escape_html(&reading.definition)
    );
    if !reading.variants.is_empty() {
        let under_primary = matches!(reading.spelling, Spelling::Primary(_));
        html += &html_from_variants(&reading.variants, under_primary, kanji);
    }
    html
}

/// A table with one `<tbody>` per non-empty section, named by its class.
fn html_from_note(sections: &[(&str, &[ReadingGroup])], kanji: Option<char>) -> String {
    let mut html = String::from("<table>\n");
    for (class, groups) in sections.iter().filter(|(_, groups)| !groups.is_empty()) {
        html += &format!("<tbody class=\"{class}\">\n");
        for group in *groups {
            for (i, reading) in group.iter().enumerate() {
                html += &html_from_reading(reading, i == 0, kanji);
            }
        }
        html += "</tbody>\n";
    }
    html + "</table>"
}

fn json_from_note(kanji: &str, part: &str, content: &str) -> String {
    let s = |text: &str| serde_json::to_string(text).unwrap();
    format!("{{\"kanji\":{},\"part\":{},\"content\":{}}}", s(kanji), s(part), s(content))
}

/// JSON with one object per card, in the format generate.py expects.
pub fn json_from_entries(entries: &[Entry]) -> String {
    let mut notes = Vec::new();
    for entry in entries {
        match entry {
            Entry::Kanji(entry) => {
                for (i, part) in entry.parts.iter().enumerate() {
                    // Parts are numbered from 1, but only if there's more than one.
                    let number = if entry.parts.len() > 1 { (i + 1).to_string() } else { String::new() };
                    let classes = [(ReadingType::On, "on"), (ReadingType::Kun, "kun"), (ReadingType::Combined, "combined")];
                    let sections = classes.map(|(reading_type, class)| (class, &part[reading_type][..]));
                    notes.push(json_from_note(&entry.kanji.to_string(), &number, &html_from_note(&sections, Some(entry.kanji))));
                }
            }
            Entry::Word(entry) => {
                let content = html_from_note(&[("word", &entry.readings[..])], None);
                notes.push(json_from_note(&entry.headword, "", &content));
            }
        }
    }
    format!("[{}]", notes.join(","))
}

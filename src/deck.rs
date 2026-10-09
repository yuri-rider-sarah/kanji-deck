//! Card contents for the Anki deck, passed to generate.py as JSON.

use crate::dict::*;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A word with the kanji highlighted, or the whole word if there's no kanji.
fn word_html(word: &str, kanji: &str) -> String {
    if kanji.is_empty() {
        format!("<span class=\"kanji\">{}</span>", escape(word))
    } else {
        let parts: Vec<String> = word.split(kanji).map(escape).collect();
        parts.join(&format!("<span class=\"kanji\">{kanji}</span>"))
    }
}

fn freq_word_html(fw: &FreqWord, kanji: &str) -> String {
    match fw.freq {
        Freq::Common => word_html(&fw.word, kanji),
        Freq::Rare => format!("<span class=\"rare\">{}</span>", word_html(&fw.word, kanji)),
    }
}

/// The spellings shown in addition to the main one.
fn other_spelling_html(spelling: &Spelling, kanji: &str) -> String {
    match spelling {
        Spelling::Primary(_) => String::new(),
        Spelling::PrimaryKanji { kanji: word, .. } | Spelling::Secondary { described: word, .. } => {
            freq_word_html(word, kanji)
        }
        Spelling::SecondaryKanji { kanji: main_kanji, described, .. } => {
            format!("{} / {}", freq_word_html(described, kanji), freq_word_html(main_kanji, kanji))
        }
    }
}

fn kana_html(kana: &Kana) -> String {
    match kana {
        Kana::Split { pre, mid, post } => {
            format!("{}<span class=\"kana\">{}</span>{}", escape(pre), escape(mid), escape(post))
        }
        Kana::Unsplit(kana) => format!("<span class=\"kana\">{}</span>", escape(kana)),
    }
}

fn variants_html(variants: &[FreqWord], under_primary: bool, kanji: &str) -> String {
    let text = variants.iter().map(|v| freq_word_html(v, kanji)).collect::<Vec<_>>().join(", ");
    if under_primary {
        format!("<tr class=\"variant\"><td></td><td>~ {text}</td><td></td><td></td></tr>\n")
    } else {
        format!("<tr class=\"variant\"><td>~ {text}</td><td></td><td></td><td></td></tr>\n")
    }
}

/// A table row for a reading, and one for its variants if it has any. `main`
/// is set for the first reading in a group.
fn reading_html(reading: &Reading, main: bool, kanji: &str) -> String {
    let mut classes = Vec::new();
    for attr in &reading.attrs {
        classes.push(match attr {
            Attr::Asian => "asian",
            Attr::European => "european",
        });
    }
    if main {
        classes.push("main");
    }
    if reading.primary {
        classes.push("primary");
    }
    let class_attr = if classes.is_empty() { String::new() } else { format!(" class=\"{}\"", classes.join(" ")) };
    let mut html = format!(
        "<tr{class_attr}><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
        other_spelling_html(&reading.spelling, kanji),
        word_html(reading.spelling.main(), kanji),
        kana_html(&reading.kana),
        escape(&reading.definition)
    );
    if !reading.variants.is_empty() {
        let under_primary = matches!(reading.spelling, Spelling::Primary(_));
        html += &variants_html(&reading.variants, under_primary, kanji);
    }
    html
}

/// A table with one `<tbody>` per non-empty section, named by its class.
fn table(sections: &[(&str, &[Group])], kanji: &str) -> String {
    let mut html = String::from("<table>\n");
    for (class, groups) in sections.iter().filter(|(_, groups)| !groups.is_empty()) {
        html += &format!("<tbody class=\"{class}\">\n");
        for group in *groups {
            for (i, reading) in group.iter().enumerate() {
                html += &reading_html(reading, i == 0, kanji);
            }
        }
        html += "</tbody>\n";
    }
    html + "</table>"
}

fn note_json(kanji: &str, part: &str, content: &str) -> String {
    let s = |text: &str| serde_json::to_string(text).unwrap();
    format!("{{\"kanji\":{},\"part\":{},\"content\":{}}}", s(kanji), s(part), s(content))
}

/// JSON with one object per card, in the format generate.py expects.
pub fn json(entries: &[Entry]) -> String {
    let mut notes = Vec::new();
    for entry in entries {
        match entry {
            Entry::Kanji(entry) => {
                for part in &entry.parts {
                    let sections = [("on", &part.on[..]), ("kun", &part.kun[..]), ("combined", &part.combined[..])];
                    notes.push(note_json(&entry.kanji, &part.name, &table(&sections, &entry.kanji)));
                }
            }
            Entry::Word(entry) => {
                let content = table(&[("word", &entry.readings[..])], "");
                notes.push(note_json(entry.headword(), "", &content));
            }
        }
    }
    format!("[{}]", notes.join(","))
}

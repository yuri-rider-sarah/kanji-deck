//! Generates JSON for Anki deck which is passed to generate.py.
//! Each note contains the kanji, part number (if present), and the content,
//! which is HTML generated from dictionary entries.

use crate::dict::*;

/// Collects HTML, escaping text and closing tags automatically.
#[derive(Default)]
struct HtmlWriter {
    html: String,
}

impl HtmlWriter {
    fn new() -> Self {
        Self::default()
    }

    /// Appends text, escaping special characters.
    fn add_text(&mut self, text: &str) {
        self.html += &escape_html(text);
    }

    /// Appends a tag with the given classes, which are left out when empty,
    /// and contents written by `f`.
    fn add_tag(&mut self, name: &str, classes: &[&str], f: impl FnOnce(&mut Self)) {
        self.html += &format!("<{name}");
        if !classes.is_empty() {
            self.html += &format!(" class=\"{}\"", escape_html(&classes.join(" ")));
        }
        self.html += ">";
        f(self);
        self.html += &format!("</{name}>");
    }

    fn add_line_break(&mut self) {
        self.html += "\n";
    }

    /// The HTML written so far.
    fn finish(self) -> String {
        self.html
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A word with the kanji highlighted, or the whole word if there's no kanji.
fn write_word(w: &mut HtmlWriter, word: &str, kanji: Option<char>) {
    let Some(kanji) = kanji else {
        return w.add_tag("span", &["kanji"], |w| w.add_text(word));
    };
    for (i, piece) in word.split(kanji).enumerate() {
        if i > 0 {
            w.add_tag("span", &["kanji"], |w| w.add_text(&kanji.to_string()));
        }
        w.add_text(piece);
    }
}

fn write_freq_word(w: &mut HtmlWriter, fw: &FreqWord, kanji: Option<char>) {
    match fw.freq {
        Freq::Common => write_word(w, &fw.word, kanji),
        Freq::Rare => w.add_tag("span", &["rare"], |w| write_word(w, &fw.word, kanji)),
    }
}

/// The spellings shown in addition to the main one.
fn write_other_spellings(w: &mut HtmlWriter, spelling: &Spelling, kanji: Option<char>) {
    match spelling {
        Spelling::Primary(_) => {}
        Spelling::PrimaryKanji(_, described) | Spelling::Secondary(_, described) => {
            write_freq_word(w, described, kanji);
        }
        Spelling::SecondaryKanji(_, main_kanji, described) => {
            write_freq_word(w, described, kanji);
            w.add_text(" / ");
            write_freq_word(w, main_kanji, kanji);
        }
    }
}

fn write_kana(w: &mut HtmlWriter, kana: &Kana) {
    match kana {
        Kana::Split { pre, mid, post } => {
            w.add_text(pre);
            w.add_tag("span", &["kana"], |w| w.add_text(mid));
            w.add_text(post);
        }
        Kana::Unsplit(kana) => w.add_tag("span", &["kana"], |w| w.add_text(kana)),
    }
}

/// A table row for the variants of a reading. They go in the second column if
/// the first one is empty.
fn write_variants(w: &mut HtmlWriter, variants: &[FreqWord], under_primary: bool, kanji: Option<char>) {
    let write_text = |w: &mut HtmlWriter| {
        w.add_text("~ ");
        for (i, variant) in variants.iter().enumerate() {
            if i > 0 {
                w.add_text(", ");
            }
            write_freq_word(w, variant, kanji);
        }
    };
    w.add_tag("tr", &["variant"], |w| {
        if under_primary {
            w.add_tag("td", &[], |_| {});
            w.add_tag("td", &[], write_text);
        } else {
            w.add_tag("td", &[], write_text);
            w.add_tag("td", &[], |_| {});
        }
        w.add_tag("td", &[], |_| {});
        w.add_tag("td", &[], |_| {});
    });
    w.add_line_break();
}

/// A table row for a reading, and one for its variants if it has any.
/// `group-top` is set for the first reading in a group.
fn write_reading(w: &mut HtmlWriter, reading: &Reading, group_top: bool, kanji: Option<char>) {
    let mut classes = Vec::new();
    match reading.origin {
        Some(ReadingOrigin::Asian) => classes.push("asian"),
        Some(ReadingOrigin::European) => classes.push("european"),
        None => {}
    }
    if group_top {
        classes.push("group-top");
    }
    if reading.core {
        classes.push("core");
    }
    if reading.spelling.described_freq() == Freq::Rare {
        classes.push("rare-spelling");
    }
    w.add_tag("tr", &classes, |w| {
        w.add_tag("td", &[], |w| write_other_spellings(w, &reading.spelling, kanji));
        w.add_tag("td", &[], |w| write_word(w, reading.spelling.main(), kanji));
        w.add_tag("td", &[], |w| write_kana(w, &reading.kana));
        w.add_tag("td", &[], |w| w.add_text(&reading.definition));
    });
    w.add_line_break();
    if !reading.variants.is_empty() {
        let under_primary = matches!(reading.spelling, Spelling::Primary(_));
        write_variants(w, &reading.variants, under_primary, kanji);
    }
}

/// A table with one `<tbody>` per non-empty section, named by its class.
fn html_from_note(sections: &[(&str, &[ReadingGroup])], kanji: Option<char>) -> String {
    let mut w = HtmlWriter::new();
    w.add_tag("table", &[], |w| {
        w.add_line_break();
        for (class, groups) in sections.iter().filter(|(_, groups)| !groups.is_empty()) {
            w.add_tag("tbody", &[class], |w| {
                w.add_line_break();
                for group in *groups {
                    for (i, reading) in group.iter().enumerate() {
                        write_reading(w, reading, i == 0, kanji);
                    }
                }
            });
            w.add_line_break();
        }
    });
    w.finish()
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

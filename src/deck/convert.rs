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
    match kanji {
        Some(kanji) => {
            for (i, piece) in word.split(kanji).enumerate() {
                // Insert <span> with the kanji highlighted between the other parts of the word.
                if i > 0 {
                    w.add_tag("span", &["kanji"], |w| w.add_text(&kanji.to_string()));
                }
                w.add_text(piece);
            }
        }
        None => w.add_tag("span", &["kanji"], |w| w.add_text(word)),
    }
}

/// A word with the kanji highlighted, in a smaller font if it's rare.
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
        Spelling::PrimaryKanji { described, .. } | Spelling::Secondary { described, .. } => {
            write_freq_word(w, described, kanji);
        }
        Spelling::SecondaryKanji { main_kanji, described, .. } => {
            write_freq_word(w, described, kanji);
            w.add_text(" / ");
            write_freq_word(w, main_kanji, kanji);
        }
    }
}

/// Kana of a reading, with the part that belongs to the kanji or word
/// highlighted.
trait WriteKana {
    fn write_kana(&self, w: &mut HtmlWriter);
}

impl WriteKana for SplitKana {
    fn write_kana(&self, w: &mut HtmlWriter) {
        w.add_text(&self.pre);
        w.add_tag("span", &["kana"], |w| w.add_text(&self.mid));
        w.add_text(&self.post);
    }
}

impl WriteKana for UnsplitKana {
    fn write_kana(&self, w: &mut HtmlWriter) {
        w.add_tag("span", &["kana"], |w| w.add_text(&self.0));
    }
}

/// A table row for the variants of a reading. They go in the same column as
/// the described reading.
fn write_variants(
    w: &mut HtmlWriter,
    variants: &[FreqWord],
    under_primary: bool,
    kanji: Option<char>,
) {
    let write_text = |w: &mut HtmlWriter| {
        w.add_text("~ ");
        for (i, variant) in variants.iter().enumerate() {
            // Separate variants with commas.
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
fn write_reading<K: WriteKana>(
    w: &mut HtmlWriter,
    reading: &Reading<K>,
    group_top: bool,
    kanji: Option<char>,
) {
    let mut classes = Vec::new();
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
        w.add_tag("td", &[], |w| reading.kana.write_kana(w));
        w.add_tag("td", &[], |w| w.add_text(&reading.definition));
    });
    w.add_line_break();
    if !reading.variants.is_empty() {
        let under_primary = matches!(reading.spelling, Spelling::Primary(_));
        write_variants(w, &reading.variants, under_primary, kanji);
    }
}

/// A table with one `<tbody>` per non-empty section, named by its class.
fn html_from_note<K: WriteKana>(part: &EntryPart<K>, kanji: Option<char>) -> String {
    let mut w = HtmlWriter::new();
    w.add_tag("table", &[], |w| {
        w.add_line_break();
        for reading_type in ReadingType::ALL {
            if part[reading_type].is_empty() {
                continue;
            }
            w.add_tag("tbody", &[&reading_type.to_string()], |w| {
                w.add_line_break();
                for group in &part[reading_type] {
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

/// The headword of a word entry shown on its cards, with the kanji it
/// introduces underlined.
fn html_from_headword(entry: &WordEntry) -> String {
    let mut w = HtmlWriter::new();
    for c in entry.headword.chars() {
        if entry.introduced_kanji.contains(&c) {
            w.add_tag("span", &["introduced"], |w| w.add_text(&c.to_string()));
        } else {
            w.add_text(&c.to_string());
        }
    }
    w.finish()
}

/// Escapes a string so it can be put between quotes in JSON.
fn escape_json(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        match c {
            '"' => escaped += "\\\"",
            '\\' => escaped += "\\\\",
            '\n' => escaped += "\\n",
            c if c.is_control() => escaped += &format!("\\u{:04x}", c as u32),
            c => escaped.push(c),
        }
    }
    escaped
}

/// A note as JSON. `id` is the plain kanji or headword that the note's GUID
/// is made from, and `kanji` the same as HTML.
fn json_from_note(id: &str, kanji: &str, part: &str, content: &str) -> String {
    let (id, kanji) = (escape_json(id), escape_json(kanji));
    let (part, content) = (escape_json(part), escape_json(content));
    format!(
        "{{\"id\":\"{id}\",\"kanji\":\"{kanji}\",\"part\":\"{part}\",\"content\":\"{content}\"}}"
    )
}

/// JSON for the notes of an entry: one per part for kanji, and one for words.
fn json_from_entry(entry: &Entry) -> Vec<String> {
    match entry {
        Entry::Kanji(entry) => {
            let kanji = entry.kanji.to_string();
            let mut notes = Vec::new();
            for (i, part) in entry.parts.iter().enumerate() {
                // Parts are numbered from 1, but only if there's more than one.
                let number =
                    if entry.parts.len() > 1 { (i + 1).to_string() } else { String::new() };
                let content = html_from_note(part, Some(entry.kanji));
                notes.push(json_from_note(&kanji, &kanji, &number, &content));
            }
            notes
        }
        Entry::Word(entry) => {
            let content = html_from_note(&entry.part, None);
            vec![json_from_note(&entry.headword, &html_from_headword(entry), "", &content)]
        }
    }
}

/// JSON with one object per card, in the format generate.py expects.
pub fn json_from_entries(entries: &[Entry]) -> String {
    let mut notes = Vec::new();
    for entry in entries {
        notes.extend(json_from_entry(entry));
    }
    format!("[{}]", notes.join(","))
}

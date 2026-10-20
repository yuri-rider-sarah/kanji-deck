//! Writes dictionary entries as an Anki package (.apkg), in the legacy format
//! (`collection.anki2`, schema version 11) that genanki also writes.

use super::convert::{Note, notes_from_entries};
use crate::dict::Entry;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::io::{Cursor, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// The ID of the note type and deck. Never change them: Anki would import
/// the notes as a new note type, losing the review history.
const MODEL_ID: i64 = 2138030364;
const DECK_ID: i64 = 1896983062;

/// The names of the note fields. Never reorder them: Anki identifies
/// fields by their position.
const FIELD_NAMES: [&str; 5] = ["Id", "Kanji", "Part", "Content", "Ordinal"];

/// The name of the font in Anki's media folder, as used by `assets/style.css`.
const FONT_FILE_NAME: &str = "_NotoSansJP.ttf";

/// The LaTeX put before formulas: Anki's default, as the deck doesn't use LaTeX.
const LATEX_PREAMBLE: &str = r"\documentclass[12pt]{article}
\special{papersize=3in,5in}
\usepackage[utf8]{inputenc}
\usepackage{amssymb,amsmath}
\pagestyle{empty}
\setlength{\parindent}{0in}
\begin{document}
";

/// The tables and indexes of a schema version 11 collection.
const SCHEMA: &str = "
CREATE TABLE col (
    id              integer primary key,
    crt             integer not null,
    mod             integer not null,
    scm             integer not null,
    ver             integer not null,
    dty             integer not null,
    usn             integer not null,
    ls              integer not null,
    conf            text not null,
    models          text not null,
    decks           text not null,
    dconf           text not null,
    tags            text not null
);
CREATE TABLE notes (
    id              integer primary key,
    guid            text not null,
    mid             integer not null,
    mod             integer not null,
    usn             integer not null,
    tags            text not null,
    flds            text not null,
    sfld            integer not null,
    csum            integer not null,
    flags           integer not null,
    data            text not null
);
CREATE TABLE cards (
    id              integer primary key,
    nid             integer not null,
    did             integer not null,
    ord             integer not null,
    mod             integer not null,
    usn             integer not null,
    type            integer not null,
    queue           integer not null,
    due             integer not null,
    ivl             integer not null,
    factor          integer not null,
    reps            integer not null,
    lapses          integer not null,
    left            integer not null,
    odue            integer not null,
    odid            integer not null,
    flags           integer not null,
    data            text not null
);
CREATE TABLE revlog (
    id              integer primary key,
    cid             integer not null,
    usn             integer not null,
    ease            integer not null,
    ivl             integer not null,
    lastIvl         integer not null,
    factor          integer not null,
    time            integer not null,
    type            integer not null
);
CREATE TABLE graves (
    usn             integer not null,
    oid             integer not null,
    type            integer not null
);
CREATE INDEX ix_notes_usn on notes (usn);
CREATE INDEX ix_cards_usn on cards (usn);
CREATE INDEX ix_revlog_usn on revlog (usn);
CREATE INDEX ix_cards_nid on cards (nid);
CREATE INDEX ix_cards_sched on cards (did, queue, due);
CREATE INDEX ix_revlog_cid on revlog (cid);
CREATE INDEX ix_notes_csum on notes (csum);
";

/// An Anki package with the notes of the entries in study order, styled with `css`,
/// and the font. `timestamp` is used as the modification time, so Anki updates
/// notes imported earlier, and as the start of the note and card IDs.
pub fn apkg_from_entries(
    entries: &[Entry],
    css: &str,
    font: &[u8],
    timestamp: SystemTime,
) -> Vec<u8> {
    let notes = notes_from_entries(entries);
    let collection = collection_from_notes(&notes, css, timestamp);
    // Media files are stored under their index, and `media` maps indexes to file names.
    let media = json!({ "0": FONT_FILE_NAME }).to_string();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, contents) in
        [("collection.anki2", collection.as_slice()), ("media", media.as_bytes()), ("0", font)]
    {
        zip.start_file(name, options).expect("failed to add a file to the package");
        zip.write_all(contents).expect("failed to write a file to the package");
    }
    zip.finish().expect("failed to finish the package").into_inner()
}

/// An SQLite database with the deck, the note type and the notes with their cards.
fn collection_from_notes(notes: &[Note], css: &str, timestamp: SystemTime) -> Vec<u8> {
    let millis =
        timestamp.duration_since(UNIX_EPOCH).expect("system time is before 1970").as_millis();
    let millis = i64::try_from(millis).expect("system time is too far in the future");
    let seconds = millis / 1000;
    let mut db = Connection::open_in_memory().expect("failed to create an SQLite database");
    db.execute_batch(SCHEMA).expect("failed to create the collection tables");
    let transaction = db.transaction().expect("failed to start a transaction");
    insert_collection_row(&transaction, css, millis);
    for (ordinal, note) in notes.iter().enumerate() {
        // IDs are creation times in milliseconds. Each note uses three: its own and its cards'.
        let note_id = millis + 3 * ordinal as i64;
        insert_note(&transaction, note, ordinal, note_id, seconds);
    }
    transaction.commit().expect("failed to commit the transaction");
    db.serialize("main").expect("failed to serialize the collection").to_vec()
}

/// Inserts the row of the `col` table, which holds the collection settings,
/// the note type and the decks as JSON.
fn insert_collection_row(db: &Connection, css: &str, millis: i64) {
    let seconds = millis / 1000;
    let models = json!({ (MODEL_ID.to_string()): model_json(css, seconds) });
    // The package is a whole collection, and every collection has the Default deck
    // with ID 1, which Anki can't delete. No cards are in it.
    let decks = json!({
        "1": deck_json(1, "Default", seconds),
        (DECK_ID.to_string()): deck_json(DECK_ID, "Kanji", seconds),
    });
    db.execute(
        "INSERT INTO col (id, crt, mod, scm, ver, dty, usn, ls, conf, models, decks, dconf, tags)
        VALUES (1, ?1, ?2, ?2, 11, 0, 0, 0, ?3, ?4, ?5, ?6, '{}')",
        params![
            seconds,
            millis,
            collection_config_json().to_string(),
            models.to_string(),
            decks.to_string(),
            deck_options_json().to_string(),
        ],
    )
    .expect("failed to insert the collection row");
}

/// Inserts a note and its two cards, which take the two IDs after `note_id`.
/// `ordinal` is the note's position in the study order.
fn insert_note(db: &Connection, note: &Note, ordinal: usize, note_id: i64, seconds: i64) {
    let guid = guid_from_note(note);
    let part = match note.part_number {
        Some(number) => number.to_string(),
        None => String::new(),
    };
    let ordinal_text = format!("{ordinal:04}");
    let fields = [guid.as_str(), &note.kanji, &part, &note.content, &ordinal_text];
    // The checksum (csum) is left at 0, since Anki recomputes it on import.
    db.execute(
        "INSERT INTO notes (id, guid, mid, mod, usn, tags, flds, sfld, csum, flags, data)
        VALUES (?1, ?2, ?3, ?4, -1, '', ?5, ?2, 0, 0, '')",
        params![note_id, guid, MODEL_ID, seconds, fields.join("\x1f")],
    )
    .expect("failed to insert a note");
    for card_ord in 0..2 {
        // New cards are shown in the order of `due`.
        db.execute(
            "INSERT INTO cards (id, nid, did, ord, mod, usn, type, queue, due,
                ivl, factor, reps, lapses, left, odue, odid, flags, data)
            VALUES (?1, ?2, ?3, ?4, ?5, -1, 0, 0, ?6, 0, 0, 0, 0, 0, 0, 0, 0, '')",
            params![note_id + 1 + card_ord, note_id, DECK_ID, card_ord, seconds, ordinal as i64],
        )
        .expect("failed to insert a card");
    }
}

/// The GUID of a note, which Anki uses to match it with the same note
/// imported earlier: the kanji or headword, followed by the part number in
/// parentheses if there is one. Never change it.
fn guid_from_note(note: &Note) -> String {
    match note.part_number {
        Some(number) => format!("{} ({number})", note.id),
        None => note.id.clone(),
    }
}

/// The note type, with the fields and the templates of the two cards:
/// one asking for the readings of the kanji, and one asking for the kanji.
fn model_json(css: &str, seconds: i64) -> Value {
    let fields: Vec<Value> = FIELD_NAMES
        .iter()
        .enumerate()
        .map(|(ord, name)| {
            json!({
                "name": name,
                "ord": ord,
                "font": "Liberation Sans",
                "media": [],
                "rtl": false,
                "size": 20,
                "sticky": false,
            })
        })
        .collect();
    let templates = [
        template_json(
            0,
            "Card 1",
            r#"<div class="q-kanji">{{Kanji}}</div><div class="part">{{Part}}</div>"#,
            r#"{{FrontSide}}<div id="answer">{{Content}}</div>"#,
        ),
        template_json(
            1,
            "Card 2",
            r#"<div class="question">{{Content}}</div>"#,
            concat!(
                r#"<div id="answer" class="q-kanji">{{Kanji}}</div>"#,
                r#"<div class="part">{{Part}}</div>{{Content}}"#,
            ),
        ),
    ];
    json!({
        "css": css,
        "did": DECK_ID,
        "flds": fields,
        "id": MODEL_ID.to_string(),
        "latexPost": r"\end{document}",
        "latexPre": LATEX_PREAMBLE,
        "latexsvg": false,
        "mod": seconds,
        "name": "Kanji",
        // The fields each card needs to be generated: the first card needs Kanji or Part,
        // the second needs Content. Both are always generated, since Kanji and Content
        // are never empty.
        "req": [[0, "any", [1, 2]], [1, "all", [3]]],
        "sortf": 0,
        "tags": [],
        "tmpls": templates,
        "type": 0,
        "usn": -1,
        "vers": [],
    })
}

/// A card template, with the HTML of the question and the answer.
fn template_json(ord: usize, name: &str, question: &str, answer: &str) -> Value {
    json!({
        "name": name,
        "qfmt": question,
        "afmt": answer,
        "ord": ord,
        "bafmt": "",
        "bqfmt": "",
        "bfont": "",
        "bsize": 0,
        "did": null,
    })
}

/// A deck using the default options.
fn deck_json(id: i64, name: &str, seconds: i64) -> Value {
    json!({
        "collapsed": false,
        "conf": 1,
        "desc": "",
        "dyn": 0,
        "extendNew": 10,
        "extendRev": 50,
        "id": id,
        "lrnToday": [0, 0],
        "mod": seconds,
        "name": name,
        "newToday": [0, 0],
        "revToday": [0, 0],
        "timeToday": [0, 0],
        "usn": -1,
    })
}

/// The default deck options group. Anki keeps the user's own options on import.
fn deck_options_json() -> Value {
    json!({
        "1": {
            "autoplay": true,
            "id": 1,
            "lapse": { "delays": [10], "leechAction": 0, "leechFails": 8, "minInt": 1, "mult": 0 },
            "maxTaken": 60,
            "mod": 0,
            "name": "Default",
            "new": {
                "bury": true,
                "delays": [1, 10],
                "initialFactor": 2500,
                "ints": [1, 4, 7],
                "order": 1,
                "perDay": 20,
                "separate": true,
            },
            "replayq": true,
            "rev": {
                "bury": true,
                "ease4": 1.3,
                "fuzz": 0.05,
                "ivlFct": 1,
                "maxIvl": 36500,
                "minSpace": 1,
                "perDay": 100,
            },
            "timer": 0,
            "usn": 0,
        }
    })
}

/// The collection settings, which Anki ignores on import.
fn collection_config_json() -> Value {
    json!({
        "activeDecks": [1],
        "addToCur": true,
        "collapseTime": 1200,
        "curDeck": 1,
        "curModel": MODEL_ID.to_string(),
        "dueCounts": true,
        "estTimes": true,
        "newBury": true,
        "newSpread": 0,
        "nextPos": 1,
        "sortBackwards": false,
        "sortType": "noteFld",
        "timeLim": 0,
    })
}

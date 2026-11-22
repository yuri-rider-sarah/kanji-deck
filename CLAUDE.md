# Kanji

Generates an Anki deck for learning kanji from a hand-curated dictionary of common readings, and checks the dictionary's coverage by decomposing UniDic's words into its readings. Planned: an online kanji dictionary (`src/bin/server.rs`).

## Commands
Run from the repo root (paths are relative).
- `cargo run --release --bin deck`: check `dictionary` and write `target/kanji.apkg`. Needs `assets/_NotoSansJP.ttf` (gitignored). If serializing doesn't round-trip, the result is saved to `target/dictionary_serialized`.
- `cargo run --release --bin verify`: print the UniDic words (`data/unidic/lex.csv`, gitignored) that can't be decomposed into the dictionary's readings.
- To look into the dictionary (listing entries, counting cases), write a temporary binary in `src/bin/` that calls `parser::parse` and `dict::load` rather than an ad-hoc script. This is encouraged.

## Architecture
Modules take values and return values; the executables in `src/bin/` do all I/O and chain modules. Errors are `error::Error` (position and message, no `Display`); executables report them through `file_error` (`main` is `report_errors(run())`). The parser is generic, not specific to the dictionary; its grammar is in `src/parser/parse.rs`. Parser unit tests are in `src/parser/tests/`.

## The dictionary file
About 1 MB and 23k lines. **Never read it whole**: use `grep -n` and `sed -n`. For edits across the whole file, temporarily modify the serializer and copy `target/dictionary_serialized` over the file, or write a script, then review the diff. The file order is the study order.

Editorial conventions (not enforced by the code):
- A word reading is on or kun depending on the reading of its irregular part, not the origin of the word: 本意ない is on, since ホ is an on reading of 本. Words with irregular parts of both kinds (飲兵衛, 天邪鬼) are classified arbitrarily.
- `#regular` marks word readings made up only of regular kanji readings, given only to contrast with the irregular readings of the same word (イチニチ next to ついたち).
- `european-group` holds borrowings from European languages, and `asian-group` other Asian borrowings whose readings are clearly distinct from the usual on readings (シャンハイ, ペキン, ミン); older borrowings read like on readings, such as those from Sanskrit, go in on groups.
- Primary readings (`#core`, shown in bold) are the most important ones: most frequent for kun, most productive for on. Groups are purely visual; on readings are grouped only as examples of the same reading.
- A spelling is a triple: the most common spelling, the most common kanji spelling, and the example spelling; the four spelling tags (`pri-spell`, `pri-kanji-spell`, `sec-spell`, `sec-kanji-spell`) say which are equal. `common`/`rare` uses about 5% of corpus usage as the threshold, plus the author's judgement.
- Rendaku and gemination forms aren't listed as separate readings. The exception is rendaku of an on reading that can't be attributed to a shorter word. 風 is read ブ in both 屏風 and 露天風呂, but 露天風呂 splits into 露天 + 風呂 (handled by `suffix 風呂 ぶろ` in `rendaku-suffixes`), so it isn't a separate reading. 屏風 can't be split like that, so ブ counts as a reading of 風 there.

## Anki compatibility
Users have review history, so **never change** note GUIDs (`K` for kanji and `H` for words, `K (N)` for parts, made by `deck::generate` from `Note::id`, not the formatted `kanji`), the model or deck IDs in `src/deck/generate.rs`, or the field order. With Anki's default import option ("Update notes: if newer"), existing notes are updated only if the imported modification time is newer, which is why it's the current time.

## Code style
The goal is code that's easy to read, not code that's short. More code is fine if each piece is easier to understand.

**Naming**
- A name says exactly what something is, even read out of context. Qualify generic words (`TokenKind`, not `Kind`; `known_kanji`, not `known`) and name the precise behaviour (`katakana_to_hiragana`, not `to_hiragana`, when other characters are left alone). If one variable means two things, use two variables.
- Use precise domain terms, and rename things when their meaning drifts.
- Getters (cheap accessors of a property) are nouns without `get_`: `args.pos()`, `spelling.main()`.
- Other functions start with a verb, and a module uses one verb for its main job (`read_*` for reading tokens, `serialize_*` for producing text, `check_*` for validation). Conversions are named `X_from_Y` (`html_from_part`). A function name mustn't collide with the natural name for its result: `let readings = readings_from_part(part)`, not `let readings = readings(part)`.

**Control flow**
- Prefer explicit control flow to clever combinators and tiny helpers. Write `while let Some(c) = self.peek_char() && is_space(c) { self.next_char(); }` rather than adding a `skip_spaces` or `next_char_if` helper.
- When code produces a result from a collection, prefer an iterator chain ending in `collect()` to a loop pushing to a `Vec` (`entries.iter().flat_map(notes_from_entry).collect()`). Use one pass rather than chaining several: `.filter()` on its own is fine, but when it would be followed by `.map()` or a `for` loop, do the filtering inside that body (`if groups.is_empty() { continue; }`, or a `filter_map`).
- One match arm per case rather than a combined arm with a nested match. Prefer a full `match` over `let … else` when both cases do real work.
- Avoid code that's hard to read at a glance, like escaped string literals used as character sets: `matches!(c, '"' | ':' | '|')`, not `"\":|".contains(c)`.
- Tricky control flow (for example, state flags whose meaning depends on the order they're set in) should first be rewritten so it's obvious. Only if that isn't possible, explain it with a proper comment.

**Abstractions**
- Add an abstraction only if the code using it becomes simpler. A good one separates tasks that were entangled (for example, walking through a list of arguments vs. interpreting each one), or gives a concept a name (an error constructor, a struct with `Display` instead of a string built early).
- Prefer groups of free functions over object-like types. A type is justified when it acts like an object, with a clear interface and lifecycle: an HTML writer that is created, appended to, and finished into a string. A struct that only bundles state to save passing parameters hides the data flow; pass the values explicitly instead. Think twice before adding a type whose name ends in `-er`.
- Keep data structured until it's displayed. Borrow instead of cloning where lifetimes allow.
- After a refactoring, check whether a function has lost most of what it did: one that only passes its arguments on, or checks a single condition before calling another function, should usually be merged into its caller or callee.
- Put code where it's used. Shared data types only get methods that several modules need; a helper used by one module lives in that module. Each module has one job.
- Use named fields when an enum variant or struct has several fields, especially several of the same type.
- Design types so they need as little validation as possible. Don't store data that can be derived from other fields, and put data that only applies to some cases inside the enum variant for those cases. Redundant fields are a sign of a missing variant or type: only `WordEntry` lists the kanji it introduces, instead of every entry having an `introduced_kanji` field that for kanji entries just repeats the kanji, and readings are generic over their kana (`SplitKana` in kanji entries, `UnsplitKana` in words) instead of an enum whose variant is decided by the entry. Where generic code needs behaviour specific to one kind of kana, the module that needs it defines a private trait for it (`ParseKana`, `WriteKana`).

**Errors**
- Failure goes through `Result`, including collected errors (`Result<(), Vec<Error>>`), so `?` works. Build errors with the `Error` constructors. Messages say exactly what's wrong (`ambiguous token: |||`, not `unexpected |`). Problems that don't make the output wrong are warnings, not errors.

**Comments**
- Doc comments on every top-level item (functions, types, traits, constants), public or not; fields and variants only when their name isn't enough. A doc comment says what purpose the item serves in its module, in simple terms; how it does that is what the code is for. Don't describe a function only by its return value, and don't explain the rules it applies.
- For a private item, a single sentence is often enough: `/// Try to match a kun reading at the start of the given kana.`, not a sentence about the leftover kana it returns followed by an explanation of rendaku. Mention a parameter or edge case only when the code using the item needs it (`None` at the start of a word).
- Public items are used outside the context of their module, so their docs can go into a little more detail: what they return, exclusions and edge cases (`/// Whether a character is katakana. Excludes ヶ and ヵ, as …`).
- Module docs are short and don't describe implementation details.
- Inside functions, comment only what isn't obvious from the code. Don't add comments to code just because it was touched; clear code stays uncommented. A comment sits directly next to the code it describes (inside the `match` arm or `if` branch, not above the whole construct).

**Formatting and idioms**
- Run `cargo fmt` (`rustfmt.toml`: 100 columns, `use_small_heuristics = "Max"`). rustfmt doesn't wrap comments, so wrap them by hand: lines of about 70–100 columns, broken where it reads naturally and stays easy to edit (at sentence boundaries, or at a comma or conjunction in a longer sentence), not filled up to a fixed width.
- Nested or-patterns (`Some('\n' | '\r')`), `.to_string()` rather than `.into()` for `&str` → `String`, no redundant `[..]`.
- Plain `pub` rather than `pub(crate)`/`pub(super)`.

**Review step**: before reporting a change as done, re-read the diff against this section and fix what doesn't match. In the summary, mention any rule deliberately broken and why.

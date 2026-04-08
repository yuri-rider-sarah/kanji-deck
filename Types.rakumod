unit module Types;

enum WordFreq is export <Common Rare>;

class FreqWord is export {
    has WordFreq $.freq is required;
    has Str $.word is required;
}

enum SpellingType is export <PrimarySpelling PrimaryKanjiSpelling SecondarySpelling SecondaryKanjiSpelling>;

class Spelling is export {
    has Str $.main is required;
    has FreqWord $.main-kanji is required;
    has FreqWord $.described is required;

    method type(--> SpellingType) {
        if ($.main eq $.main-kanji.word) {
            if ($.main-kanji.word eq $.described.word) {
                PrimarySpelling
            } else {
                SecondarySpelling
            }
        } else {
            if ($.main-kanji.word eq $.described.word) {
                PrimaryKanjiSpelling
            } else {
                SecondaryKanjiSpelling
            }
        }
    }
}

role ReadingKana is export {
    method joined(--> Str) { ... }
}

class SplitKana does ReadingKana is export {
    has Str $.pre is required;
    has Str $.mid is required;
    has Str $.post is required;

    method joined(--> Str) {
        $.pre ~ $.mid ~ $.post
    }
}

class UnsplitKana does ReadingKana is export {
    has Str $.kana is required;

    method joined(--> Str) { $.kana }
}

enum ReadingAttr is export <Genitive Asian European>;

enum MainReadingType is export <PrimaryReading SecondaryReading>;

role Reading is export {
    has Spelling $.spelling is required;
    has ReadingKana $.kana is required;
    has Str $.definition is required;
    has ReadingAttr @.attrs;
}

class VariantReading does Reading is export {}

role NonVariantReading does Reading is export {
    has FreqWord @.variants;
    has VariantReading @.variant-readings;
}

class RelatedReading does NonVariantReading is export {}

class MainReading does NonVariantReading is export {
    has MainReadingType $.type is required;
    has RelatedReading @.related-readings;
}

class KanjiPart is export {
    has Str $.name is required;
    has MainReading @.kun-readings;
    has MainReading @.on-readings;
    has MainReading @.combined-readings;
}

role Entry is export {}

class KanjiEntry does Entry is export {
    has Str $.kanji is required;
    has KanjiPart @.parts;
}

class CombinedEntry does Entry is export {
    has MainReading @.readings;
}

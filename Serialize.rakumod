unit module Serialize;

use Types;

sub se-open(Str $s) {
    if $*buf-str {
        $*buf-str ~= "\n";
    }
    $*buf-str ~= ' ' x $*indent-width ~ '(' ~ $s;
    $*indent-width += 4;
}

sub se-close() {
    $*buf-str ~= ')';
    $*indent-width -= 4;
}

sub se-line(Str $s) {
    if $*buf-str {
        $*buf-str ~= "\n";
    }
    $*buf-str ~= ' ' x $*indent-width ~ $s;
}

multi to-str(WordFreq $freq --> Str) {
    given $freq {
        when Common { 'common' }
        when Rare { 'rare' }
    }
}

multi to-str(FreqWord $fw --> Str) {
    "({to-str($fw.freq)} {$fw.word})"
}

multi serialize(Spelling $spelling) {
    given $spelling.type {
        when PrimarySpelling {
            se-line "(primary-spelling {$spelling.main})";
        }
        when PrimaryKanjiSpelling {
            se-line "(primary-kanji-spelling {$spelling.main} {to-str($spelling.described)})";
        }
        when SecondarySpelling {
            se-line "(secondary-spelling {$spelling.main} {to-str($spelling.described)})";
        }
        when SecondaryKanjiSpelling {
            se-line "(secondary-kanji-spelling {$spelling.main} {to-str($spelling.main-kanji)} {to-str($spelling.described)})";
        }
    }
}

multi serialize(SplitKana $kana) {
    se-line "{$kana.pre}*{$kana.mid}*{$kana.post}"
}

multi serialize(UnsplitKana $kana) {
    se-line $kana.kana;
}

multi to-str(ReadingAttr $attr --> Str) {
    given $attr {
        when Genitive { 'genitive' }
        when Asian { 'asian' }
        when European { 'european' }
    }
}

multi to-str(MainReadingType $type --> Str) {
    given $type {
        when PrimaryReading { 'primary-reading' }
        when SecondaryReading { 'secondary-reading' }
    }
}

multi serialize(Reading $reading) {
    my $type = do given $reading {
        when MainReading { to-str($reading.type) }
        when RelatedReading { 'related-reading' }
        when VariantReading { 'variant-reading' }
    };
    se-open $type;
    serialize $reading.spelling;
    serialize $reading.kana;
    se-line '"' ~ $reading.definition ~ '"';
    for $reading.attrs -> $attr {
        se-line "(special {to-str($attr)})";
    }
    if $reading ~~ NonVariantReading {
        for $reading.variants -> $variant {
            se-line "(variant {to-str($variant)})";
        }
        for $reading.variant-readings -> $variant {
            serialize $variant;
        }
    }
    if $reading ~~ MainReading {
        for $reading.related-readings -> $related {
            serialize $related;
        }
    }
    se-close;
}

multi serialize(KanjiPart $part) {
    if $part.kun-readings {
        se-open 'kun';
        for $part.kun-readings -> $reading {
            serialize $reading;
        }
        se-close;
    }
    if $part.on-readings {
        se-open 'on';
        for $part.on-readings -> $reading {
            serialize $reading;
        }
        se-close;
    }
    if $part.combined-readings {
        se-open 'from-combined';
        for $part.combined-readings -> $reading {
            serialize $reading;
        }
        se-close;
    }
}

multi serialize(KanjiEntry $entry) {
    if $entry.parts.elems == 1 {
        se-open "kanji {$entry.kanji}";
        serialize $entry.parts[0];
        se-close;
    } else {
        se-open "kanji-split {$entry.kanji}";
        for $entry.parts -> $part {
            se-open 'part "' ~ $part.name ~ '"';
            serialize $part;
            se-close;
        }
        se-close;
    }
}

multi serialize(CombinedEntry $entry) {
    se-open 'combined';
    for $entry.readings -> $reading {
        serialize $reading;
    }
    se-close;
}

sub serialize-entries(Entry @entries) is export {
    my $*indent-width = 0;
    my $*buf-str = '';
    for @entries -> $entry {
        serialize $entry;
    }
    $*buf-str ~= "\n";
    $*buf-str
}

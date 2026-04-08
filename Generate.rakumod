unit module Generate;

use Types;

sub html-escape(Str $str --> Str) {
    $str.subst('&', '&amp;', :g)
        .subst('<', '&lt;', :g)
        .subst('>', '&gt;', :g)
        .subst('"', '&quot;', :g)
}

sub json-string(Str $str --> Str) {
    '"' ~ $str
        .subst('\\', '\\\\', :g)
        .subst('"', '\"', :g)
        .subst("\n", '\n', :g)
        .subst("\t", '\t', :g) ~ '"'
}

sub html-from-word(Str $word, Bool $is-orig --> Str) {
    if $*kanji {
        $word.split($*kanji)».&html-escape.join("<span class=\"kanji\">{$*kanji}</span>")
    } elsif $is-orig {
        html-escape($word)
    } else {
        "<span class=\"kanji\">{html-escape($word)}</span>"
    }
}

sub cell-from-freq-word(FreqWord $fw, Bool $is-orig --> Str) {
    given $fw.freq {
        when Common { html-from-word($fw.word, $is-orig) }
        when Rare { "<span class=\"rare\">{html-from-word($fw.word, $is-orig)}</span>" }
    }
}

sub cell-from-orig-spelling(Spelling $spelling --> Str) {
    given $spelling.type {
        when PrimarySpelling { '' }
        when PrimaryKanjiSpelling | SecondarySpelling { html-from-word($spelling.main, True) }
        when SecondaryKanjiSpelling { html-from-word($spelling.main, True) ~ ' / ' ~ cell-from-freq-word($spelling.main-kanji, True) }
    }
}

multi cell-from-kana(SplitKana $kana, Bool $is-genitive --> Str) {
    if $is-genitive {
        "{html-escape($kana.pre)}<span class=\"kana\">{html-escape($kana.mid.substr(0, *-1))}<span class=\"particle\">{html-escape($kana.mid.substr(*-1))}</span></span>{html-escape($kana.post)}"
    } else {
        "{html-escape($kana.pre)}<span class=\"kana\">{html-escape($kana.mid)}</span>{html-escape($kana.post)}"
    }
}

multi cell-from-kana(UnsplitKana $kana, Bool $is-genitive --> Str) {
    "<span class=\"kana\">{html-escape($kana.kana)}</span>"
}

sub tr-from-variant(FreqWord $variant, Bool $combined --> Str) {
    my $combined-attr = $combined ?? ' combined' !! '';
    "<tr class=\"variant$combined-attr\"><td></td><td>{cell-from-freq-word($variant, False)}</td><td></td><td></td></tr>\n"
}

sub trs-from-reading(Reading $reading, Bool $combined --> Str) {
    my @classes;
    for $reading.attrs -> $attr {
        @classes.push(do given $attr {
            when Genitive { 'genitive' }
            when Asian { 'asian' }
            when European { 'european' }
        });
    }
    @classes.push(do given $reading {
        when MainReading {
            given $reading.type {
                when PrimaryReading { 'primary' }
                when SecondaryReading { 'secondary' }
            }
        }
        when RelatedReading { 'related' }
        when VariantReading { 'variant' }
    });
    if $combined {
        @classes.push('combined');
    }
    my $class-attr = @classes ?? ' class="' ~ @classes.join(' ') ~ '"' !! '';
    my $is-genitive = so $reading.attrs.grep(Genitive);
    my $result = "<tr$class-attr><td>{cell-from-orig-spelling($reading.spelling)}</td><td>{cell-from-freq-word($reading.spelling.described, False)}</td><td>{cell-from-kana($reading.kana, $is-genitive)}</td><td>{html-escape($reading.definition)}</td></tr>\n";
    if ($reading ~~ NonVariantReading) {
        for $reading.variants -> $variant {
            $result ~= tr-from-variant($variant, $combined);
        }
        for $reading.variant-readings -> $variant-reading {
            $result ~= trs-from-reading($variant-reading, $combined);
        }
    }
    if ($reading ~~ MainReading) {
        for $reading.related-readings -> $related-reading {
            $result ~= trs-from-reading($related-reading, $combined);
        }
    }
    $result
}

sub table-from-kanji-part(KanjiPart $part --> Str) {
    my $result = "<table>\n";
    for $part.kun-readings -> $reading {
        $result ~= trs-from-reading($reading, False);
    }
    for $part.on-readings -> $reading {
        $result ~= trs-from-reading($reading, False);
    }
    for $part.combined-readings -> $reading {
        $result ~= trs-from-reading($reading, True);
    }
    $result ~= '</table>';
    $result
}

sub table-from-combined-entry(CombinedEntry $entry --> Str) {
    my $result = "<table>\n";
    for $entry.readings -> $reading {
        $result ~= trs-from-reading($reading, True);
    }
    $result ~= '</table>';
    $result
}

sub json-from-part(KanjiPart $part, Str $kanji --> Str) {
    '{"kanji":' ~ json-string($kanji) ~ ',"part":' ~ json-string($part.name)
    ~ ',"content":' ~ json-string(table-from-kanji-part($part)) ~ '}'
}

multi json-from-entry(KanjiEntry $entry --> Str) {
    my $*kanji = $entry.kanji;
    $entry.parts».&{ json-from-part($_, $entry.kanji) }.join(',')
}

multi json-from-entry(CombinedEntry $entry --> Str) {
    my $*kanji = '';
    '{"kanji":' ~ json-string($entry.readings[0].spelling.described.word)
    ~ ',"part":"","content":' ~ json-string(table-from-combined-entry($entry)) ~ '}'
}

sub json-from-entries(Entry @entries --> Str) is export {
    '[' ~ @entries».&json-from-entry.join(',') ~ ']'
}

unit module Check;

use Types;

sub replace-kanji(Str $word --> Str) {
    $word.split($*kanji).join('[]')
}

multi to-table(Reading $reading --> Array[Str]) {
    my Str @result = (
        replace-kanji($reading.spelling.main),
        replace-kanji($reading.spelling.main-kanji.word),
        replace-kanji($reading.spelling.described.word),
        $reading.kana.joined,
        $reading.definition
    );
    if $reading ~~ MainReading {
        for $reading.related-readings -> $related-reading {
            @result.append(to-table($related-reading));
        }
    }
    @result
}

multi to-table(KanjiPart $part --> Array[Str]) {
    my Str @result;
    for $part.kun-readings -> $reading {
        @result.append(to-table($reading));
    }
    for $part.on-readings -> $reading {
        @result.append(to-table($reading));
    }
    for $part.combined-readings -> $reading {
        @result.append(to-table($reading));
    }
    @result
}

multi to-table(WordEntry $entry --> Array[Str]) {
    my Str @result;
    for $entry.readings -> $reading {
        @result.append(to-table($reading));
    }
    @result
}

sub check-def(Str $name, Str $val, Str %defs) {
    if %defs{$val}:exists {
        say "Warning: {%defs{$val}} and $name have similar readings";
    } else {
        %defs{$val} = $name;
    }
}

sub check-word(Str $word, SetHash[Str] $kanji-set) {
    for $word.comb(/<:Script<Han>>/) -> $kanji {
        if $kanji ∉ $kanji-set {
            say "Warning: word entry $word contains new kanji $kanji";
        }
    }
}

sub check-reading(Reading $reading, SetHash[Str] $kanji-set) {
    check-word($reading.spelling.main, $kanji-set);
    check-word($reading.spelling.main-kanji.word, $kanji-set);
    check-word($reading.spelling.described.word, $kanji-set);
    for $reading.variants -> $variant {
        check-word($variant.word, $kanji-set);
    }
}

sub check-entries(Entry @entries) is export {
    my SetHash[Str] $kanji-set = SetHash[Str].new('々');
    my Str %defs;
    for @entries -> $entry {
        given $entry {
            when KanjiEntry {
                $kanji-set.set($entry.kanji);
                my $*kanji = $entry.kanji;
                for $entry.parts -> $part {
                    check-def("{$entry.kanji}", to-table($part).join("\n"), %defs);
                }
            }
            when WordEntry {
                if $entry.readings[0].type != PrimaryReading {
                    say "Warning: first reading in word entry {$entry.readings[0].spelling.main} is not primary";
                }
                for $entry.readings -> $main_reading {
                    check-reading($main_reading, $kanji-set);
                    for $main_reading.related-readings -> $related-reading {
                        check-reading($related-reading, $kanji-set);
                    }
                }
            }
        }
    }
}

unit module Check;

use Types;

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
    for @entries -> $entry {
        given $entry {
            when KanjiEntry {
                $kanji-set.set($entry.kanji);
            }
            when WordEntry {
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

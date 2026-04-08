unit module Parse;

use Types;

enum KanaType <KunReading OnReading CombinedReading>;

class ReadingList {
    has MainReading @.kun;
    has MainReading @.on;
    has MainReading @.combined;
}

grammar Dictionary {
    token word { \w+ }
    token string { '"' $<text>=(<-["]>+) '"' }
    token word-freq { 'common' | 'rare' }
    rule freq-word { '(' <word-freq> <word> ')' }
    proto rule spelling {*}
    rule spelling:sym<primary> { '(' 'primary-spelling' <word> ')' }
    rule spelling:sym<primary-kanji> { '(' 'primary-kanji-spelling' <word> <freq-word> ')' }
    rule spelling:sym<secondary> { '(' 'secondary-spelling' <word> <freq-word> ')' }
    rule spelling:sym<secondary-kanji> { '(' 'secondary-kanji-spelling' <word> <kanji=.freq-word> <desc=.freq-word> ')' }
    token kana(KanaType $type) {
        || <?{ $type != OnReading }> $<pre>=(<:Hiragana>*) '*' $<mid>=(<:Hiragana>+) '*' $<post>=(<:Hiragana>*)
        || <?{ $type != KunReading }> $<pre>=([<:Katakana>|ー]*) '*' $<mid>=([<:Katakana>|ー]+) '*' $<post>=([<:Katakana>|ー]*)
        || <?{ $type == CombinedReading }> $<kana>=(<:Hiragana>+ | [<:Katakana>|ー]+)
    }
    token special-attr { 'genitive' | 'asian' | 'european' }
    rule special { '(' 'special' <special-attr> ')' }
    rule variant { '(' 'variant' <freq-word> ')' }
    rule variant-reading(KanaType $type) {
        '(' 'variant-reading' <spelling> <kana($type)> <string>
        <special>* ')'
    }
    rule related-reading(KanaType $type) {
        '(' 'related-reading' <spelling> <kana($type)> <string>
        <variant>* <variant-reading($type)>* <special>* ')'
    }
    token main-reading-type { 'primary-reading' | 'secondary-reading' }
    rule main-reading(KanaType $type) {
        '(' <main-reading-type> <spelling> <kana($type)> <string>
        <variant>* <variant-reading($type)>* <related-reading($type)>* <special>* ')'
    }
    rule kun-list { '(' 'kun' <main-reading(KunReading)>+ ')' }
    rule on-list { '(' 'on' <main-reading(OnReading)>+ ')' }
    rule combined-list { '(' 'from-combined' <main-reading(CombinedReading)>+ ')' }
    rule reading-list { <kun-list>? <on-list>? <combined-list>? }
    rule part { '(' 'part' <string> <reading-list> ')' }
    proto rule entry {*}
    rule entry:sym<kanji> { '(' 'kanji' <word> <reading-list> ')' }
    rule entry:sym<kanji-split> { '(' 'kanji-split' <word> <part>+ ')' }
    rule entry:sym<combined> { '(' 'combined' <main-reading(CombinedReading)>+ ')' }
    rule TOP { <entry>* [ $ || <.entry-error> ] }

    method entry-error() {
        my $line = $*parsed-text.substr(0, self.pos).lines.elems + 1;
        die "Entry at line $line is malformed";
    }
}

class DictionaryActions {
    method word($/ --> Str) {
        make ~$/
    }
    method string($/ --> Str) {
        make ~$<text>
    }
    method word-freq($/ --> WordFreq) {
        given ~$/ {
            when 'common' { make Common }
            when 'rare' { make Rare }
        }
    }
    method freq-word($/ --> FreqWord) {
        make FreqWord.new(
            freq => $<word-freq>.made,
            word => $<word>.made,
        )
    }
    method spelling:sym<primary>($/ --> Spelling) {
        make Spelling.new(
            main => $<word>.made,
            main-kanji => FreqWord.new(freq => Common, word => ~$<word>.made),
            described => FreqWord.new(freq => Common, word => ~$<word>.made),
        )
    }
    method spelling:sym<primary-kanji>($/ --> Spelling) {
        make Spelling.new(
            main => $<word>.made,
            main-kanji => $<freq-word>.made,
            described => $<freq-word>.made,
        )
    }
    method spelling:sym<secondary>($/ --> Spelling) {
        make Spelling.new(
            main => $<word>.made,
            main-kanji => FreqWord.new(freq => Common, word => ~$<word>.made),
            described => $<freq-word>.made,
        )
    }
    method spelling:sym<secondary-kanji>($/ --> Spelling) {
        make Spelling.new(
            main => $<word>.made,
            main-kanji => $<kanji>.made,
            described => $<desc>.made,
        )
    }
    method kana($/ --> ReadingKana) {
        with $<kana> {
            make UnsplitKana.new(kana => ~$<kana>)
        } else {
            make SplitKana.new(pre => ~$<pre>, mid => ~$<mid>, post => ~$<post>)
        }
    }
    method special-attr($/ --> ReadingAttr) {
        given ~$/ {
            when 'genitive' { make Genitive }
            when 'european' { make European }
            when 'asian' { make Asian }
        }
    }
    method special($/ --> ReadingAttr) {
        make $<special-attr>.made
    }
    method variant($/ --> FreqWord) {
        make $<freq-word>.made
    }
    method variant-reading($/ --> VariantReading) {
        make VariantReading.new(
            spelling => $<spelling>.made,
            kana => $<kana>.made,
            definition => $<string>.made,
            attrs => @<special>».made,
        )
    }
    method related-reading($/ --> RelatedReading) {
        make RelatedReading.new(
            spelling => $<spelling>.made,
            kana => $<kana>.made,
            definition => $<string>.made,
            attrs => @<special>».made,
            variants => @<variant>».made,
            variant-readings => @<variant-reading>».made,
        )
    }
    method main-reading-type($/ --> MainReadingType) {
        given ~$/ {
            when 'primary-reading' { make PrimaryReading }
            when 'secondary-reading' { make SecondaryReading }
        }
    }
    method main-reading($/ --> MainReading) {
        make MainReading.new(
            type => $<main-reading-type>.made,
            spelling => $<spelling>.made,
            kana => $<kana>.made,
            definition => $<string>.made,
            attrs => @<special>».made,
            variants => @<variant>».made,
            variant-readings => @<variant-reading>».made,
            related-readings => @<related-reading>».made,
        )
    }
    method kun-list($/ --> Array[MainReading]) {
        make Array[MainReading].new(@<main-reading>».made)
    }
    method on-list($/ --> Array[MainReading]) {
        make Array[MainReading].new(@<main-reading>».made)
    }
    method combined-list($/ --> Array[MainReading]) {
        make Array[MainReading].new(@<main-reading>».made)
    }
    method reading-list($/ --> ReadingList) {
        make ReadingList.new(
            kun => $<kun-list> ?? $<kun-list>.made !! [],
            on => $<on-list> ?? $<on-list>.made !! [],
            combined => $<combined-list> ?? $<combined-list>.made !! [],
        )
    }
    method part($/ --> KanjiPart) {
        my ReadingList $reading-list = $<reading-list>.made;
        make KanjiPart.new(
            name => $<string>.made,
            kun-readings => $reading-list.kun,
            on-readings => $reading-list.on,
            combined-readings => $reading-list.combined,
        )
    }
    method entry:sym<kanji>($/ --> KanjiEntry) {
        my ReadingList $reading-list = $<reading-list>.made;
        make KanjiEntry.new(
            kanji => $<word>.made,
            parts => [KanjiPart.new(
                name => "",
                kun-readings => $reading-list.kun,
                on-readings => $reading-list.on,
                combined-readings => $reading-list.combined,
            )]
        );
    }
    method entry:sym<kanji-split>($/ --> KanjiEntry) {
        make KanjiEntry.new(
            kanji => $<word>.made,
            parts => @<part>».made,
        )
    }
    method entry:sym<combined>($/ --> CombinedEntry) {
        make CombinedEntry.new(readings => @<main-reading>».made)
    }
    method TOP($/ --> Array[Entry]) {
        make Array[Entry].new(@<entry>».made)
    }
}

sub parse-file(Str $text --> Array[Entry]) is export {
    my $*parsed-text = $text;
    my $match = Dictionary.parse($text, actions => DictionaryActions);
    die "Parse error" unless $match;
    $match.made
}

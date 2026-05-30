#!/usr/bin/env -S raku -I.

use Types;
use Parse;
use Serialize;
use Check;
use Generate;

my $dict = slurp 'dictionary';
my Entry @entries = parse-file($dict);
my $serialized-dict = serialize-entries(@entries);
if $serialized-dict ne $dict {
    spurt 'dictionary2', $serialized-dict;
    say 'Warning: Serialized file does not match original!';
}
check-entries(@entries);
my $json = json-from-entries(@entries);
my $proc = run('./generate.py', :in);
$proc.in.print($json);
$proc.in.close;

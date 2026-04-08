#!/usr/bin/env -S raku -I.

use Types;
use Parse;
use Serialize;
use Generate;

my $dict = slurp 'dictionary';
my Entry @entries = parse-file($dict);
say "Warning: Serialized file does not match original!" if serialize-entries(@entries) ne $dict;
my $json = json-from-entries(@entries);
my $proc = run('./generate.py', :in);
$proc.in.print($json);
$proc.in.close;

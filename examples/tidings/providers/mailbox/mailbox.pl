#!/usr/bin/env perl
# Tidings's mailbox, shared as `app.mailbox`: SAMPLE DATA read from
# sample.json beside this script and published once, as one JSON line.
# Point it at an IMAP or JMAP account to make it real. Exits when the
# app closes stdin; messages are ignored.
use strict;
use warnings;
use JSON::PP ();

$| = 1;
my $json = JSON::PP->new->canonical;
open my $fh, '<:raw', 'sample.json' or do {
    print qq({"available":false,"reason":"sample.json is missing"}\n);
    1 while <STDIN>;
    exit 0;
};
my $data = do { local $/; $json->decode(<$fh>) };
close $fh;
$data->{available} = JSON::PP::true;
print $json->encode($data), "\n";
1 while <STDIN>;

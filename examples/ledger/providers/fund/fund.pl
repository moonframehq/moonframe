#!/usr/bin/env perl
# Publishes sample.json (sample numbers for the demo) as one JSON line, and
# again on any message. Exits when the app closes stdin.
use strict;
use warnings;
use JSON::PP ();

$| = 1;
my $json = JSON::PP->new->canonical;

sub publish {
    open my $fh, '<', 'sample.json' or do {
        print qq({"available":false,"reason":"sample.json is missing"}\n);
        return;
    };
    local $/;
    my $data = eval { $json->decode(<$fh>) };
    close $fh;
    print $data ? $json->encode({ available => JSON::PP::true, sample => JSON::PP::true, %$data }) . "\n"
        : qq({"available":false,"reason":"sample.json is not JSON"}\n);
}

publish();
while (my $line = <STDIN>) {
    publish();
}

#!/usr/bin/env perl
# Sorrel's research run, shared as `app.research`: SAMPLE DATA read from
# sample.json beside this script, and a SCRIPTED DEMO of an agent working
# through its plan, played on a clock (one tick every 0.4 s). It is not a
# model: a real agent would publish its plan, progress and cells the same way.
#
# Output: everything in sample.json, plus `run`:
#   tick, playing, done
#   step      1 to 4, or 5 once done
#   found     apps found so far (step 1 adds the sheet's rows one by one)
#   row, cell the cell being written in step 2 (cell 1 to 4: APY, monthly
#             fee, minimum deposit, rating), else 0
#   filled    cells written so far; reviewed: apps reviewed in step 3
#   elapsed   "2m 14s"; left: "about 2 min left"
#   used      credits this run used; estimate: what it should use
#
# RESEARCH_DEMO_TICK=n in the environment starts the run at tick n (for
# stills: 61 is step 2, row 10, the monthly fee being typed).
#
# Messages (any mod): "toggle" pauses or plays (and replays once done),
# "restart" starts over. Exits when the app closes stdin.
use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use Time::HiRes qw(time);
use POSIX qw(floor ceil);

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);

open my $fh, '<:raw', 'sample.json' or do {
    print qq({"available":false,"reason":"sample.json is missing"}\n);
    1 while <STDIN>;
    exit 0;
};
my $data = do { local $/; JSON::PP->new->decode(<$fh>) };
close $fh;
$data->{available} = JSON::PP::true;

my $N = scalar @{ $data->{apps} };
my $CELLS = 4;
my $PER_ROW = $CELLS + 1;               # a beat between rows
my $COLLECT = $N + 2;                   # step 2 starts
my $REVIEW = $COLLECT + $PER_ROW * $N;  # step 3 starts
my $SUMMARY = $REVIEW + $N + 2;         # step 4 starts
my $DONE = $SUMMARY + 12;               # finished
my $LOOP = $DONE + 30;                  # plays again
my $PACE = 1.84;                        # demo seconds per tick, for the clock
my $TICK = 0.4;

my $tick = ($ENV{RESEARCH_DEMO_TICK} // '') =~ /^(\d+)$/ && $1 < $LOOP ? $1 + 0 : 0;
my $playing = 1;

sub run {
    my $t = $tick < $DONE ? $tick : $DONE;
    my ($step, $found, $row, $cell, $filled, $reviewed) = (1, 0, 0, 0, 0, 0);
    $found = $t < $N ? $t : $N;
    if ($t >= $COLLECT) {
        $step = 2;
        my $k = $t - $COLLECT;
        my $index = floor($k / $PER_ROW);
        if ($index < $N) {
            $row = $index + 1;
            my $at = $k % $PER_ROW;
            $cell = $at < $CELLS ? $at + 1 : 0;
            $filled = $index * $CELLS + ($cell ? $cell - 1 : $CELLS);
        }
        else {
            $filled = $N * $CELLS;
        }
    }
    if ($t >= $REVIEW) {
        $step = 3;
        $reviewed = $t - $REVIEW < $N ? $t - $REVIEW : $N;
    }
    $step = 4 if $t >= $SUMMARY;
    $step = 5 if $t >= $DONE;
    my $seconds = 20 + floor($t * $PACE);
    my $left = ($DONE - $t) * $PACE;
    my $used = 30 + $found + floor($filled * 1.15) + floor($reviewed / 2) + ($t >= $SUMMARY ? floor(($t - $SUMMARY) / 2) : 0);
    return {
        tick => $tick, playing => $playing ? JSON::PP::true : JSON::PP::false,
        done => $step == 5 ? JSON::PP::true : JSON::PP::false,
        step => $step, found => $found, row => $row, cell => $cell, filled => $filled,
        reviewed => $reviewed, used => $used, estimate => 120,
        elapsed => sprintf('%dm %02ds', floor($seconds / 60), $seconds % 60),
        left => $step == 5 ? 'done' : ($left < 60 ? 'under a minute left' : sprintf('about %d min left', ceil($left / 60))),
    };
}

sub publish {
    $data->{run} = run();
    print $json->encode($data), "\n";
}

sub receive {
    my ($line) = @_;
    my $in = eval { JSON::PP->new->decode($line) } or return;
    my $msg = ref $in eq 'HASH' ? $in->{message} : undef;
    return unless defined $msg && !ref $msg;
    if ($msg eq 'toggle') {
        if ($tick >= $DONE) { $tick = 0; $playing = 1 }
        else { $playing = !$playing }
    }
    elsif ($msg eq 'restart') {
        $tick = 0;
        $playing = 1;
    }
    else {
        return;
    }
    publish();
}

publish();
my $select = IO::Select->new(\*STDIN);
my $buffer = '';
my $next = time + $TICK;
while (1) {
    my $wait = $playing ? $next - time : undef;
    $wait = 0 if defined $wait && $wait < 0;
    if ($select->can_read($wait)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        receive($1) while $buffer =~ s/^([^\n]*)\n//;
    }
    if ($playing && time >= $next) {
        $tick = $tick + 1 >= $LOOP ? 0 : $tick + 1;
        $next = time + $TICK;
        publish();
    }
    $next = time + $TICK unless $playing;
}

#!/usr/bin/env perl
# Cadence's workflow operations, shared as `app.cadence`: SAMPLE DATA. The
# figures for each time range come from sample.json beside this script; the
# live feed is simulated here, once a second: a run starts every few
# seconds, running ones finish (now and then with an error), and finished
# runs count towards the success figures. Point it at your orchestrator's
# API to make it real.
#
# Messages (from any mod): { range = "1H" | "4H" | "24H" | "72H" | "7D" | "30D" }
# switches the figures; { rerun = <run id> } starts that run's workflow
# again. Exits when the app closes stdin.
use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use Time::HiRes qw(time);

$| = 1;
my $json = JSON::PP->new->canonical->utf8;

open my $fh, '<:raw', 'sample.json' or do {
    print qq({"available":false,"reason":"sample.json is missing"}\n);
    1 while <STDIN>;
    exit 0;
};
my $sample = do { local $/; $json->decode(<$fh>) };
close $fh;

my %workflow = map { $_->{name} => $_ } @{ $sample->{workflows} };
my @names    = map { $_->{name} } @{ $sample->{workflows} };
srand(42);

# The figures of the chosen range, copied so live runs can add to them.
my $range = '1H';
my $figures;
sub choose {
    my ($name) = @_;
    return unless $sample->{ranges}{$name};
    $range   = $name;
    $figures = $json->decode($json->encode($sample->{ranges}{$name}));
}
choose('1H');

# The live feed, newest first.
my $now    = time;
my $serial = 0;
my @runs;
for my $past (@{ $sample->{history} }) {
    my $flow = $workflow{ $past->{workflow} };
    push @runs, {
        id       => 'r' . ++$serial,
        workflow => $past->{workflow},
        started  => $now - $past->{age},
        duration => $past->{duration},
        target   => $past->{target} // $past->{duration},
        status   => $past->{status},
        error    => $past->{status} eq 'failed' ? $flow->{error} : '',
        retried  => JSON::PP::false,
    };
}
my $next_start = $now + 6;
my $tick       = 0;

sub start_run {
    my ($name) = @_;
    unshift @runs, {
        id       => 'r' . ++$serial,
        workflow => $name,
        started  => time,
        duration => 0,
        target   => 4 + rand(55),
        status   => 'running',
        error    => '',
        retried  => JSON::PP::false,
    };
    splice @runs, 9 if @runs > 9;
}

sub finish {
    my ($run) = @_;
    my $flow = $workflow{ $run->{workflow} };
    my $failed = rand() < $flow->{fails};
    $run->{status}   = $failed ? 'failed' : 'success';
    $run->{duration} = $run->{target};
    $run->{error}    = $failed ? $flow->{error} : '';
    if ($failed) { $figures->{success}{errors}++ } else { $figures->{success}{success}++ }
    for my $row (@{ $figures->{errored} }) {
        next unless $row->{name} eq $run->{workflow};
        $row->{runs}++;
        if ($failed) { $row->{failed}++; $row->{count}++ }
    }
}

sub ago {
    my ($seconds) = @_;
    $seconds = int($seconds);
    return "${seconds}s ago" if $seconds < 60;
    my ($m, $s) = (int($seconds / 60), $seconds % 60);
    return "${m}m ${s}s ago" if $m < 60;
    my $h = int($m / 60);
    return "${h}h " . ($m % 60) . "m ago";
}

sub until_text {
    my ($seconds) = @_;
    $seconds = int($seconds);
    return "in ${seconds}s" if $seconds < 60;
    my ($m, $s) = (int($seconds / 60), $seconds % 60);
    return "in ${m}m ${s}s" if $m < 60;
    my $h = int($m / 60);
    return "in ${h}h " . ($m % 60) . "m";
}

# 1444 as "1 444", as the design groups thousands.
sub grouped {
    my ($n) = @_;
    1 while $n =~ s/^(\d+)(\d{3})/$1\x{2009}$2/;
    return $n;
}

# A small swing of a series' latest point around the sample's own value,
# so the charts breathe without drifting away.
sub wobble {
    my ($series, $step) = @_;
    my $base = $sample->{ranges}{$range}{$series}{points}[-1];
    $figures->{$series}{points}[-1] = 0 + sprintf('%.3f', $base + (rand(2) - 1) * $step);
}

sub publish {
    my $at = time;
    my $sla     = $figures->{sla};
    my $latency = $figures->{latency};
    my $lag     = $figures->{lag};
    my $ok      = $figures->{success}{success};
    my $bad     = $figures->{success}{errors};
    my $rate    = 100 * $ok / ($ok + $bad);
    my $most    = 0;
    for my $row (@{ $figures->{errored} }) { $most = $row->{count} if $row->{count} > $most }

    my @feed = map {
        my $running = $_->{status} eq 'running';
        {
            id       => $_->{id},
            started  => ago($at - $_->{started}),
            workflow => $_->{workflow},
            duration => sprintf('%.1fs', $running ? $at - $_->{started} : $_->{duration}),
            status   => $_->{status},
            error    => $_->{error},
            retried  => $_->{retried},
        }
    } @runs;

    my %last;
    for my $run (reverse @runs) { $last{ $run->{workflow} } = $run->{status} unless $run->{status} eq 'running' }
    my @upcoming = sort { $a->{seconds} <=> $b->{seconds} } map {
        my $flow   = $workflow{$_};
        my $offset = length($_) * 37;
        my $left   = $flow->{period} - ((int($at) + $offset) % $flow->{period});
        {
            id       => $_,
            workflow => $_,
            every    => $flow->{every},
            next     => until_text($left),
            seconds  => $left,
            last     => $last{$_} // 'success',
        }
    } @names;

    print $json->encode({
        available => JSON::PP::true,
        note      => 'Sample data, simulated live.',
        range     => $range,
        tick      => $tick,
        sla       => {
            value     => sprintf('%d%%', $sla->{value}),
            latency   => $sla->{latency},
            freshness => $sla->{freshness},
            points    => $sla->{points},
        },
        latency => {
            value  => sprintf('%.1fs', $latency->{points}[-1]),
            p99    => $latency->{p99},
            p95    => $latency->{p95},
            points => $latency->{points},
        },
        lag => {
            value  => "$lag->{value}s",
            p99    => $lag->{p99},
            p95    => $lag->{p95},
            points => $lag->{points},
        },
        workflows => {
            total  => $figures->{workflows}{active} + $figures->{workflows}{paused},
            active => $figures->{workflows}{active},
            paused => $figures->{workflows}{paused},
        },
        success => {
            rate    => sprintf('%.1f%%', $rate),
            success => grouped($ok),
            errors  => grouped($bad),
            # The bar reads against a 97% floor, so a fraction of a percent
            # still shows: 98.7% fills a little over half.
            share   => 0 + sprintf('%.3f', ($rate - 97) / 3 < 0.05 ? 0.05 : ($rate - 97) / 3 > 0.95 ? 0.95 : ($rate - 97) / 3),
        },
        errored => [ map { {
            name   => $_->{name},
            kind   => $_->{kind},
            runs   => grouped($_->{runs}),
            failed => grouped($_->{failed}),
            count  => grouped($_->{count}),
            share  => $most ? 0 + sprintf('%.3f', $_->{count} / $most) : 0,
        } } @{ $figures->{errored} } ],
        feed     => \@feed,
        upcoming => [ @upcoming[0 .. 5] ],
    }), "\n";
}

sub receive {
    my ($line) = @_;
    my $in = eval { $json->decode($line) } or return;
    my $message = $in->{message};
    return unless ref $message eq 'HASH';
    if (my $name = $message->{range}) {
        choose($name);
    }
    if (my $id = $message->{rerun}) {
        my ($run) = grep { $_->{id} eq $id } @runs;
        if ($run && !$run->{retried}) {
            $run->{retried} = JSON::PP::true;
            start_run($run->{workflow});
        }
    }
}

my $select = IO::Select->new(\*STDIN);
my $buffer = '';
publish();
while (1) {
    my $wake = time + 1;
    while ((my $wait = $wake - time) > 0) {
        next unless $select->can_read($wait);
        my $read = sysread STDIN, $buffer, 4096, length $buffer;
        exit 0 unless $read;
        while ($buffer =~ s/^([^\n]*)\n//) {
            receive($1);
            publish();
        }
    }
    $tick++;
    my $at = time;
    for my $run (@runs) {
        finish($run) if $run->{status} eq 'running' && $at - $run->{started} >= $run->{target};
    }
    if ($at >= $next_start) {
        start_run($names[ int(rand(@names)) ]);
        $next_start = $at + 5 + rand(6);
    }
    if ($tick % 3 == 0) {
        wobble("latency", 0.08);
        wobble("sla", 0.2);
        wobble("lag", 8);
    }
    publish();
}

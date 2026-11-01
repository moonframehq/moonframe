#!/usr/bin/env perl
# Gantry's services and their telemetry: SAMPLE data, simulated here. Every
# 3 seconds a new sample is taken, standing for 15 minutes of the platform,
# and the whole state is printed as one JSON line (`app.telemetry`). Reads
# the app's messages, one `{"from": ..., "message": ...}` per line, on stdin;
# exits when the app closes it.
#
# Output:
#   now        the simulated clock, "HH:MM"
#   times      a heading per sample, oldest first ("TODAY, 06:00")
#   axis       { index, text }: x-axis labels every 3 hours, "NOW" last
#   services   { id, name, status, tags, regions, description, version,
#                deployed, replicas, rps, p95, errors, resources, history,
#                logs, deploys }
#     resources  cpu, gpu, memory: { allocated, used, util, label, unit,
#                limit, limit_unit, step }
#     history    cpu, gpu, memory: utilization in %, one per sample;
#                rps and p95 per sample
#
# Messages:
#   limit = { service, resource, by = 1|-1 }  raise or lower an allocation
#   deploy = id                               roll out a new version

use strict;
use utf8;
use warnings;
use JSON::PP ();
use IO::Select;
use POSIX qw(floor);
use Time::HiRes qw(time);

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);

my $SAMPLES = 97;        # 24 hours of 15 minutes, both ends included
my $STEP = 15 * 60;
my $TICK = 3;

srand(7);

# What each service asks of its resources, in absolute units: a base, a daily
# swing and noise. Utilization is that over what is allocated.
sub resource {
    my (%r) = @_;
    return { %r, demand => [] };
}

my @SERVICES = (
    {
        id => 'token-relay', name => 'token-relay', status => 'active',
        tags => ['API', 'EDGE', 'AGENT RUNTIME'], regions => 'iad · fra · sin',
        description => 'Edge relay that fans agent tool calls out to workers, streams tokens back to clients and replays failed runs in three regions.',
        version => 142, deployed => 9, replicas => 12, rps => 1840, p95 => 412, errors => 0.18,
        cpu => resource(allocated => 8, base => 4.4, swing => 0.6, noise => 0.18, unit => 'CORES', step => 1, min => 2, max => 32),
        gpu => resource(allocated => 4, base => 2.6, swing => 0.45, noise => 0.08, unit => 'GPUS', model => 'H100', step => 1, min => 1, max => 8, rise => 0.9),
        memory => resource(allocated => 64, base => 14, swing => 1.6, noise => 0.6, unit => 'GB', step => 8, min => 16, max => 256),
    },
    {
        id => 'vector-index', name => 'vector-index', status => 'active',
        tags => ['STORAGE', 'SEARCH'], regions => 'iad · fra',
        description => 'Sharded embedding index behind retrieval: serves nearest-neighbour queries and rebuilds shards as documents land.',
        version => 88, deployed => 31, replicas => 6, rps => 920, p95 => 38, errors => 0.02,
        cpu => resource(allocated => 16, base => 9.5, swing => 2.0, noise => 0.4, unit => 'CORES', step => 1, min => 2, max => 32),
        gpu => resource(allocated => 2, base => 0.7, swing => 0.3, noise => 0.06, unit => 'GPUS', model => 'L40S', step => 1, min => 1, max => 8),
        memory => resource(allocated => 128, base => 92, swing => 6, noise => 1.5, unit => 'GB', step => 8, min => 16, max => 256),
    },
    {
        id => 'eval-scheduler', name => 'eval-scheduler', status => 'degraded',
        tags => ['BATCH', 'AGENT RUNTIME'], regions => 'iad',
        description => 'Queues evaluation suites against new checkpoints, leases GPUs by the minute and posts scores to the run log.',
        version => 57, deployed => 3, replicas => 4, rps => 64, p95 => 1290, errors => 2.4,
        cpu => resource(allocated => 4, base => 3.3, swing => 0.4, noise => 0.15, unit => 'CORES', step => 1, min => 2, max => 32),
        gpu => resource(allocated => 8, base => 7.1, swing => 0.6, noise => 0.2, unit => 'GPUS', model => 'H100', step => 1, min => 1, max => 8),
        memory => resource(allocated => 96, base => 58, swing => 8, noise => 2, unit => 'GB', step => 8, min => 16, max => 256),
    },
    {
        id => 'artifact-store', name => 'artifact-store', status => 'active',
        tags => ['STORAGE', 'API'], regions => 'iad · fra · sin',
        description => 'Content-addressed store for checkpoints, traces and tool outputs, with signed URLs and a lifecycle sweep.',
        version => 211, deployed => 64, replicas => 9, rps => 2310, p95 => 96, errors => 0.05,
        cpu => resource(allocated => 8, base => 2.6, swing => 0.5, noise => 0.12, unit => 'CORES', step => 1, min => 2, max => 32),
        gpu => resource(allocated => 1, base => 0.05, swing => 0.02, noise => 0.01, unit => 'GPUS', model => 'T4', step => 1, min => 1, max => 8),
        memory => resource(allocated => 32, base => 11, swing => 2, noise => 0.4, unit => 'GB', step => 8, min => 16, max => 256),
    },
    {
        id => 'webhook-fanout', name => 'webhook-fanout', status => 'active',
        tags => ['EDGE', 'EVENTS'], regions => 'fra · sin',
        description => 'Delivers run events to customer webhooks with retries, backoff and a dead-letter queue per endpoint.',
        version => 34, deployed => 140, replicas => 3, rps => 410, p95 => 180, errors => 0.4,
        cpu => resource(allocated => 2, base => 0.7, swing => 0.25, noise => 0.06, unit => 'CORES', step => 1, min => 2, max => 32),
        gpu => resource(allocated => 1, base => 0.0, swing => 0.0, noise => 0.0, unit => 'GPUS', model => 'NONE', step => 1, min => 1, max => 8),
        memory => resource(allocated => 16, base => 5, swing => 1, noise => 0.3, unit => 'GB', step => 8, min => 16, max => 256),
    },
    {
        id => 'sandbox-pool', name => 'sandbox-pool', status => 'paused',
        tags => ['AGENT RUNTIME', 'ISOLATION'], regions => 'iad',
        description => 'Warm pool of microVM sandboxes where agents run untrusted code; paused while the kernel image is rebuilt.',
        version => 19, deployed => 410, replicas => 0, rps => 0, p95 => 0, errors => 0,
        cpu => resource(allocated => 24, base => 0.4, swing => 0.1, noise => 0.04, unit => 'CORES', step => 1, min => 2, max => 32),
        gpu => resource(allocated => 1, base => 0.0, swing => 0.0, noise => 0.0, unit => 'GPUS', model => 'NONE', step => 1, min => 1, max => 8),
        memory => resource(allocated => 128, base => 8, swing => 0.5, noise => 0.2, unit => 'GB', step => 8, min => 16, max => 256),
    },
);
my %BY_ID = map { $_->{id} => $_ } @SERVICES;

my @LOG_LINES = (
    ['info', sub { sprintf 'stream opened tenant=t_%04d run=r_%05d', $_[0], $_[1] }],
    ['info', sub { sprintf 'tool call dispatched worker=%s-%d queue_ms=%d', $_[3], $_[0] % 40, $_[1] % 90 }],
    ['info', sub { sprintf 'tokens flushed bytes=%d region=%s', $_[0] * 7, $_[3] }],
    ['warn', sub { sprintf 'retrying run=r_%05d attempt=%d reason=upstream_timeout', $_[1], $_[2] % 3 + 2 }],
    ['info', sub { sprintf 'health check ok region=%s p50_ms=%d', $_[3], $_[2] % 60 + 12 }],
    ['debug', sub { sprintf 'lease renewed gpu=%s-%d ttl=%ds', $_[3], $_[0] % 8, 30 + $_[1] % 60 }],
    ['error', sub { sprintf 'worker %s-%d exited code=137 (oom), rescheduling', $_[3], $_[0] % 40 }],
);
my @REGIONS = qw(iad fra sin);

my $start = floor(time() / $STEP) * $STEP;    # the clock at the last sample
my $index = 0;                                # samples taken so far

sub hhmm { my @t = localtime($_[0]); sprintf '%02d:%02d', $t[2], $t[1] }
sub day_of { my @t = localtime($_[0]); $t[7] + 1000 * $t[5] }

sub demand_at {
    my ($r, $n) = @_;
    my $phase = 2 * 3.14159265 * ($n % 96) / 96;
    my $rise = ($r->{rise} // 0) * $r->{base} * (($n % 192) / 192);
    my $v = $r->{base} + $r->{swing} * sin($phase) + $rise * 0.25 + $r->{noise} * (rand() * 2 - 1) * 2;
    return $v < 0 ? 0 : $v;
}

sub sample {
    my ($n) = @_;
    for my $s (@SERVICES) {
        for my $key (qw(cpu gpu memory)) {
            my $r = $s->{$key};
            my $prev = @{ $r->{demand} } ? $r->{demand}[-1] : demand_at($r, $n);
            my $next = $prev * 0.55 + demand_at($r, $n) * 0.45;
            push @{ $r->{demand} }, $next;
            shift @{ $r->{demand} } while @{ $r->{demand} } > $SAMPLES;
        }
        my $load = $s->{status} eq 'paused' ? 0 : 1;
        push @{ $s->{rps_history} }, int($load * $s->{rps} * (0.75 + 0.25 * sin(2 * 3.14159265 * ($n % 96) / 96)) * (0.92 + rand() * 0.16));
        push @{ $s->{p95_history} }, int($load * $s->{p95} * (0.85 + rand() * 0.35));
        shift @{ $s->{rps_history} } while @{ $s->{rps_history} } > $SAMPLES;
        shift @{ $s->{p95_history} } while @{ $s->{p95_history} } > $SAMPLES;
        $s->{deployed}++ if $n > 0;
        if ($s->{rolling}) {
            $s->{rolling}--;
            unless ($s->{rolling}) {
                $s->{status} = 'active';
                $s->{version}++;
                $s->{deployed} = 0;
                $_->{status} = 'superseded' for @{ $s->{deploy_log} };
                unshift @{ $s->{deploy_log} }, { version => 'v' . $s->{version}, at => $n, who => 'Ines Varga', note => 'Rolled out from the console', status => 'live' };
            }
        }
        if ($load) {
            my ($level, $format) = @{ $LOG_LINES[ int(rand(@LOG_LINES)) ] };
            my $text = $format->((map { int(rand(9000)) + 100 } 1 .. 3), $REGIONS[ int(rand(@REGIONS)) ]);
            push @{ $s->{logs} }, { at => $n, level => $level, text => $text };
            shift @{ $s->{logs} } while @{ $s->{logs} } > 40;
        }
    }
}

# A day of history to begin with.
for my $s (@SERVICES) {
    $s->{deploy_log} = [
        map { { version => 'v' . ($s->{version} - $_), at => -($_ * 70 + 9), who => ('Ines Varga', 'Tomas Reyes', 'Priya Nair')[ $_ % 3 ], note => ('Bump runtime to 0.31', 'Raise stream buffer', 'Retry budget per tenant', 'Pin tokenizer build')[ $_ % 4 ], status => 'superseded' } } 1 .. 4
    ];
    my %latest = ('token-relay' => 'Stream tokens over QUIC', 'vector-index' => 'Rebuild shards in the background', 'eval-scheduler' => 'Lease GPUs by the minute', 'artifact-store' => 'Signed URLs expire after 15 minutes', 'webhook-fanout' => 'Dead-letter queue per endpoint', 'sandbox-pool' => 'Kernel 6.9 microVM image');
    unshift @{ $s->{deploy_log} }, { version => 'v' . $s->{version}, at => -$s->{deployed}, who => ('Tomas Reyes', 'Priya Nair', 'Ines Varga')[ $s->{version} % 3 ], note => $latest{ $s->{id} }, status => 'live' };
}
sample($_) for -($SAMPLES - 1) .. 0;
srand();

sub when_of {
    my ($n) = @_;
    my $t = $start + ($n - $index) * $STEP;
    my $now = $start;
    my $day = day_of($t) == day_of($now) ? 'TODAY' : day_of($t) == day_of($now - 86400) ? 'YESTERDAY' : 'EARLIER';
    return "$day, " . hhmm($t);
}

sub ago {
    my ($samples) = @_;
    my $minutes = $samples * 15;
    return 'just now' if $minutes < 1;
    return "${minutes}m ago" if $minutes < 60;
    my $hours = int($minutes / 60);
    return "${hours}h ago" if $hours < 48;
    return int($hours / 24) . 'd ago';
}

sub out {
    my @times = map { when_of($index - ($SAMPLES - 1) + $_) } 0 .. $SAMPLES - 1;
    my @axis;
    for (my $i = 0; $i < $SAMPLES; $i += 12) {
        my $t = $start - ($SAMPLES - 1 - $i) * $STEP;
        push @axis, { index => $i + 1, text => $i == $SAMPLES - 1 ? 'NOW' : hhmm($t) };
    }
    my @services;
    for my $s (@SERVICES) {
        my (%resources, %history);
        for my $key (qw(cpu gpu memory)) {
            my $r = $s->{$key};
            my $alloc = $r->{allocated};
            my @util = map { my $u = $alloc ? 100 * $_ / $alloc : 0; $u > 100 ? 100 : int($u + 0.5) } @{ $r->{demand} };
            my $used = $r->{demand}[-1];
            $used = $alloc if $used > $alloc;
            $history{$key} = \@util;
            $resources{$key} = {
                allocated => $alloc,
                used => $key eq 'memory' ? sprintf('%.1f', $used) + 0 : sprintf('%.1f', $used) + 0,
                util => $util[-1],
                unit => $r->{unit},
                step => $r->{step},
                min => $r->{min},
                max => $r->{max},
                label => $key eq 'gpu' ? "$alloc\x{d7} $r->{model}" : $key eq 'memory' ? "$alloc GB" : "$alloc CORES",
                limit => $key eq 'memory' ? $alloc * 1024 : $alloc,
                limit_unit => $key eq 'memory' ? 'MB' : $r->{unit},
            };
        }
        $history{rps} = $s->{rps_history};
        $history{p95} = $s->{p95_history};
        my @logs = map { { time => (split /, /, when_of($_->{at}))[1], level => $_->{level}, text => $_->{text} } } reverse @{ $s->{logs} // [] };
        my @deploys = map { { version => $_->{version}, when => ago($index - $_->{at}), who => $_->{who}, note => $_->{note}, status => $_->{status} } } @{ $s->{deploy_log} }[0 .. 4];
        push @services, {
            id => $s->{id}, name => $s->{name}, status => $s->{status}, tags => $s->{tags},
            regions => $s->{regions}, description => $s->{description},
            version => 'v' . $s->{version}, deployed => ago($s->{deployed}), replicas => $s->{replicas},
            rps => $s->{rps_history}[-1], p95 => $s->{p95_history}[-1], errors => $s->{errors},
            resources => \%resources, history => \%history, logs => \@logs, deploys => \@deploys,
        };
    }
    return { available => JSON::PP::true, sample => JSON::PP::true, now => hhmm($start), times => \@times, axis => \@axis, services => \@services };
}

sub receive {
    my ($line) = @_;
    my $in = eval { $json->decode($line) } or return;
    my $msg = ref $in eq 'HASH' ? $in->{message} : undef;
    return unless ref $msg eq 'HASH';
    if (ref $msg->{limit} eq 'HASH') {
        my $s = $BY_ID{ $msg->{limit}{service} // '' } or return;
        my $r = $s->{ $msg->{limit}{resource} // '' };
        return unless ref $r eq 'HASH' && exists $r->{allocated};
        my $by = ($msg->{limit}{by} // 0) < 0 ? -1 : 1;
        my $next = $r->{allocated} + $by * $r->{step};
        $r->{allocated} = $next if $next >= $r->{min} && $next <= $r->{max};
    }
    elsif (defined $msg->{deploy} && !ref $msg->{deploy}) {
        my $s = $BY_ID{ $msg->{deploy} } or return;
        return if $s->{rolling};
        $s->{status} = 'deploying';
        $s->{rolling} = 3;
    }
}

my $select = IO::Select->new(\*STDIN);
my $buffer = '';
my $next = time() + $TICK;
print $json->encode(out()), "\n";
while (1) {
    my $wait = $next - time();
    if ($wait <= 0) {
        $index++;
        $start += $STEP;
        sample($index);
        print $json->encode(out()), "\n";
        $next += $TICK;
        next;
    }
    if ($select->can_read($wait)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        my $changed = 0;
        while ($buffer =~ s/^([^\n]*)\n//) {
            receive($1);
            $changed = 1;
        }
        print $json->encode(out()), "\n" if $changed;
    }
}

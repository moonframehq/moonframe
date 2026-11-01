#!/usr/bin/env perl
# What runs on this machine, for Lantern's process monitor. Prints one JSON
# line every two seconds (as `iostat` ticks) and after each message; reads
# the app's messages, one `{"from": ..., "message": ...}` per line, on stdin.
# Exits when the app closes stdin. macOS: ps, iostat, vm_stat, sysctl.
#
# Output (`app.procs`):
#   available   false with `reason` when the tools are missing
#   cpu         busy share of all cores, 0 to 100; `user` and `system` split it
#   history     the last 60 `cpu` readings, oldest first
#   load        { 1m, 5m, 15m } load averages
#   memory      { used, total, active, wired, compressed, swap } in bytes,
#               and `pressure`: "normal", "warning" or "critical"
#   groups      processes grouped by name, the busiest first (or the largest
#               with sort = "memory"), at most 80, matching `query`:
#               { name, pid (the busiest member), count, cpu (% of one core,
#               summed), mem (bytes), open, children (when open: up to 12
#               { pid, name, cpu, mem }) }
#   total       how many processes match; `query`, `sort` as asked
#
# Messages (any combination of keys):
#   query = "<text>"        keep processes whose name or pid contains it
#   sort = "cpu" | "memory"
#   expand = "<name>"       show or hide that group's members

use strict;
use warnings;
use JSON::PP ();
use IO::Select;

$| = 1;

my $MAX_GROUPS   = 80;
my $MAX_CHILDREN = 12;
my $HISTORY      = 60;

my $json = JSON::PP->new->canonical->utf8(1);
my %state = (query => '', sort => 'cpu', open => {});
my @history;
my %cpu = (cpu => 0, user => 0, system => 0, load => [0, 0, 0]);

sub run {
    my (@command) = @_;
    my $pid = open(my $out, '-|');
    return undef unless defined $pid;
    if ($pid == 0) {
        open STDERR, '>', '/dev/null';
        exec @command or exit 127;
    }
    local $/;
    my $text = <$out>;
    close $out;
    return $? == 0 ? $text : undef;
}

sub round1 { return int($_[0] * 10 + 0.5) / 10 }

sub memory {
    my $total = run('sysctl', '-n', 'hw.memsize') // 0;
    chomp $total;
    my $vm = run('vm_stat') // '';
    my ($page) = $vm =~ /page size of (\d+) bytes/;
    $page ||= 16384;
    my %pages;
    while ($vm =~ /^(.+?):\s+(\d+)\.?$/mg) { $pages{$1} = $2 }
    my $active     = ($pages{'Pages active'} // 0) * $page;
    my $wired      = ($pages{'Pages wired down'} // 0) * $page;
    my $compressed = ($pages{'Pages occupied by compressor'} // 0) * $page;
    my $anonymous  = ($pages{'Anonymous pages'} // 0) * $page;
    my $purgeable  = ($pages{'Pages purgeable'} // 0) * $page;
    # As Activity Monitor counts it: app memory, wired, compressed.
    my $used = $anonymous - $purgeable + $wired + $compressed;
    $used = $total if $total && $used > $total;
    my $swap = 0;
    if ((run('sysctl', '-n', 'vm.swapusage') // '') =~ /used = ([\d.]+)M/) {
        $swap = int($1 * 1024 * 1024);
    }
    my $level = run('sysctl', '-n', 'kern.memorystatus_vm_pressure_level') // 1;
    chomp $level;
    my $pressure = $level >= 4 ? 'critical' : $level >= 2 ? 'warning' : 'normal';
    return {
        used => $used + 0, total => $total + 0, active => $active, wired => $wired,
        compressed => $compressed, swap => $swap, pressure => $pressure,
    };
}

sub processes {
    my $text = run('ps', '-Axo', 'pid=,pcpu=,rss=,comm=') // '';
    my @all;
    for my $line (split /\n/, $text) {
        next unless $line =~ /^\s*(\d+)\s+([\d.]+)\s+(\d+)\s+(.+?)\s*$/;
        my ($pid, $pcpu, $rss, $comm) = ($1, $2, $3, $4);
        my $name = $comm =~ s{.*/}{}r;
        next if $name eq '';
        push @all, { pid => $pid + 0, cpu => $pcpu + 0, mem => $rss * 1024, name => $name };
    }
    return \@all;
}

sub groups {
    my ($all) = @_;
    my $query = lc $state{query};
    my %by;
    my $total = 0;
    for my $proc (@$all) {
        next if length $query && index(lc $proc->{name}, $query) < 0 && index($proc->{pid}, $query) < 0;
        $total++;
        my $group = $by{$proc->{name}} //= { name => $proc->{name}, count => 0, cpu => 0, mem => 0, members => [] };
        $group->{count}++;
        $group->{cpu} += $proc->{cpu};
        $group->{mem} += $proc->{mem};
        push @{ $group->{members} }, $proc;
    }
    my $key = $state{sort} eq 'memory' ? 'mem' : 'cpu';
    my @sorted = sort { $b->{$key} <=> $a->{$key} || $a->{name} cmp $b->{name} } values %by;
    splice @sorted, $MAX_GROUPS if @sorted > $MAX_GROUPS;
    my @out;
    for my $group (@sorted) {
        my @members = sort { $b->{$key} <=> $a->{$key} } @{ $group->{members} };
        my $open = $state{open}{ $group->{name} } && $group->{count} > 1 ? JSON::PP::true : JSON::PP::false;
        my %row = (
            name => $group->{name}, pid => $members[0]{pid}, count => $group->{count},
            cpu => round1($group->{cpu}), mem => $group->{mem}, open => $open,
        );
        if ($open) {
            my @children = map { { pid => $_->{pid}, name => $_->{name}, cpu => round1($_->{cpu}), mem => $_->{mem} } }
                @members[0 .. ($#members < $MAX_CHILDREN - 1 ? $#members : $MAX_CHILDREN - 1)];
            $row{children} = \@children;
        }
        push @out, \%row;
    }
    return (\@out, $total);
}

sub emit {
    my ($groups, $total) = groups(processes());
    print $json->encode({
        available => JSON::PP::true,
        cpu => $cpu{cpu}, user => $cpu{user}, system => $cpu{system}, load => $cpu{load},
        history => [@history], memory => memory(), groups => $groups, total => $total,
        query => $state{query}, sort => $state{sort},
    }), "\n";
}

sub receive {
    my ($line) = @_;
    my $msg = eval { $json->decode($line) } or return;
    $msg = $msg->{message} if ref $msg eq 'HASH' && exists $msg->{message};
    return unless ref $msg eq 'HASH';
    $state{query} = $msg->{query} if defined $msg->{query} && !ref $msg->{query};
    $state{sort} = $msg->{sort} if defined $msg->{sort} && ($msg->{sort} eq 'cpu' || $msg->{sort} eq 'memory');
    if (defined $msg->{expand} && !ref $msg->{expand}) {
        my $name = $msg->{expand};
        if ($state{open}{$name}) { delete $state{open}{$name} } else { $state{open}{$name} = 1 }
    }
}

# iostat prints a line every two seconds: "us sy id 1m 5m 15m".
my $iostat_pid = open(my $iostat, '-|', 'iostat', '-n0', '-w2');
unless ($iostat_pid) {
    print $json->encode({ available => JSON::PP::false, reason => 'iostat is not available' }), "\n";
    # Never exit on an error: wait for the app to close stdin.
    1 while <STDIN>;
    exit 0;
}

my $select = IO::Select->new(\*STDIN, $iostat);
my ($input, $stats) = ('', '');
my $first = 1;
emit();
while (1) {
    my $changed = 0;
    for my $handle ($select->can_read) {
        if ($handle == \*STDIN || fileno($handle) == fileno(STDIN)) {
            my $got = sysread STDIN, $input, 65536, length $input;
            if (!$got) { kill 'TERM', $iostat_pid; exit 0 }
            while ($input =~ s/^([^\n]*)\n//) { receive($1); $changed = 1 }
        } else {
            my $got = sysread $iostat, $stats, 4096, length $stats;
            if (!$got) { $select->remove($iostat); next }
            while ($stats =~ s/^([^\n]*)\n//) {
                my $line = $1;
                next unless $line =~ /^\s*(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)/;
                # The first reading averages since boot: skip it.
                if ($first) { $first = 0; next }
                %cpu = (cpu => 100 - $3, user => $1 + 0, system => $2 + 0, load => [$4 + 0, $5 + 0, $6 + 0]);
                push @history, 100 - $3;
                shift @history while @history > $HISTORY;
                $changed = 1;
            }
        }
    }
    emit() if $changed;
}

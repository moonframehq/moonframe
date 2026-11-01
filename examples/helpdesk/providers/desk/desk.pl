#!/usr/bin/env perl
# Tandem's help desk, shared as `app.desk`: SAMPLE DATA read from sample.json
# beside this script, kept in memory (each launch starts over), and changed by
# the app's messages. Prints one JSON line whenever it changes; reads the
# app's messages, one `{"from": ..., "message": ...}` per line, on stdin.
# Exits when the app closes stdin.
#
# Output (`app.desk`):
#   assistant, workspace { team, company }, me { name, email }
#   week      { total, share, first_reply, split = { { id, label, count } } }
#   tickets   as in sample.json: { id, kind, priority, age, minutes, title,
#             requester { name, team }, source, status, status_label, mine,
#             created, device, summary, sla, watchers, check?, earlier?, thread }
#   counts    { all, open, mine, waiting }
#
# Messages (one key each):
#   watch = id                 a ticket is on screen: a check step still
#                              running finishes a few seconds later
#   act = id                   run the check's suggestion (its primary button)
#   reply = { id, text, internal }  add a reply, or an internal note, to the thread
#   new = true                 open a blank ticket at the top of the inbox
#
# Point this at your help desk's API to make it real.

use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use Time::HiRes qw(time);

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);

open my $fh, '<:raw', 'sample.json' or do {
    print qq({"available":false,"reason":"sample.json is missing"}\n);
    1 while <STDIN>;
    exit 0;
};
my $desk = do { local $/; JSON::PP->new->utf8->decode(<$fh>) };
close $fh;

my $next_number = 1240;
# Things that finish later: { at, run } with `run` a closure.
my @timers;

sub find {
    my ($id) = @_;
    return unless defined $id && !ref $id;
    for (@{ $desk->{tickets} }) { return $_ if $_->{id} eq $id }
    return;
}

sub later {
    my ($seconds, $run) = @_;
    push @timers, { at => time + $seconds, run => $run };
}

sub say_in {
    my ($ticket, $text) = @_;
    push @{ $ticket->{thread} }, { from => 'assistant', age => 'just now', text => $text };
}

# Finish each running step of a ticket's check, one after the other.
sub watch {
    my ($ticket) = @_;
    my $check = $ticket->{check} or return;
    return if $check->{watched}++;
    my $delay = 0;
    for my $step (@{ $check->{steps} }) {
        next unless $step->{state} eq 'running';
        $delay += 12;
        later($delay, sub {
            $step->{state} = 'done';
            $step->{label} = $step->{done_label} if $step->{done_label};
        });
    }
}

# Run the suggestion: a new step that runs, then the assistant says so.
sub act {
    my ($ticket) = @_;
    my $check = $ticket->{check} or return;
    return if $check->{acting} || $check->{acted};
    $check->{acting} = JSON::PP::true;
    my $name = $ticket->{requester}{name} =~ s/ .*//r;
    my $step = { label => "$check->{primary}…", state => 'running' };
    push @{ $check->{steps} }, $step;
    $ticket->{status} = 'progress';
    $ticket->{status_label} = 'In progress';
    later(4, sub {
        $step->{state} = 'done';
        $step->{label} = "$check->{primary}: done";
        delete $check->{acting};
        $check->{acted} = JSON::PP::true;
        say_in($ticket, "All done, $name. I ran \"$check->{suggestion}\" for you through $check->{via}. "
            . "Let me know if things still feel off and I'll bring in the IT team.");
    });
}

sub receive {
    my ($line) = @_;
    my $value = eval { $json->decode($line) };
    my $msg = ref $value eq 'HASH' ? $value->{message} : undef;
    return unless ref $msg eq 'HASH';
    if (defined $msg->{watch} and my $watched = find($msg->{watch})) {
        watch($watched);
    }
    elsif (defined $msg->{act} and my $acted = find($msg->{act})) {
        act($acted);
    }
    elsif (ref $msg->{reply} eq 'HASH' and my $ticket = find($msg->{reply}{id})) {
        my $text = $msg->{reply}{text};
        return unless defined $text && !ref $text && $text =~ /\S/;
        push @{ $ticket->{thread} }, {
            from => $msg->{reply}{internal} ? 'note' : 'agent',
            name => $desk->{me}{name},
            age => 'just now',
            text => substr($text, 0, 2000),
        };
    }
    elsif ($msg->{new}) {
        my $id = 'INC-' . $next_number++;
        unshift @{ $desk->{tickets} }, {
            id => $id, kind => 'Incident', priority => 'Low', age => 'now', minutes => 0,
            title => 'New ticket', requester => { name => $desk->{me}{name}, team => 'IT' },
            source => 'web', status => 'open', status_label => 'Open', mine => JSON::PP::true,
            created => 'just now', device => { name => 'Not set', detail => '' },
            summary => 'A blank ticket opened from the inbox. Describe the problem in a reply.',
            sla => [
                { left => '1h 00m', label => 'First response', due => 'in 1 hour', used => 0, urgent => JSON::PP::false },
                { left => '8h 00m', label => 'Resolution', due => 'in 8 hours', used => 0, urgent => JSON::PP::false },
            ],
            watchers => [ $desk->{me}{name} ], thread => [],
        };
    }
}

sub out {
    my %counts = (all => 0, open => 0, mine => 0, waiting => 0);
    for (@{ $desk->{tickets} }) {
        $counts{all}++;
        $counts{open}++ if $_->{status} ne 'resolved';
        $counts{mine}++ if $_->{mine} && $_->{status} ne 'resolved';
        $counts{waiting}++ if $_->{status} eq 'waiting';
    }
    return { %$desk, available => JSON::PP::true, counts => \%counts };
}

my $last = '';
my $select = IO::Select->new(\*STDIN);
my $buffer = '';
while (1) {
    my $now = time;
    for my $timer (grep { $_->{at} <= $now } @timers) { $timer->{run}->() }
    @timers = grep { $_->{at} > $now } @timers;
    my $text = $json->encode(out());
    if ($text ne $last) {
        print $text, "\n";
        $last = $text;
    }
    my ($soonest) = sort { $a <=> $b } map { $_->{at} } @timers;
    my $wait = defined $soonest ? ($soonest - time > 0 ? $soonest - time : 0) : 60;
    if ($select->can_read($wait)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        receive($1) while $buffer =~ s/^([^\n]*)\n//;
    }
}

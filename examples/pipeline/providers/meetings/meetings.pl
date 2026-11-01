#!/usr/bin/env perl
# Tally's meetings, kept in a JSON file, with today's date and the time now
# (a quarter hour at a time), which views can't read themselves. Prints one
# JSON line whenever that changes; reads the app's messages, one
# `{"message": ...}` per line, on stdin. Exits when the app closes stdin.
#
# Output (`app.meetings`):
#   today     "YYYY-MM-DD"
#   now       { date, hour } (hour as 13.25 for 1:15 PM)
#   me        the signed-in person's name
#   meetings  { id, title, date, start, finish (hours), mark (video, meet,
#               chat, call, grid), host, related, created = { date, time,
#               zone }, participants = { names }, agenda, link, notes },
#             by date and time
#
# Messages (one key each):
#   reschedule = id                      the same time, a day later
#   schedule = { date, start, title? }   a new hour-long meeting
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/meetings.json (by default
# under ~/.local/share), started the first time with sample meetings around
# the current week.

use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use POSIX qw(strftime floor);
use Time::Local qw(timegm);

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);
my $HOME = $ENV{HOME} // '';
my $DATA = (($ENV{XDG_DATA_HOME} // '') =~ m{^/} ? $ENV{XDG_DATA_HOME} : "$HOME/.local/share")
    . '/moonframe/' . (($ENV{MOONFRAME_PROFILE} // 'pipeline') =~ s/[^A-Za-z0-9_-]/-/gr);
my $FILE = "$DATA/meetings.json";
my $ME = 'Maya Kim';

# A date "YYYY-MM-DD" moved by some days (computed at noon UTC: no DST).
sub shift_date {
    my ($date, $days) = @_;
    my ($y, $m, $d) = $date =~ /^(\d{4})-(\d\d)-(\d\d)$/ or return $date;
    return strftime('%Y-%m-%d', gmtime(timegm(0, 0, 12, $d, $m - 1, $y) + $days * 86400));
}

sub long_date {
    my ($date) = @_;
    my ($y, $m, $d) = $date =~ /^(\d{4})-(\d\d)-(\d\d)$/ or return $date;
    return strftime('%B %-d, %Y', gmtime(timegm(0, 0, 12, $d, $m - 1, $y)));
}

sub today { strftime('%Y-%m-%d', localtime) }

sub sample {
    my @now = localtime;
    my $sunday = shift_date(today(), -$now[6]);
    my $id = 0;
    my $meeting = sub {
        my ($day, $start, $finish, $title, $mark, $related, $people, %more) = @_;
        return {
            id => 'm' . ++$id, title => $title, date => shift_date($sunday, $day),
            start => $start, finish => $finish, mark => $mark, host => $more{host} // $ME,
            related => $related,
            created => { date => long_date(shift_date($sunday, $day - 3)),
                time => $more{at} // '11:58 PM', zone => 'PST' },
            participants => $people,
            agenda => $more{agenda} // "Walk through where $related stands, agree on the next step and who owns it.",
            link => $more{link} // 'https://meet.tally.example/' . lc($related =~ s/\W+/-/gr),
            notes => $more{notes} // 'Share the latest numbers before the call.',
        };
    };
    return {
        next => 100,
        meetings => [
            $meeting->(0, 10, 12, 'Platform roadmap review', 'grid', 'Lumen Retail',
                ['Tess Okafor', 'Jonah Price', 'Lena Brooks'],
                agenda => 'Align on the renewal scope for 2027: which modules stay, the timeline, and what procurement needs before signing.',
                notes => 'Bring the usage report and the revised pricing deck.'),
            $meeting->(1, 8, 9, 'Product demo - Kestrel Airways', 'video', 'Kestrel Airways',
                ['Ravi Desai', 'Owen Hart']),
            $meeting->(1, 12, 13, 'Discovery call - new lead', 'grid', 'Bluefin Studio',
                ['Owen Hart', 'Priya Nair'], at => '9:14 AM'),
            $meeting->(2, 10, 11, 'Quarterly business review', 'meet', 'Fernhill Co-op',
                ['Tess Okafor', 'Marco Silva']),
            $meeting->(2, 14, 15, 'Security audit discussion', 'video', 'Quarry Data',
                ['Sami Lund', 'Ada Fenwick'], host => 'Sami Lund'),
            $meeting->(3, 12, 13, 'Discovery call - Granary Foods', 'grid', 'Granary Foods',
                ['Tess Okafor', 'Lena Brooks']),
            $meeting->(3, 12.5, 13.5, 'Pricing sync', 'call', 'Vesper Hotels',
                ['Ravi Desai'], host => 'Ravi Desai'),
            $meeting->(4, 9, 10, 'Follow-up call - discussion', 'meet', 'Ferro Logistics',
                ['Ravi Desai', 'Marco Silva']),
            $meeting->(4, 15, 17, 'Schedule business review', 'chat', 'Ashby College',
                ['Tess Okafor', 'Priya Nair', 'Owen Hart']),
            $meeting->(5, 11, 12, 'Contract redlines', 'video', 'Lumen Retail',
                ['Jonah Price', 'Lena Brooks']),
            $meeting->(6, 9.5, 10.5, 'Weekly pipeline sync', 'chat', 'Tally team',
                ['Sami Lund', 'Ravi Desai', 'Tess Okafor'],
                link => 'https://chat.tally.example/pipeline'),
            $meeting->(8, 10, 12, 'Product roadmap review', 'grid', 'Pinewood Labs',
                ['Sebastian Gray', 'Nathan Phillips', 'Ben Scott'],
                agenda => 'This meeting will focus on aligning the team on Q1-Q2 2027 product priorities, timelines, and dependencies to prepare for the rollout to three sites.',
                link => 'https://chat.tally.example/pinewood',
                notes => 'Finalize roadmap priorities before the rollout, share the latest mockups and metrics.'),
            $meeting->(8, 14, 15, 'Onboarding kickoff', 'meet', 'Inkwell Press',
                ['Sami Lund', 'Ada Fenwick']),
            $meeting->(9, 9, 10, 'Field licences demo', 'video', 'Solstice Power',
                ['Sami Lund', 'Priya Nair']),
            $meeting->(10, 13, 14, 'Renewal check-in', 'call', 'Wren Supply',
                ['Ravi Desai', 'Marco Silva']),
            $meeting->(11, 10, 11, 'Campus licence Q&A', 'meet', 'Ashby College',
                ['Tess Okafor', 'Owen Hart']),
        ],
    };
}

my $store;

sub load {
    if (open my $fh, '<:encoding(UTF-8)', $FILE) {
        local $/;
        $store = eval { $json->decode(<$fh>) };
        close $fh;
    }
    if (ref $store ne 'HASH' || ref $store->{meetings} ne 'ARRAY') {
        $store = sample();
        save();
    }
}

sub save {
    system 'mkdir', '-p', $DATA unless -d $DATA;
    open my $fh, '>:encoding(UTF-8)', "$FILE.tmp" or return;
    print $fh JSON::PP->new->canonical->pretty->encode($store);
    close $fh;
    rename "$FILE.tmp", $FILE;
}

sub find {
    my ($id) = @_;
    return undef unless defined $id && !ref $id;
    return (grep { $_->{id} eq $id } @{ $store->{meetings} })[0];
}

sub out {
    my @now = localtime;
    my $hour = $now[2] + floor($now[1] / 15) / 4;
    my @meetings = sort { $a->{date} cmp $b->{date} || $a->{start} <=> $b->{start} } @{ $store->{meetings} };
    return {
        available => JSON::PP::true,
        today => today(),
        now => { date => today(), hour => $hour },
        me => $ME,
        meetings => \@meetings,
    };
}

sub receive {
    my ($line) = @_;
    my $value = eval { $json->decode($line) };
    my $msg = ref $value eq 'HASH' ? $value->{message} : undef;
    return unless ref $msg eq 'HASH';
    if (defined $msg->{reschedule} && (my $meeting = find($msg->{reschedule}))) {
        $meeting->{date} = shift_date($meeting->{date}, 1);
    }
    elsif (ref $msg->{schedule} eq 'HASH' && ($msg->{schedule}{date} // '') =~ /^\d{4}-\d\d-\d\d$/) {
        my $start = $msg->{schedule}{start};
        $start = 9 unless defined $start && !ref $start && $start =~ /^\d+(\.\d+)?$/ && $start < 23;
        my $title = $msg->{schedule}{title};
        $title = 'New meeting' unless defined $title && !ref $title && $title =~ /\S/;
        push @{ $store->{meetings} }, {
            id => 'm' . $store->{next}++, title => substr($title, 0, 80),
            date => $msg->{schedule}{date}, start => $start + 0, finish => $start + 1,
            mark => 'video', host => $ME, related => 'Tally team',
            created => { date => strftime('%B %-d, %Y', localtime), time => strftime('%-I:%M %p', localtime),
                zone => strftime('%Z', localtime) },
            participants => [], agenda => 'Added from the calendar.',
            link => 'https://meet.tally.example/new', notes => '',
        };
    }
    else {
        return;
    }
    save();
}

load();
my $last = '';
my $select = IO::Select->new(\*STDIN);
my $buffer = '';
while (1) {
    my $text = $json->encode(out());
    if ($text ne $last) {
        print $text, "\n";
        $last = $text;
    }
    if ($select->can_read(60)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        receive($1) while $buffer =~ s/^([^\n]*)\n//;
    }
}

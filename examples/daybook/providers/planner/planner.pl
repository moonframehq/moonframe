#!/usr/bin/env perl
# Daybook's tasks, events and notes, kept in a JSON file, and the dates the
# views need (views have no clock). Prints one JSON line whenever that
# changes; reads the app's messages, one `{"message": ...}` per line, on
# stdin. Exits when the app closes stdin.
#
# Output (`app.planner`):
#   today, selected   dates as YYYY-MM-DD; `selected` is the day shown
#   title, subtitle   "Today" (or the weekday) and "2026. October 31."
#   greeting          "Good evening"
#   days              two weeks from the selected day, for `day-strip`:
#                     { id, day, weekday, today, dot }
#   items             every task, event and note: { id, kind (task, event,
#                     note), title, date?, time?, when (the date as "Sat,
#                     Oct 31"), state (open, important, progress, done),
#                     due, overdue, habit, tags, emoji?, favorite }
#   tags              { name, color }
#   folders           folder names
#   month             the selected day's month, Monday first: { title,
#                     weeks = { { { id, day, today, outside, count } x7 } } }
#   look              the window's look: classic, sepia or light
#
# Messages (one key each):
#   select = "YYYY-MM-DD" | "today"   show that day
#   shift = 1 | -1                    the next or previous day
#   month = 1 | -1                    the same day of the next or previous month
#   toggle = id                       a task done or open again
#   cycle = id                        open → important → in progress → open
#   add = "title"                     a task for the day shown
#   remove = id
#   look = "classic" | "sepia" | "light"
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/planner.json (by default
# under ~/.local/share), started with sample items the first time.

use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use POSIX qw(strftime mktime);

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);
my $HOME = $ENV{HOME} // '';
my $DATA = (($ENV{XDG_DATA_HOME} // '') =~ m{^/} ? $ENV{XDG_DATA_HOME} : "$HOME/.local/share")
    . '/moonframe/' . (($ENV{MOONFRAME_PROFILE} // 'daybook') =~ s/[^A-Za-z0-9_-]/-/gr);
my $FILE = "$DATA/planner.json";
my $LOOK = 30;    # seconds between looks at the clock and the file

# ---------------------------------------------------------------- dates ---

# YYYY-MM-DD of the day `offset` days after `date` (today if undefined).
sub day {
    my ($date, $offset) = @_;
    my @t = defined $date && $date =~ /^(\d{4})-(\d\d)-(\d\d)$/
        ? (0, 0, 12, $3, $2 - 1, $1 - 1900)
        : (0, 0, 12, (localtime)[3, 4, 5]);
    $t[3] += $offset // 0;
    return strftime('%Y-%m-%d', localtime mktime(@t));
}

sub parts {
    my ($date) = @_;
    my ($y, $m, $d) = $date =~ /^(\d{4})-(\d\d)-(\d\d)$/ or return;
    return localtime mktime(0, 0, 12, $d, $m - 1, $y - 1900);
}

sub label {
    my ($date, $format) = @_;
    my @t = parts($date) or return '';
    return strftime($format, @t);
}

# ----------------------------------------------------------------- data ---

sub sample {
    my $today = day();
    my $id = 0;
    my $item = sub { my %item = @_; $item{id} = 'i' . ++$id; \%item };
    return {
        tags => [
            { name => 'Work', color => '#3b6fd8' },
            { name => 'Health', color => '#2f9e5b' },
            { name => 'Ideas', color => '#8b5cf6' },
            { name => 'Home', color => '#d97706' },
        ],
        folders => ['Work', 'Personal'],
        items => [
            $item->(kind => 'task', title => 'Water the plants', date => $today, done => JSON::PP::true),
            $item->(kind => 'task', title => 'Finish the quarterly review', date => $today, time => '17:00',
                state => 'progress', tags => ['Work']),
            $item->(kind => 'task', title => 'Call Sam about the offsite', date => $today, time => '16:00',
                state => 'important', tags => ['Work']),
            $item->(kind => 'task', title => 'Morning run', date => $today, time => '07:30', habit => JSON::PP::true,
                tags => ['Health']),
            $item->(kind => 'event', title => 'Dinner with Lea', date => $today, time => '19:00'),
            $item->(kind => 'event', title => 'Design critique', date => $today, time => '10:00', tags => ['Work']),
            $item->(kind => 'task', title => 'Book the train to Porto', date => day($today, -1), state => 'important',
                tags => ['Home']),
            $item->(kind => 'task', title => 'Sketch the onboarding flow', tags => ['Ideas']),
            $item->(kind => 'task', title => 'Read the research notes'),
            $item->(kind => 'event', title => 'Team standup', date => day($today, 1), time => '09:30', tags => ['Work']),
            $item->(kind => 'task', title => 'Renew the passport', date => day($today, 3), tags => ['Home']),
            $item->(kind => 'event', title => 'Climbing', date => day($today, 4), time => '18:30', tags => ['Health']),
            $item->(kind => 'note', title => 'Getting started', emoji => "\x{2b50}", favorite => JSON::PP::true),
            $item->(kind => 'note', title => 'Trip to Porto', emoji => "\x{1f9f3}"),
            $item->(kind => 'note', title => 'Launch plan', emoji => "\x{1f680}"),
            $item->(kind => 'note', title => 'App ideas', emoji => "\x{1f4a1}"),
        ],
        next => $id + 1,
    };
}

my $store;

sub load {
    if (open my $fh, '<:encoding(UTF-8)', $FILE) {
        local $/;
        $store = eval { $json->decode(<$fh>) };
        close $fh;
    }
    if (ref $store ne 'HASH' || ref $store->{items} ne 'ARRAY') {
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
    return (grep { $_->{id} eq $id } @{ $store->{items} })[0];
}

# ----------------------------------------------------------------- loop ---

my $selected;

sub out {
    my $today = day();
    $selected //= $today;
    my $hour = (localtime)[2];
    my $now = strftime('%H:%M', localtime);
    my %busy = map { ($_->{date} // '') => 1 } grep { $_->{kind} ne 'note' && !$_->{done} } @{ $store->{items} };
    my @days = map {
        my $date = day($selected, $_);
        { id => $date, day => label($date, '%-d'), weekday => label($date, '%a'),
          today => ($date eq $today ? JSON::PP::true : JSON::PP::false),
          dot => ($busy{$date} ? JSON::PP::true : JSON::PP::false) }
    } 0 .. 13;
    my @items = map {
        my %item = %$_;
        $item{state} = $item{done} ? 'done' : ($item{state} // 'open');
        $item{done} = $item{done} ? JSON::PP::true : JSON::PP::false;
        $item{when} = $item{date} ? label($item{date}, '%a, %b %-d') : undef;
        my $late = $item{date} && ($item{date} lt $today
            || ($item{date} eq $today && $item{time} && $item{time} le $now));
        $item{overdue} = ($item{kind} eq 'task' && !$_->{done} && $item{date} && $item{date} lt $today)
            ? JSON::PP::true : JSON::PP::false;
        $item{due} = ($item{kind} ne 'note' && !$_->{done} && $item{time} && $late) ? JSON::PP::true : JSON::PP::false;
        $item{time} =~ s/^0// if defined $item{time};
        $item{tags} //= [];
        \%item;
    } @{ $store->{items} };
    my %count;
    $count{ $_->{date} }++ for grep { $_->{date} && $_->{kind} ne 'note' && !$_->{done} } @{ $store->{items} };
    my ($month_key) = $selected =~ /^(\d{4}-\d\d)/;
    my $first = "$month_key-01";
    my $wday = (parts($first))[6];             # 0 Sunday
    my $start = day($first, -(($wday + 6) % 7));  # back to Monday
    my @weeks;
    for my $week (0 .. 5) {
        my @row = map {
            my $date = day($start, $week * 7 + $_);
            { id => $date, day => label($date, '%-d'),
              today => ($date eq $today ? JSON::PP::true : JSON::PP::false),
              outside => (substr($date, 0, 7) ne $month_key ? JSON::PP::true : JSON::PP::false),
              count => $count{$date} // 0 }
        } 0 .. 6;
        last if $week >= 4 && $row[0]{outside};
        push @weeks, \@row;
    }
    return {
        available => JSON::PP::true,
        look => $store->{look} // 'classic',
        month => { title => label($first, '%B %Y'), weeks => \@weeks },
        today => $today,
        selected => $selected,
        title => ($selected eq $today ? 'Today' : label($selected, '%A')),
        subtitle => label($selected, '%Y. %B %-d.'),
        greeting => $hour < 5 ? 'Good night' : $hour < 12 ? 'Good morning' : $hour < 18 ? 'Good afternoon' : 'Good evening',
        days => \@days,
        items => \@items,
        tags => $store->{tags} // [],
        folders => $store->{folders} // [],
    };
}

sub receive {
    my ($line) = @_;
    my $value = eval { $json->decode($line) };
    my $msg = ref $value eq 'HASH' ? $value->{message} : undef;
    return unless ref $msg eq 'HASH';
    my $changed;
    if (defined $msg->{select} && !ref $msg->{select}) {
        $selected = $msg->{select} eq 'today' ? day() : day($msg->{select});
    }
    elsif (defined $msg->{shift} && $msg->{shift} =~ /^-?\d+$/) {
        $selected = day($selected, $msg->{shift});
    }
    elsif (defined $msg->{month} && $msg->{month} =~ /^-?\d+$/) {
        my ($y, $m, $d) = ($selected // day()) =~ /^(\d{4})-(\d\d)-(\d\d)$/;
        my $months = $y * 12 + $m - 1 + $msg->{month};
        ($y, $m) = (int($months / 12), $months % 12 + 1);
        # The 31st of a shorter month is its last day.
        $d = 28 if $d > 28 && day(sprintf('%04d-%02d-%02d', $y, $m, $d)) !~ /^\d{4}-\Q@{[sprintf '%02d', $m]}\E/;
        $selected = sprintf('%04d-%02d-%02d', $y, $m, $d);
    }
    elsif (defined $msg->{toggle} && (my $item = find($msg->{toggle}))) {
        $item->{done} = $item->{done} ? JSON::PP::false : JSON::PP::true;
        $changed = 1;
    }
    elsif (defined $msg->{cycle} && (my $task = find($msg->{cycle}))) {
        my %next = (open => 'important', important => 'progress', progress => 'open');
        $task->{state} = $next{ $task->{state} // 'open' } // 'open';
        $changed = 1;
    }
    elsif (defined $msg->{add} && !ref $msg->{add} && $msg->{add} =~ /\S/) {
        my $title = substr($msg->{add} =~ s/^\s+|\s+$//gr, 0, 200);
        my %task = (id => 'i' . $store->{next}++, kind => 'task', title => $title);
        # From the inbox: no date; else the day shown.
        $task{date} = $selected // day() unless $msg->{inbox};
        push @{ $store->{items} }, \%task;
        $changed = 1;
    }
    elsif (defined $msg->{look} && $msg->{look} =~ /^(classic|sepia|light)$/) {
        $store->{look} = $msg->{look};
        $changed = 1;
    }
    elsif (defined $msg->{remove}) {
        $store->{items} = [grep { $_->{id} ne $msg->{remove} } @{ $store->{items} }];
        $changed = 1;
    }
    save() if $changed;
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
    if ($select->can_read($LOOK)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        receive($1) while $buffer =~ s/^([^\n]*)\n//;
    }
}

#!/usr/bin/env perl
# Tally's deals, kept in a JSON file, and the figures the board shows.
# Prints one JSON line whenever that changes; reads the app's messages, one
# `{"message": ...}` per line, on stdin. Exits when the app closes stdin.
#
# Output (`app.deals`):
#   stages    { id, label, color, count, total }, in order
#   deals     { id, company, summary, amount, date, owner, stage, chance,
#               status (risk, won or absent), touched (days since contact) }
#   quota     { target, won, commit, days_left }
#   figures   { weighted, open, win_rate, win_delta, cycle_days, cycle_delta,
#               weighted_delta }
#   months    closed revenue by month, { label, value, future }
#   owners    { initials, name, color }
#
# Messages (one key each):
#   move = { id, stage }      put a deal in another stage
#   shift = { id, by = 1|-1 } the next or previous stage
#   won = id                  mark won (moves it to Closed won)
#   risk = id                 flag or unflag it as at risk
#   add = { stage, company? } a new deal in a stage
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/deals.json (by default under
# ~/.local/share), started with sample deals the first time.

use strict;
use warnings;
use JSON::PP ();
use IO::Select;

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);
my $HOME = $ENV{HOME} // '';
my $DATA = (($ENV{XDG_DATA_HOME} // '') =~ m{^/} ? $ENV{XDG_DATA_HOME} : "$HOME/.local/share")
    . '/moonframe/' . (($ENV{MOONFRAME_PROFILE} // 'pipeline') =~ s/[^A-Za-z0-9_-]/-/gr);
my $FILE = "$DATA/deals.json";

my @STAGES = (
    { id => 'lead', label => 'Lead', color => '#9a9aa2' },
    { id => 'qualified', label => 'Qualified', color => '#4b8df8' },
    { id => 'proposal', label => 'Proposal', color => '#a78bfa' },
    { id => 'negotiation', label => 'Negotiation', color => '#f5a524' },
    { id => 'won', label => 'Closed won', color => '#22c55e' },
);
my %INDEX = map { $STAGES[$_]{id} => $_ } 0 .. $#STAGES;

sub sample {
    my $id = 0;
    my $deal = sub {
        my ($company, $summary, $amount, $date, $owner, $stage, $chance, %more) = @_;
        return { id => 'd' . ++$id, company => $company, summary => $summary, amount => $amount,
            date => $date, owner => $owner, stage => $stage, chance => $chance, %more };
    };
    return {
        next => 100,
        deals => [
            $deal->('Bluefin Studio', 'Site rebuild and hosting', 22000, 'Dec 11', 'MK', 'lead', 10),
            $deal->('Cobalt Clinics', 'Booking portal trial', 41500, 'Jan 9', 'SL', 'lead', 15),
            $deal->('Granary Foods', 'Loyalty app scoping', 14000, 'Jan 21', 'TO', 'lead', 10),
            $deal->('Harbor Credit', 'Reporting add-on', 49000, 'Feb 3', 'RD', 'lead', 20),
            $deal->('Pinewood Labs', 'Team rollout, 3 sites', 68000, 'Dec 5', 'TO', 'qualified', 30),
            $deal->('Solstice Power', 'Field crew licences', 43000, 'Nov 27', 'SL', 'qualified', 35),
            $deal->('Marlow & Daughters', 'Brand refresh retainer', 26500, 'Dec 16', 'MK', 'qualified', 25),
            $deal->('Wren Supply', 'Seats upgrade', 17200, 'Dec 23', 'RD', 'qualified', 30),
            $deal->('Kestrel Airways', 'Crew rostering suite', 132000, 'Nov 29', 'RD', 'proposal', 50),
            $deal->('Ashby College', 'Campus licence', 91000, 'Dec 10', 'TO', 'proposal', 45),
            $deal->('Quarry Data', 'Warehouse migration', 61000, 'Nov 21', 'SL', 'proposal', 55),
            $deal->('Lumen Retail', 'Platform renewal', 98000, 'Nov 13', 'TO', 'negotiation', 70,
                status => 'risk', touched => 12),
            $deal->('Ferro Logistics', 'Fleet tracking expansion', 47500, 'Nov 25', 'RD', 'negotiation', 80,
                touched => 11),
            $deal->('Vesper Hotels', 'Point of sale integration', 70000, 'Nov 18', 'MK', 'negotiation', 75,
                touched => 14),
            $deal->('Inkwell Press', 'Enterprise plan', 118000, 'Oct 20', 'SL', 'won', 100, status => 'won'),
            $deal->('Fernhill Co-op', 'Annual renewal', 52000, 'Oct 17', 'TO', 'won', 100, status => 'won'),
            $deal->('Orbit Works', 'Seats expansion', 33500, 'Oct 8', 'MK', 'won', 100, status => 'won'),
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
    if (ref $store ne 'HASH' || ref $store->{deals} ne 'ARRAY') {
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
    return (grep { $_->{id} eq $id } @{ $store->{deals} })[0];
}

sub place {
    my ($deal, $stage) = @_;
    return unless defined $stage && exists $INDEX{$stage};
    $deal->{stage} = $stage;
    if ($stage eq 'won') {
        $deal->{status} = 'won';
        $deal->{chance} = 100;
    }
    elsif (($deal->{status} // '') eq 'won') {
        delete $deal->{status};
        $deal->{chance} = 50;
    }
}

sub out {
    my @deals = @{ $store->{deals} };
    my @stages = map {
        my $id = $_->{id};
        my @in = grep { $_->{stage} eq $id } @deals;
        my $total = 0;
        $total += $_->{amount} for @in;
        +{ %$_, count => scalar @in, total => $total }
    } @STAGES;
    my ($won, $weighted, $open) = (0, 0, 0);
    for (@deals) {
        if ($_->{stage} eq 'won') { $won += $_->{amount} }
        else { $weighted += $_->{amount} * ($_->{chance} // 0) / 100; $open++ }
    }
    my $target = 2_000_000;
    # Earlier wins this quarter, beside the ones on the board.
    $won += 1_160_000;
    return {
        available => JSON::PP::true,
        stages => \@stages,
        deals => \@deals,
        quota => { target => $target, won => $won, commit => 280_000, days_left => 34 },
        figures => {
            weighted => int($weighted + 2_640_000), open => $open + 25, weighted_delta => '+9.2%',
            win_rate => '31%', win_delta => '+4 pts', cycle_days => 38, cycle_delta => '+3 days',
        },
        months => [
            { label => 'Jul', value => 180, future => JSON::PP::false },
            { label => 'Aug', value => 260, future => JSON::PP::false },
            { label => 'Sep', value => 210, future => JSON::PP::false },
            { label => 'Oct', value => 330, future => JSON::PP::false },
            { label => 'Nov', value => 410, future => JSON::PP::false },
            { label => 'Dec', value => 380, future => JSON::PP::true },
        ],
        owners => [
            { initials => 'MK', name => 'Maya Kim', color => '#4b8df8' },
            { initials => 'SL', name => 'Sami Lund', color => '#ec4899' },
            { initials => 'RD', name => 'Ravi Desai', color => '#22c55e' },
            { initials => 'TO', name => 'Tess Okafor', color => '#f59e0b' },
        ],
    };
}

sub receive {
    my ($line) = @_;
    my $value = eval { $json->decode($line) };
    my $msg = ref $value eq 'HASH' ? $value->{message} : undef;
    return unless ref $msg eq 'HASH';
    if (ref $msg->{move} eq 'HASH' && (my $deal = find($msg->{move}{id}))) {
        place($deal, $msg->{move}{stage});
    }
    elsif (ref $msg->{shift} eq 'HASH' && (my $moved = find($msg->{shift}{id}))) {
        my $to = $INDEX{ $moved->{stage} } + (($msg->{shift}{by} // 0) <=> 0);
        place($moved, $STAGES[$to]{id}) if $to >= 0 && $to <= $#STAGES;
    }
    elsif (defined $msg->{won} && (my $won = find($msg->{won}))) {
        place($won, 'won');
    }
    elsif (defined $msg->{risk} && (my $risky = find($msg->{risk}))) {
        if (($risky->{status} // '') eq 'risk') { delete $risky->{status} }
        elsif (($risky->{status} // '') ne 'won') { $risky->{status} = 'risk' }
    }
    elsif (ref $msg->{add} eq 'HASH' && exists $INDEX{ $msg->{add}{stage} // '' }) {
        my $company = $msg->{add}{company};
        $company = 'New deal' unless defined $company && !ref $company && $company =~ /\S/;
        push @{ $store->{deals} }, { id => 'd' . $store->{next}++, company => substr($company, 0, 80),
            summary => 'Added from the board', amount => 10000, date => 'Dec 31', owner => 'MK',
            stage => $msg->{add}{stage}, chance => 10 };
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

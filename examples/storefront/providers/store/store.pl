#!/usr/bin/env perl
# Storefront's audit log, SAMPLE DATA: a few hundred store activities made
# up from templates with a fixed seed, so every launch shows the same log.
# Publishes them as one JSON line, then answers `{ "export": [ids] }` from
# the audit mod by writing those rows to a CSV file (in ~/Downloads when it
# exists, else ~/.cache/moonframe/storefront) and publishing where it went.
# Exits when the app closes stdin.
use strict;
use warnings;
use JSON::PP ();
use Time::Local qw(timegm);
use File::Path qw(make_path);

$| = 1;
my $json = JSON::PP->new->canonical;

# "Now" for the sample: the log ends here, whatever the clock says.
my $now = timegm(0, 20, 17, 30, 9, 2026);    # 2026-10-30 17:20 UTC

my @kinds = (
    { id => "orders",    label => "Orders",        icon => "file-text" },
    { id => "products",  label => "Products",      icon => "layers" },
    { id => "customers", label => "Customers",     icon => "user" },
    { id => "users",     label => "Users & roles", icon => "shield" },
    { id => "apps",      label => "Apps",          icon => "plug" },
    { id => "content",   label => "Content",       icon => "palette" },
    { id => "discounts", label => "Discounts",     icon => "tag" },
);

my @authors = (
    { id => "nadia",  name => "Nadia Okafor",   color => "#7fc8a9" },
    { id => "theo",   name => "Theo Lindqvist", color => "#f2a7b8" },
    { id => "priya",  name => "Priya Raman",    color => "#9ec5f0" },
    { id => "marcus", name => "Marcus Hale",    color => "#f4b183" },
);

# kind, subject pattern, verb, tags. %d is an order number, %s a name.
my @templates = (
    [ "orders",    'Order #%d',             "marked as",         [ "Fulfilled", "Refunded", "Paid", "Cancelled" ] ],
    [ "orders",    'Order #%d',             "moved to archived", [ "Archived" ] ],
    [ "products",  'Product "%s"',          "marked as",         [ "Active", "Draft" ] ],
    [ "products",  'Product "%s"',          "updated with",      [ "New variant", "New price", "New images" ] ],
    [ "customers", 'Customer "%s"',         "marked as",         [ "Added" ] ],
    [ "customers", 'Customer "%s"',         "updated with",      [ "New customer tags", "New email" ] ],
    [ "users",     'User "%s"',             "updated with",      [ "New permissions", "New role" ] ],
    [ "users",     'Role preset "%s"',      "updated with",      [ "New access rule" ] ],
    [ "apps",      'App "%s"',              "marked as",         [ "Installed" ] ],
    [ "apps",      'App "%s"',              "updated with",      [ "Removed", "New scopes" ] ],
    [ "content",   'Theme "%s"',            "marked as",         [ "Published" ] ],
    [ "content",   'Page "%s"',             "updated with",      [ "New content" ] ],
    [ "discounts", 'Discount "%s"',         "marked as",         [ "Active", "Expired" ] ],
);

my %names = (
    products  => [ "Harbor Hoodie", "Summer Tee", "Canvas Tote", "Linen Shirt", "Trail Cap", "Wool Beanie" ],
    customers => [ "Ada Moreno", "Kenji Sato", "Lena Fischer", "Omar Haddad", "Sofia Rossi", "Yuki Tanaka" ],
    users     => [ "Ines Duarte", "Rafael Costa", "Maya Brooks", "Elio Marchetti" ],
    roles     => [ "Support Agent", "Warehouse", "Store Manager" ],
    apps      => [ "Mailwise", "Paydock", "Shipwell", "Reviewly" ],
    content   => [ "Autumn Collection", "Coastline", "About us", "Shipping policy" ],
    discounts => [ "AUTUMN20", "WELCOME10", "FREESHIP", "VIP15" ],
);

# A small fixed-seed generator, so the sample never changes.
my $seed = 20261030;
sub rnd { my ($n) = @_; $seed = ($seed * 1103515245 + 12345) % 2147483648; return int($seed / 65536) % $n; }
sub pick { my ($list) = @_; return $list->[ rnd(scalar @$list) ]; }

sub stamp {
    my ($t) = @_;
    my ($s, $m, $h, $d, $mo, $y) = gmtime($t);
    my $ampm = $h < 12 ? "AM" : "PM";
    my $h12 = $h % 12 || 12;
    return (sprintf("%02d/%02d/%04d", $mo + 1, $d, $y + 1900), sprintf("%02d:%02d:%02d %s", $h12, $m, $s, $ampm));
}

my @activities;
my $today = $now - ($now % 86400);
# 145 in the last seven days (the sample's default filter), the rest over
# the seven weeks before.
for my $i (1 .. 232) {
    my $day = $i <= 145 ? rnd(7) : 7 + rnd(49);
    my $time = $today - $day * 86400 + 8 * 3600 + rnd(11 * 3600);
    $time -= 86400 if $time > $now;
    my $t = pick(\@templates);
    my ($kind, $pattern, $verb, $tags) = @$t;
    my $subject;
    if ($pattern =~ /%d/) {
        $subject = sprintf($pattern, 1000 + rnd(9000));
    } else {
        my $pool = $pattern =~ /^Role/ ? "roles" : $kind;
        $subject = sprintf($pattern, pick($names{$pool}));
    }
    my ($date, $clock) = stamp($time);
    push @activities, {
        kind    => $kind,
        subject => $subject,
        verb    => $verb,
        tag     => pick($tags),
        author  => pick(\@authors)->{id},
        time    => $time,
        date    => $date,
        clock   => $clock,
    };
}
@activities = sort { $a->{time} <=> $b->{time} } @activities;
my $n = 0;
$_->{id} = "a" . ++$n for @activities;
my %by_id = map { $_->{id} => $_ } @activities;
my %author_name = map { $_->{id} => $_->{name} } @authors;

my $export;
my $exports = 0;

sub publish {
    print $json->encode({
        available  => JSON::PP::true,
        sample     => JSON::PP::true,
        now        => $now,
        store      => { name => "Fernhill Supply", detail => "fernhill.shop" },
        kinds      => \@kinds,
        authors    => \@authors,
        activities => \@activities,
        ($export ? (export => $export) : ()),
    }) . "\n";
}

sub csv { my ($v) = @_; $v =~ s/"/""/g; return qq("$v"); }

sub write_csv {
    my ($ids) = @_;
    my $dir = -d "$ENV{HOME}/Downloads" ? "$ENV{HOME}/Downloads" : "$ENV{HOME}/.cache/moonframe/storefront";
    make_path($dir);
    my $path = "$dir/storefront-audit-log.csv";
    open my $fh, '>', $path or do {
        $export = { ok => JSON::PP::false, error => "Could not write $path: $!", seq => ++$exports };
        return;
    };
    print $fh "Activity,Author,Timestamp\n";
    my $count = 0;
    for my $id (@$ids) {
        my $a = $by_id{$id} or next;
        print $fh join(",", csv("$a->{subject} $a->{verb} $a->{tag}"), csv($author_name{ $a->{author} }), csv("$a->{date} $a->{clock}")), "\n";
        $count++;
    }
    close $fh;
    (my $shown = $path) =~ s/^\Q$ENV{HOME}\E/~/;
    $export = { ok => JSON::PP::true, path => $shown, count => $count, seq => ++$exports };
}

publish();
while (my $line = <STDIN>) {
    my $in = eval { $json->decode($line) } or next;
    my $msg = $in->{message};
    # Only the audit page may write files.
    if (($in->{from} // "") eq "audit" && ref $msg eq "HASH" && ref $msg->{export} eq "ARRAY") {
        write_csv($msg->{export});
    }
    publish();
}

#!/usr/bin/env perl
# msgvault's archive for the msgvault app, published as `app.vault`.
#
# Where the data comes from:
# - a msgvault archive, when $MSGVAULT_HOME/msgvault.db (else
#   ~/.msgvault/msgvault.db) exists and `sqlite3` is installed: the newest
#   messages are read with `sqlite3 -json`, never written (PRAGMA
#   query_only), and read again every minute;
# - else sample.json beside this file: SAMPLE data, a snapshot of
#   msgvault-gpui's demo archive, with times kept relative to now.
#
# What it publishes is shaped for the views: the sidebar's counts, labels
# and accounts, the list of the mailbox in view, and the open thread. Mods
# change the view by message: { mailbox = "inbox" | "label:Work" |
# "kind:whatsapp" | ... }, { account = id (0 for all) }, { query = "text" },
# { open = message id }. The view is kept in ~/.cache/moonframe/msgvault.
#
# `perl vault.pl --dump` prints a sample.json from the archive it finds
# (msgvault-fixture's demo archive made this one).
use strict;
use warnings;
use utf8;
use JSON::PP ();
use IO::Select;
use IO::Handle;
use File::Path qw(make_path);
use POSIX qw(floor);

binmode STDOUT, ':raw';
STDOUT->autoflush(1);

my $JSON = JSON::PP->new->canonical->utf8;
my $TRUE = JSON::PP::true;
my $FALSE = JSON::PP::false;
my $LIMIT = 2000;     # newest messages read from an archive
my $ROWS = 400;       # rows published for one list
my $REFRESH = 60;     # seconds between archive reads

# ---- where things are --------------------------------------------------

my $home = $ENV{HOME} // '';
my $vault_home = $ENV{MSGVAULT_HOME} || "$home/.msgvault";
my $db_path = "$vault_home/msgvault.db";
my $cache_dir = ($ENV{XDG_CACHE_HOME} || "$home/.cache") . '/moonframe/msgvault';
my $view_file = "$cache_dir/view.json";

sub have_sqlite {
    for my $dir (split /:/, $ENV{PATH} // '') {
        return 1 if -x "$dir/sqlite3";
    }
    return 0;
}

sub tilde {
    my ($path) = @_;
    $path =~ s/^\Q$home\E/~/ if $home;
    return $path;
}

# ---- formatting, as msgvault-gpui's format.rs --------------------------

my @WEEKDAYS = qw(Sunday Monday Tuesday Wednesday Thursday Friday Saturday);
my @DAYS = qw(Sun Mon Tue Wed Thu Fri Sat);
my @MONTHS = qw(Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec);

sub clock {
    my ($t) = @_;
    my @l = localtime $t;
    my $h = $l[2] % 12 || 12;
    return sprintf '%d:%02d %s', $h, $l[1], $l[2] < 12 ? 'AM' : 'PM';
}

sub day_number {
    my ($t) = @_;
    my @l = localtime $t;
    # Local noon of that day, so daylight saving changes never move it.
    return floor(POSIX::mktime(0, 0, 12, $l[3], $l[4], $l[5]) / 86400);
}

sub list_date {
    my ($t, $now) = @_;
    return '' unless $t;
    my $days = day_number($now) - day_number($t);
    my @l = localtime $t;
    my @n = localtime $now;
    return clock($t) if $days == 0;
    return 'Yesterday' if $days == 1;
    return $WEEKDAYS[$l[6]] if $days >= 2 && $days < 7;
    return "$MONTHS[$l[4]] $l[3]" if $l[5] == $n[5];
    return "$MONTHS[$l[4]] $l[3], " . ($l[5] + 1900);
}

sub full_date {
    my ($t) = @_;
    return '' unless $t;
    my @l = localtime $t;
    return "$DAYS[$l[6]], $MONTHS[$l[4]] $l[3], " . ($l[5] + 1900) . ' at ' . clock($t);
}

sub bytes {
    my ($n) = @_;
    my @units = qw(B KB MB GB TB);
    my ($value, $unit) = ($n, 0);
    while ($value >= 1024 && $unit < $#units) { $value /= 1024; $unit++ }
    return "$n B" if $unit == 0;
    return sprintf('%.0f %s', $value, $units[$unit]) if $value >= 100;
    return sprintf('%.1f %s', $value, $units[$unit]);
}

sub count {
    my ($n) = @_;
    1 while $n =~ s/^(\d+)(\d{3})/$1,$2/;
    return $n;
}

sub plural {
    my ($n, $one, $many) = @_;
    return count($n) . ' ' . ($n == 1 ? $one : $many);
}

sub fnv {
    my ($text) = @_;
    my $hash = 2166136261;
    my $bytes = $text;
    utf8::encode($bytes);
    for my $byte (unpack 'C*', $bytes) {
        $hash = (($hash ^ $byte) * 16777619) & 0xffffffff;
    }
    return $hash;
}

# A label's colour when it has none stored: format::label_color.
my @PALETTE = ('#4a86e8', '#8e63ce', '#16a766', '#f2a60c', '#e66550', '#2da2bb', '#d5608c', '#3dc789', '#7a8cff', '#c28b3c');
sub label_color {
    my ($name, $stored) = @_;
    return lc $stored if defined $stored && $stored =~ /^#[0-9a-fA-F]{6}$/;
    return $PALETTE[fnv($name) % @PALETTE];
}

my %SYSTEM = map { $_ => 1 } qw(INBOX SENT TRASH SPAM DRAFT DRAFTS UNREAD STARRED IMPORTANT CHAT);
my %STATE = map { $_ => 1 } qw(UNREAD STARRED INBOX SENT DRAFT IMPORTANT);

sub title_case {
    my ($word) = @_;
    return ucfirst lc $word;
}

sub display_label {
    my ($name) = @_;
    my $upper = uc $name;
    return title_case($1) if $upper =~ /^CATEGORY_(.+)$/;
    return title_case($upper) if $SYSTEM{$upper};
    return $name;
}

my %KINDS = (
    '' => 'Email', email => 'Email', imessage => 'iMessage', sms => 'SMS', mms => 'MMS',
    rcs => 'RCS', whatsapp => 'WhatsApp', fbmessenger => 'Messenger', teams => 'Teams',
    slack => 'Slack', discord => 'Discord', beeper => 'Beeper', google_chat => 'Google Chat',
    calendar_event => 'Calendar', meeting_transcript => 'Meetings', matrix => 'Matrix',
);
sub display_kind {
    my ($kind) = @_;
    return $KINDS{$kind} if exists $KINDS{$kind};
    (my $words = $kind) =~ s/_/ /g;
    return title_case($words);
}

sub kind_icon {
    my ($kind) = @_;
    return 'calendar-days' if $kind eq 'calendar_event';
    return 'message-square' if $kind eq 'meeting_transcript';
    return 'hash' if $kind =~ /^(slack|discord|teams|google_chat|matrix)$/;
    return 'messages-square';
}

sub account_icon {
    my ($kind) = @_;
    return 'mail' if $kind =~ /^(gmail|imap|fastmail|microsoft|outlook|mbox|emlx|pst)$/;
    return 'message-square' if $kind =~ /^(whatsapp|imessage|apple_messages|google_messages|sms|beeper|google_voice)$/;
    return 'hash' if $kind =~ /^(slack|discord|teams|google_chat|matrix)$/;
    return 'calendar-days' if $kind =~ /^(gcal|calendar)$/;
    return 'database';
}

# Initials and a hue (0..11) for an avatar, as gpui-kit's Avatar draws one.
sub initials {
    my ($name) = @_;
    my @words = grep { length } split / /, $name // '';
    my $text = join '', map { substr($_, 0, 1) } @words[0 .. ($#words < 1 ? $#words : 1)];
    $text = substr($name // '?', 0, 2) if length($text) == 1;
    $text = '?' if $text eq '';
    return uc $text;
}

sub hue {
    my ($initials) = @_;
    return fnv($initials) % 12;
}

sub clean {
    my ($text) = @_;
    return '' unless defined $text;
    $text =~ s/&#39;/'/g;
    $text =~ s/&quot;/"/g;
    $text =~ s/&lt;/</g;
    $text =~ s/&gt;/>/g;
    $text =~ s/&nbsp;/ /g;
    $text =~ s/&amp;/&/g;
    $text =~ s/\s+/ /g;
    $text =~ s/^ | $//g;
    return $text;
}

# A readable plain rendering of an HTML body: reader.rs's html_to_text.
sub html_to_text {
    my ($html) = @_;
    $html =~ s{<(style|script|head)\b.*?</\1\s*>}{}gis;
    $html =~ s{<li\b[^>]*>}{\n• }gi;
    $html =~ s{</?(br|p|div|tr|li|h[1-4]|blockquote|table)\b[^>]*>}{\n}gi;
    $html =~ s{</?(td|th)\b[^>]*>}{\t}gi;
    $html =~ s{<[^>]*>}{}g;
    $html =~ s/&nbsp;/ /g;
    $html =~ s/&lt;/</g;
    $html =~ s/&gt;/>/g;
    $html =~ s/&quot;/"/g;
    $html =~ s/&#39;/'/g;
    $html =~ s/&amp;/&/g;
    my @lines;
    for my $line (split /\n/, $html) {
        $line =~ s/\s+$//;
        $line =~ s/^\t+//;
        $line =~ s/\t+/  /g;
        next if $line eq '' && @lines && $lines[-1] eq '';
        push @lines, $line;
    }
    my $text = join "\n", @lines;
    $text =~ s/^\s+|\s+$//g;
    return $text;
}

sub attachment_icon {
    my ($mime) = @_;
    return 'image' if $mime =~ m{^image/};
    return 'calendar-days' if $mime eq 'text/calendar';
    return 'file-archive' if $mime =~ /zip|compressed|tar/;
    return 'file-text' if $mime =~ m{pdf|^text/|document};
    return 'download';
}

# ---- reading an archive ------------------------------------------------

sub sql {
    my ($query) = @_;
    open my $fh, '-|', 'sqlite3', '-json', '-cmd', 'PRAGMA query_only=1', '-cmd', '.timeout 2000', $db_path, $query
        or die "sqlite3: $!\n";
    local $/;
    my $out = <$fh>;
    close $fh or die "sqlite3 failed on: " . substr($query, 0, 80) . "\n";
    return [] unless defined $out && $out =~ /\S/;
    return $JSON->decode($out);
}

sub has_table {
    my ($name) = @_;
    my $rows = sql("SELECT 1 AS ok FROM sqlite_master WHERE type = 'table' AND name = '$name'");
    return scalar @$rows;
}

my $TS = q{(CASE WHEN typeof(COALESCE(m.sent_at, m.received_at, m.internal_date)) IN ('integer', 'real')
    THEN CAST(COALESCE(m.sent_at, m.received_at, m.internal_date) / (CASE WHEN COALESCE(m.sent_at, m.received_at, m.internal_date) > 100000000000 THEN 1000 ELSE 1 END) AS INTEGER)
    ELSE CAST(strftime('%s', COALESCE(m.sent_at, m.received_at, m.internal_date)) AS INTEGER) END)};

my $SUMMARY = qq{SELECT m.id AS id, m.conversation_id AS conv, m.source_id AS src,
    COALESCE(NULLIF(TRIM(m.subject), ''), conv.title, '') AS subject,
    COALESCE(NULLIF(TRIM(m.snippet), ''), SUBSTR(mb.body_text, 1, 240), '') AS snippet,
    COALESCE(NULLIF(TRIM(mr_from.display_name), ''), NULLIF(TRIM(p.display_name), ''), '') AS name,
    COALESCE(NULLIF(p.email_address, ''), NULLIF(p.phone_number, ''), '') AS address,
    $TS AS ts,
    COALESCE(m.has_attachments, 0) AS att, COALESCE(m.message_type, '') AS kind,
    conv.message_count AS thread, conv.title AS conv_title,
    m.deleted_from_source_at IS NOT NULL AS removed, COALESCE(m.is_from_me, 0) AS mine
FROM messages m
LEFT JOIN message_recipients mr_from ON mr_from.id = (
    SELECT mr.id FROM message_recipients mr
    WHERE mr.message_id = m.id AND mr.recipient_type = 'from' ORDER BY mr.id LIMIT 1)
LEFT JOIN participants p ON p.id = COALESCE(mr_from.participant_id, m.sender_id)
LEFT JOIN conversations conv ON conv.id = m.conversation_id
LEFT JOIN message_bodies mb ON mb.message_id = m.id};

# Everything the views need but the bodies: the newest messages with their
# labels and flags, the accounts, labels and totals.
sub read_archive {
    my $newest = "SELECT id FROM messages WHERE deleted_at IS NULL
        ORDER BY COALESCE(sent_at, received_at, internal_date) DESC, id DESC LIMIT $LIMIT";
    my $messages = sql("$SUMMARY WHERE m.id IN ($newest) ORDER BY ts DESC, m.id DESC");
    my %by_id = map { $_->{id} => $_ } @$messages;
    for my $row (@{ sql("SELECT ml.message_id AS id, l.name AS name FROM message_labels ml
            JOIN labels l ON l.id = ml.label_id WHERE ml.message_id IN ($newest)
            ORDER BY l.name COLLATE NOCASE") }) {
        my $m = $by_id{ $row->{id} } or next;
        push @{ $m->{labels} }, $row->{name} unless grep { $_ eq $row->{name} } @{ $m->{labels} // [] };
    }
    if (has_table('imap_message_memberships')) {
        for my $row (@{ sql("SELECT message_id AS id, MAX(flags LIKE '%Seen%') AS seen,
                MAX(flags LIKE '%Flagged%') AS flagged FROM imap_message_memberships
                WHERE message_id IN ($newest) GROUP BY message_id") }) {
            my $m = $by_id{ $row->{id} } or next;
            $m->{imap_unread} = $row->{seen} ? 0 : 1;
            $m->{imap_starred} = $row->{flagged} ? 1 : 0;
        }
    }
    my $accounts = sql("SELECT s.id AS id, s.source_type AS kind, s.identifier AS identifier,
        s.display_name AS name, (SELECT COUNT(*) FROM messages m WHERE m.source_id = s.id
        AND m.deleted_at IS NULL) AS count FROM sources s
        ORDER BY s.source_type, s.identifier COLLATE NOCASE");
    my $labels = sql("SELECT name, MAX(label_type) AS type, MAX(color) AS color FROM labels
        GROUP BY name ORDER BY name COLLATE NOCASE");
    my ($totals) = @{ sql("SELECT (SELECT COUNT(*) FROM messages WHERE deleted_at IS NULL) AS messages,
        (SELECT COUNT(*) FROM conversations) AS threads,
        (SELECT COUNT(*) FROM attachments) AS attachments") };
    my $size = 0;
    $size += (-s $_) // 0 for ($db_path, "$db_path-wal");
    $totals->{size} = $size;
    return {
        messages => $messages, accounts => $accounts, labels => $labels, stats => $totals,
        location => tilde($db_path), backend => 'SQLite',
    };
}

# Every message of a conversation, oldest first, with bodies, recipients and
# attachments.
sub read_thread {
    my ($conv) = @_;
    $conv = int $conv;
    my $messages = sql("$SUMMARY WHERE m.conversation_id = $conv AND m.deleted_at IS NULL
        ORDER BY ts ASC, m.id ASC");
    return [] unless @$messages;
    my %by_id = map { $_->{id} => $_ } @$messages;
    my $ids = join ', ', map { int $_->{id} } @$messages;
    for my $row (@{ sql("SELECT mr.message_id AS id, mr.recipient_type AS type,
            COALESCE(NULLIF(TRIM(mr.display_name), ''), NULLIF(TRIM(p.display_name), ''),
                NULLIF(p.email_address, ''), NULLIF(p.phone_number, ''), '') AS name
            FROM message_recipients mr JOIN participants p ON p.id = mr.participant_id
            WHERE mr.message_id IN ($ids) AND mr.recipient_type IN ('to', 'cc', 'bcc')
            ORDER BY mr.message_id, mr.id") }) {
        push @{ $by_id{ $row->{id} }{ $row->{type} } }, $row->{name};
    }
    for my $row (@{ sql("SELECT message_id AS id, body_text AS text, body_html AS html
            FROM message_bodies WHERE message_id IN ($ids)") }) {
        my $m = $by_id{ $row->{id} };
        $m->{text} = $row->{text} if defined $row->{text} && $row->{text} =~ /\S/;
        $m->{html} = $row->{html} if defined $row->{html} && $row->{html} =~ /\S/;
    }
    my $role = grep({ $_->{name} eq 'attachment_role' } @{ sql("PRAGMA table_info(attachments)") })
        ? 'attachment_role' : 'NULL';
    for my $row (@{ sql("SELECT message_id AS id, COALESCE(filename, '') AS name,
            COALESCE(mime_type, '') AS mime, COALESCE(size, 0) AS size, $role AS role
            FROM attachments WHERE message_id IN ($ids) ORDER BY id") }) {
        next if defined $row->{role} && $row->{role} =~ /^(inline|avatar|thumbnail|preview|ui_asset)$/;
        push @{ $by_id{ $row->{id} }{files} }, { name => $row->{name}, mime => $row->{mime}, size => $row->{size} };
    }
    return $messages;
}

# ---- the sample ----------------------------------------------------------

sub read_sample {
    open my $fh, '<:raw', 'sample.json' or return undef;
    local $/;
    my $data = eval { $JSON->decode(<$fh>) };
    close $fh;
    return undef unless $data;
    my $now = time;
    for my $m (@{ $data->{messages} }, map { @$_ } values %{ $data->{threads} }) {
        $m->{ts} = $now - $m->{ago} if defined $m->{ago};
    }
    $data->{backend} = 'Sample';
    $data->{location} = 'demo archive';
    $data->{sample} = 1;
    return $data;
}

# ---- the model -----------------------------------------------------------

my ($data, $mode, $read_at) = (undef, '', 0);

sub load {
    if (-f $db_path && have_sqlite()) {
        my $archive = eval { read_archive() };
        if ($archive) {
            ($data, $mode, $read_at) = ($archive, 'archive', time);
            decorate($data);
            return;
        }
        print STDERR "vault: reading $db_path failed: $@";
    }
    ($data, $mode, $read_at) = (read_sample(), 'sample', time);
    decorate($data) if $data;
}

sub has_label {
    my ($m, @names) = @_;
    my %want = map { $_ => 1 } @names;
    return scalar grep { $want{ uc $_ } } @{ $m->{labels} // [] };
}

sub decorate {
    my ($d) = @_;
    for my $m (@{ $d->{messages} }) {
        $m->{labels} //= [];
        $m->{unread} = (has_label($m, 'UNREAD') || $m->{imap_unread}) ? 1 : 0;
        $m->{starred} = (has_label($m, 'STARRED') || $m->{imap_starred}) ? 1 : 0;
        $m->{email} = ($m->{kind} eq '' || $m->{kind} eq 'email') ? 1 : 0;
        $m->{snippet} = clean($m->{snippet});
    }
}

my @SENT = ('SENT', 'SENT ITEMS', 'SENT MAIL', 'SENT MESSAGES', '[GMAIL]/SENT MAIL');
my @DRAFTS = ('DRAFT', 'DRAFTS', '[GMAIL]/DRAFTS');
my @SPAM = ('SPAM', 'JUNK', 'JUNK E-MAIL', 'JUNK EMAIL', '[GMAIL]/SPAM');
my @TRASH = ('TRASH', 'BIN', 'DELETED ITEMS', 'DELETED MESSAGES', '[GMAIL]/TRASH', '[GMAIL]/BIN');

sub in_mailbox {
    my ($m, $box) = @_;
    return has_label($m, 'INBOX') if $box eq 'inbox';
    return $m->{starred} if $box eq 'starred';
    return $m->{unread} if $box eq 'unread';
    return has_label($m, 'IMPORTANT') if $box eq 'important';
    return has_label($m, @SENT) || ($m->{mine} && $m->{email}) if $box eq 'sent';
    return has_label($m, @DRAFTS) if $box eq 'drafts';
    return $m->{email} if $box eq 'all';
    return $m->{email} && $m->{att} if $box eq 'attachments';
    return has_label($m, @SPAM) if $box eq 'spam';
    return has_label($m, @TRASH) if $box eq 'trash';
    return $m->{removed} if $box eq 'removed';
    if ($box =~ /^label:(.*)$/s) {
        my $name = $1;
        return scalar grep { $_ eq $name } @{ $m->{labels} };
    }
    if ($box =~ /^kind:(.*)$/s) {
        my $kind = $1;
        return ($m->{kind} || 'email') eq $kind;
    }
    return 0;
}

my %TITLES = (
    inbox => 'Inbox', starred => 'Starred', unread => 'Unread', important => 'Important',
    sent => 'Sent', drafts => 'Drafts', all => 'All Mail', attachments => 'Attachments',
    spam => 'Spam', trash => 'Trash', removed => 'Removed Upstream',
);
sub mailbox_title {
    my ($box) = @_;
    return $TITLES{$box} if $TITLES{$box};
    return display_label($1) if $box =~ /^label:(.*)$/s;
    return display_kind($1) if $box =~ /^kind:(.*)$/s;
    return $box;
}

# msgvault's Gmail-like search: free text, from:, to:, subject:, label:,
# has:attachment, is:unread, is:starred, before:, after:.
sub matches_query {
    my ($m, $query) = @_;
    for my $term ($query =~ /(\S+:"[^"]*"|"[^"]*"|\S+)/g) {
        my ($op, $value) = $term =~ /^(\w+):(.*)$/ ? (lc $1, $2) : ('', $term);
        $value =~ s/^"|"$//g;
        my $v = lc $value;
        my $has = sub { my ($text) = @_; index(lc($text // ''), $v) >= 0 };
        if ($op eq 'from') { return 0 unless $has->($m->{name}) || $has->($m->{address}) }
        elsif ($op eq 'subject') { return 0 unless $has->($m->{subject}) }
        elsif ($op eq 'label') {
            return 0 unless grep { lc($_) eq $v || lc($_) eq "category_$v" || lc(display_label($_)) eq $v } @{ $m->{labels} };
        }
        elsif ($op eq 'has') { return 0 if $v =~ /^attach/ && !$m->{att} }
        elsif ($op eq 'is') {
            return 0 if $v eq 'unread' && !$m->{unread};
            return 0 if $v eq 'starred' && !$m->{starred};
        }
        elsif ($op eq 'before' || $op eq 'after') {
            next unless $v =~ /^(\d{4})-(\d\d)-(\d\d)$/;
            my $day = sprintf '%04d-%02d-%02d', (localtime($m->{ts} // 0))[5] + 1900, (localtime($m->{ts} // 0))[4] + 1, (localtime($m->{ts} // 0))[3];
            return 0 if $op eq 'before' && $day ge $v;
            return 0 if $op eq 'after' && $day lt $v;
        }
        elsif ($op eq 'to') { next }    # recipients are only read with a thread
        else {
            return 0 unless $has->($m->{subject}) || $has->($m->{snippet}) || $has->($m->{name})
                || $has->($m->{address}) || $has->($m->{conv_title});
        }
    }
    return 1;
}

sub chips {
    my ($m, $most) = @_;
    my @chips;
    for my $name (@{ $m->{labels} }) {
        next if $STATE{ uc $name };
        push @chips, { title => display_label($name), color => label_color($name) };
        last if $most && @chips >= $most;
    }
    return \@chips;
}

sub person {
    my ($m) = @_;
    return $m->{name} ne '' ? $m->{name} : ($m->{address} ne '' ? $m->{address} : 'Unknown');
}

sub row {
    my ($m, $now) = @_;
    my $sender = person($m);
    my ($title, $subject);
    if ($m->{email}) {
        $title = $sender;
        $subject = $m->{subject} =~ /\S/ ? $m->{subject} : '(no subject)';
    } else {
        $title = $m->{subject} =~ /\S/ ? $m->{subject} : display_kind($m->{kind});
        $subject = $sender;
    }
    my $initials = initials($title);
    return {
        id => $m->{id} + 0,
        title => $title,
        subject => $subject,
        snippet => $m->{snippet},
        date => list_date($m->{ts}, $now),
        unread => $m->{unread} ? $TRUE : $FALSE,
        starred => $m->{starred} ? $TRUE : $FALSE,
        attachments => $m->{att} ? $TRUE : $FALSE,
        removed => $m->{removed} ? $TRUE : $FALSE,
        count => ($m->{email} && ($m->{thread} // 0) > 1) ? $m->{thread} + 0 : 0,
        chips => chips($m, 2),
        initials => $initials,
        hue => hue($initials),
    };
}

sub thread_view {
    my ($open, $now) = @_;
    my ($first) = grep { $_->{id} == $open } @{ $data->{messages} };
    return undef unless $first;
    my $messages;
    if ($mode eq 'archive') {
        $messages = defined $first->{conv} ? eval { read_thread($first->{conv}) } : undef;
        $messages = undef unless $messages && @$messages;
    } else {
        $messages = $data->{threads}{ $first->{conv} // '' };
    }
    $messages ||= [ $first ];
    my $chat = grep { ($_->{kind} // '') ne '' && $_->{kind} ne 'email' } @$messages;
    # Labels from the list's copies, which carry them.
    my %listed = map { $_->{id} => $_ } @{ $data->{messages} };
    my (@labels, %seen);
    for my $m (@$messages) {
        my $known = $listed{ $m->{id} } // $m;
        for my $chip (@{ chips($known) }) {
            push @labels, $chip unless $seen{ $chip->{title} }++;
        }
    }
    my $account;
    for my $a (@{ $data->{accounts} }) {
        $account = ($a->{name} // '') ne '' ? $a->{name} : $a->{identifier} if defined $first->{src} && $a->{id} == $first->{src};
    }
    my $subject = $first->{conv_title} // '';
    $subject = $messages->[0]{subject} // '' if $first->{email} || $subject !~ /\S/;
    $subject = '(no subject)' unless $subject =~ /\S/;
    my $total = scalar @$messages;
    my @meta = (plural($total, 'message', 'messages'));
    unshift @meta, display_kind($first->{kind}) if $chat;
    push @meta, $account if defined $account && !grep { $_ eq $account } @meta;
    my @out;
    for my $m (@$messages) {
        my $sender = person($m);
        my $initials = initials($sender);
        my $body = defined $m->{text} ? $m->{text} : defined $m->{html} ? html_to_text($m->{html}) : $m->{snippet} // '';
        $body =~ s/\s+$//;
        my $known = $listed{ $m->{id} } // $m;
        push @out, {
            id => $m->{id} + 0,
            from => $sender,
            address => ($m->{address} // '') ne $sender ? $m->{address} // '' : '',
            to => join(', ', @{ $m->{to} // [] }),
            cc => join(', ', @{ $m->{cc} // [] }),
            date => full_date($m->{ts}),
            time => $m->{ts} ? clock($m->{ts}) : '',
            snippet => clean($m->{snippet}),
            body => $body,
            mine => $m->{mine} ? $TRUE : $FALSE,
            starred => $known->{starred} ? $TRUE : $FALSE,
            attachments => $m->{att} ? $TRUE : $FALSE,
            initials => $initials,
            hue => hue($initials),
            files => [ map { { name => $_->{name} ne '' ? $_->{name} : 'attachment', size => bytes($_->{size}), icon => attachment_icon($_->{mime}) } } @{ $m->{files} // [] } ],
        };
    }
    return {
        id => $first->{id} + 0,
        subject => $subject,
        chat => $chat ? $TRUE : $FALSE,
        meta => join(' · ', @meta),
        labels => \@labels,
        messages => \@out,
    };
}

# ---- the view ------------------------------------------------------------

my %view = (mailbox => 'inbox', account => 0, query => '', open => 0);

sub read_view {
    open my $fh, '<:raw', $view_file or return;
    local $/;
    my $saved = eval { $JSON->decode(<$fh>) };
    close $fh;
    %view = (%view, %$saved) if ref $saved eq 'HASH';
}

sub save_view {
    make_path($cache_dir) unless -d $cache_dir;
    open my $fh, '>:raw', "$view_file.tmp" or return;
    print $fh $JSON->encode(\%view);
    close $fh;
    rename "$view_file.tmp", $view_file;
}

sub publish {
    unless ($data) {
        print $JSON->encode({ available => $FALSE, reason => 'No msgvault archive found and sample.json is missing' }), "\n";
        return;
    }
    my $now = time;
    my @all = @{ $data->{messages} };
    my @scoped = $view{account} ? grep { ($_->{src} // 0) == $view{account} } @all : @all;

    my %counts = (inbox => 0, starred => 0, drafts => 0, all => 0, attachments => 0, removed => 0);
    my (%label_count, %kind_count);
    for my $m (@scoped) {
        $counts{inbox}++ if $m->{unread} && in_mailbox($m, 'inbox');
        $counts{starred}++ if $m->{starred};
        $counts{drafts}++ if in_mailbox($m, 'drafts');
        $counts{all}++ if $m->{email};
        $counts{attachments}++ if $m->{email} && $m->{att};
        $counts{removed}++ if $m->{removed};
        $label_count{$_}++ for @{ $m->{labels} };
        $kind_count{ $m->{kind} || 'email' }++;
    }
    my (@labels, @categories);
    for my $label (@{ $data->{labels} }) {
        my $n = $label_count{ $label->{name} } or next;
        my $upper = uc $label->{name};
        my $entry = { name => $label->{name}, title => display_label($label->{name}), count => $n,
            color => label_color($label->{name}, $label->{color}) };
        if ($upper =~ /^CATEGORY_/) { push @categories, $entry }
        elsif (!$SYSTEM{$upper} && ($label->{type} // '') !~ /^system$/i) { push @labels, $entry }
    }
    my @archive = map { { id => "kind:$_", title => display_kind($_), icon => kind_icon($_), count => $kind_count{$_} } }
        sort { $kind_count{$b} <=> $kind_count{$a} || $a cmp $b } grep { $_ ne 'email' } keys %kind_count;
    push @archive, { id => 'removed', title => 'Removed Upstream', icon => 'cloud-off', count => $counts{removed} }
        if $counts{removed};

    my @accounts = map {
        { id => $_->{id} + 0, title => (($_->{name} // '') ne '' ? $_->{name} : $_->{identifier}),
          kind => display_kind($_->{kind}) eq title_case($_->{kind}) ? title_case($_->{kind}) : display_kind($_->{kind}),
          identifier => $_->{identifier}, icon => account_icon($_->{kind}), count => $_->{count} + 0 }
    } @{ $data->{accounts} };
    my ($current) = grep { $_->{id} == $view{account} } @accounts;
    $view{account} = 0 unless $current;
    my $header = $current
        ? { title => $current->{title}, subtitle => "$current->{kind} · $current->{identifier}", icon => $current->{icon} }
        : { title => 'All Accounts', subtitle => plural(scalar @accounts, 'source', 'sources'), icon => 'archive' };

    my $query = $view{query} // '';
    my @rows = $query =~ /\S/
        ? grep { matches_query($_, $query) } @scoped
        : grep { in_mailbox($_, $view{mailbox}) } @scoped;
    # One row per conversation: its newest message, with the older ones'
    # unread, starred and attachment state folded in (merge_page).
    my (@threads, %row_of);
    for my $m (@rows) {
        my $conv = $m->{conv};
        if (!defined $conv) { push @threads, { %$m }; next }
        if (my $row = $row_of{$conv}) {
            $row->{unread} ||= $m->{unread};
            $row->{starred} ||= $m->{starred};
            $row->{att} ||= $m->{att};
            next;
        }
        push @threads, $row_of{$conv} = { %$m };
    }
    @rows = @threads;
    my $title = $query =~ /\S/ ? 'Search' : mailbox_title($view{mailbox});
    my $subtitle = $query =~ /\S/
        ? plural(scalar @rows, 'result', 'results') . " for “$query”"
        : plural(scalar @rows, 'conversation', 'conversations');
    $subtitle .= " · $current->{title}" if $current;
    my $more = @rows > $ROWS;
    @rows = @rows[0 .. $ROWS - 1] if $more;

    my $stats = $data->{stats};
    print $JSON->encode({
        available => $TRUE,
        sample => $data->{sample} ? $TRUE : $FALSE,
        backend => $data->{backend},
        location => $data->{location},
        stats => {
            messages => plural($stats->{messages}, 'message', 'messages'),
            threads => plural($stats->{threads}, 'thread', 'threads'),
            attachments => plural($stats->{attachments}, 'attachment', 'attachments'),
            size => bytes($stats->{size} // 0),
        },
        header => $header,
        accounts => \@accounts,
        account => $view{account} + 0,
        counts => \%counts,
        labels => \@labels,
        categories => \@categories,
        archive => \@archive,
        mailbox => $view{mailbox},
        query => $query,
        list => { title => $title, subtitle => $subtitle . ($more ? '+' : ''), rows => [ map { row($_, $now) } @rows ] },
        selected => $view{open} + 0,
        thread => $view{open} ? thread_view($view{open}, $now) : undef,
    }), "\n";
}

sub receive {
    my ($line) = @_;
    my $envelope = eval { $JSON->decode($line) } or return;
    my $msg = $envelope->{message};
    return unless ref $msg eq 'HASH';
    if (defined $msg->{mailbox}) {
        $view{mailbox} = "$msg->{mailbox}";
        $view{query} = '';
    }
    $view{account} = int $msg->{account} if defined $msg->{account};
    $view{query} = "$msg->{query}" if defined $msg->{query};
    $view{open} = int $msg->{open} if defined $msg->{open};
    load() if $msg->{refresh};
    save_view();
}

# ---- main ----------------------------------------------------------------

if (@ARGV && $ARGV[0] eq '--dump') {
    die "no archive at $db_path\n" unless -f $db_path;
    my $archive = read_archive();
    my $now = time;
    my %threads;
    my %convs;
    $convs{ $_->{conv} } = 1 for grep { defined $_->{conv} } @{ $archive->{messages} };
    for my $conv (keys %convs) {
        $threads{$conv} = read_thread($conv);
    }
    for my $m (@{ $archive->{messages} }, map { @$_ } values %threads) {
        $m->{ago} = $now - delete $m->{ts};
    }
    $archive->{threads} = \%threads;
    delete @$archive{qw(location backend)};
    print JSON::PP->new->canonical->utf8->pretty->encode($archive);
    exit 0;
}

read_view();
load();
publish();
my $select = IO::Select->new(\*STDIN);
my $buffer = '';
while (1) {
    my @ready = $select->can_read($REFRESH);
    if (!@ready) {
        if ($mode eq 'archive') { load(); publish() }
        next;
    }
    my $got = sysread STDIN, $buffer, 65536, length $buffer;
    last unless $got;    # the app closed stdin
    while ($buffer =~ s/^([^\n]*)\n//) {
        receive($1);
    }
    publish();
}

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
# "kind:whatsapp" | "files" | ... }, { account = id (0 for all) },
# { query = "text" }, { open = message id }, { remote = message id } (load
# its remote pictures), and in the attachments browser { file = id },
# { kind = "image" | ... }, { fquery = true, value = "text" }, { show =
# message id }. The view is kept in ~/.cache/moonframe/msgvault, with the
# HTML pages it writes for the reader. Only the reader and the list may ask
# for { open_file = id } (a temporary copy, opened in the system's app) and
# { save_file = id } (into ~/Downloads), each on a click.
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
use Digest::SHA qw(sha256_hex);
use Compress::Zlib ();
use File::Temp qw(tempfile);

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
my $html_dir = "$cache_dir/html";
# Pictures go where ui.image may read them: the app's image cache.
my $images_dir = ($ENV{XDG_CACHE_HOME} || "$home/.cache") . '/moonframe/images/msgvault';
my $blob_root = "$vault_home/attachments";
my $OPEN_LIMIT = 512 * 1024 * 1024;   # largest attachment read, as msgvault-gpui
my $THUMB_LIMIT = 8 * 1024 * 1024;    # largest picture shown inline

# What was read (the archive or the sample), and what the user looks at.
my ($data, $mode, $read_at) = (undef, '', 0);
my %view = (mailbox => 'inbox', account => 0, query => '', open => 0, fkind => 'all', fquery => '', file => '', remote => {});

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
    for my $row (@{ sql("SELECT id AS aid, message_id AS id, COALESCE(filename, '') AS name,
            COALESCE(mime_type, '') AS mime, COALESCE(size, 0) AS size, $role AS role,
            content_hash AS hash, storage_path AS path
            FROM attachments WHERE message_id IN ($ids) ORDER BY id") }) {
        next if defined $row->{role} && $row->{role} =~ /^(inline|avatar|thumbnail|preview|ui_asset)$/;
        push @{ $by_id{ $row->{id} }{files} }, {
            aid => "a$row->{aid}", name => $row->{name}, mime => $row->{mime}, size => $row->{size},
            hash => $row->{hash}, path => $row->{path},
        };
    }
    return $messages;
}

# ---- attachments -----------------------------------------------------------

# Every attachment in the archive (the newest $LIMIT), with the message it
# came in, for the attachments browser.
sub read_files {
    my $role = grep({ $_->{name} eq 'attachment_role' } @{ sql("PRAGMA table_info(attachments)") })
        ? 'a.attachment_role' : 'NULL';
    my $files = sql("SELECT a.id AS aid, a.message_id AS msg, COALESCE(a.filename, '') AS name,
        COALESCE(a.mime_type, '') AS mime, COALESCE(a.size, 0) AS size, $role AS role,
        a.content_hash AS hash, a.storage_path AS path
        FROM attachments a JOIN messages m ON m.id = a.message_id
        WHERE m.deleted_at IS NULL ORDER BY a.id DESC LIMIT $LIMIT");
    @$files = grep { !(defined $_->{role} && $_->{role} =~ /^(inline|avatar|thumbnail|preview|ui_asset)$/) } @$files;
    return [] unless @$files;
    my $ids = join ', ', map { int $_->{msg} } @$files;
    my %messages = map { $_->{id} => $_ } @{ sql("$SUMMARY WHERE m.id IN ($ids)") };
    for my $f (@$files) {
        $f->{aid} = "a$f->{aid}";
        my $m = $messages{ $f->{msg} } or next;
        @$f{qw(from subject ts conv)} = (person($m), $m->{subject}, $m->{ts}, $m->{conv});
        $f->{subject_for_content} = $m->{subject};
    }
    return [ sort { ($b->{ts} // 0) <=> ($a->{ts} // 0) } @$files ];
}

# The sample's attachments, from its threads: ids made from the message's.
sub sample_files {
    my ($d) = @_;
    my @files;
    for my $thread (values %{ $d->{threads} }) {
        for my $m (@$thread) {
            my $i = 0;
            for my $f (@{ $m->{files} // [] }) {
                $f->{aid} //= "s$m->{id}-" . $i++;
                $f->{subject_for_content} = $m->{conv_title} // $m->{subject};
                push @files, { %$f, msg => $m->{id}, from => person($m), subject => $m->{subject}, ts => $m->{ts}, conv => $m->{conv} };
            }
        }
    }
    return [ sort { ($b->{ts} // 0) <=> ($a->{ts} // 0) || $a->{name} cmp $b->{name} } @files ];
}

# What kind of file it is, for the browser's filters and the icon.
my @FILE_KINDS = (
    [ image => 'Images', 'image' ],
    [ pdf => 'PDFs', 'file-text' ],
    [ document => 'Documents', 'file-text' ],
    [ calendar => 'Calendar', 'calendar-days' ],
    [ archive => 'Archives', 'file-archive' ],
    [ other => 'Other', 'file' ],
);
my %KIND_ICON = map { $_->[0] => $_->[2] } @FILE_KINDS;

sub file_kind {
    my ($f) = @_;
    my $mime = lc($f->{mime} // '');
    my $name = lc($f->{name} // '');
    return 'image' if $mime =~ m{^image/} || $name =~ /\.(png|jpe?g|gif|webp|heic|bmp|tiff?)$/;
    return 'pdf' if $mime =~ /pdf/ || $name =~ /\.pdf$/;
    return 'calendar' if $mime eq 'text/calendar' || $name =~ /\.ics$/;
    return 'archive' if $mime =~ /zip|compressed|tar|gzip|7z|rar/ || $name =~ /\.(zip|tar|gz|tgz|7z|rar)$/;
    return 'document' if $mime =~ m{^text/|document|sheet|presentation|msword|excel|powerpoint|rtf|json|xml|csv}
        || $name =~ /\.(txt|md|csv|docx?|xlsx?|pptx?|pages|numbers|key|rtf|odt|ods|json)$/;
    return 'other';
}

# Filenames from mail headers are untrusted: keep a safe last component.
sub safe_filename {
    my ($raw) = @_;
    my $name = (split m{[/\\]}, $raw // '')[-1] // '';
    $name =~ s/[[:cntrl:]:*?"<>|]/_/g;
    $name =~ s/^[\s.]+|\s+$//g;
    return $name eq '' ? 'attachment' : $name;
}

# The SHA-256 the bytes are filed under: content_hash, else storage_path's name.
sub content_hash {
    my ($f) = @_;
    for my $candidate ($f->{hash}, (split m{/}, $f->{path} // '')[-1]) {
        return lc $candidate if defined $candidate && $candidate =~ /^[0-9a-fA-F]{64}$/;
    }
    return undef;
}

sub slurp {
    my ($path, $offset, $length) = @_;
    open my $fh, '<:raw', $path or return undef;
    seek $fh, $offset, 0 if $offset;
    my $bytes = '';
    if (defined $length) { read $fh, $bytes, $length }
    else { local $/; $bytes = <$fh> // '' }
    close $fh;
    return $bytes;
}

# An attachment's bytes, read the way msgvault stores them (internal/store):
# a sealed pack found through attachment_pack_index (a zstd frame when flag
# bit 0 is set), else a loose file at attachments/<sha[:2]>/<sha>; checked
# against their SHA-256. The sample's are made as msgvault-gpui's fixture
# makes them. Returns (bytes), or (undef, why).
sub file_bytes {
    my ($f) = @_;
    return synthesize($f) if $mode ne 'archive';
    return (undef, "it is stored remotely") if ($f->{path} // '') =~ m{^https?://};
    my $hash = content_hash($f) or return (undef, 'msgvault has no content for it');
    return (undef, 'it is larger than 512 MB') if ($f->{size} // 0) > $OPEN_LIMIT;
    my $bytes;
    if (has_table('attachment_pack_index')) {
        my ($entry) = @{ sql("SELECT pack_id AS pack, pack_offset AS offset, stored_len AS stored,
            raw_len AS raw, flags FROM attachment_pack_index WHERE blob_hash = '$hash'") };
        if ($entry && ($entry->{pack} // '') =~ /^[0-9A-Za-z_-]{2,}$/) {
            my $pack = "$blob_root/packs/" . substr($entry->{pack}, 0, 2) . "/$entry->{pack}.mvpack";
            my $stored = slurp($pack, $entry->{offset}, $entry->{stored});
            return (undef, 'its pack is missing: ' . tilde($pack)) unless defined $stored;
            if (($entry->{flags} // 0) & 1) {
                my ($zstd) = grep { -x "$_/zstd" } split /:/, $ENV{PATH} // '';
                return (undef, 'it is compressed with zstd, which is not installed') unless $zstd;
                my ($tmp, $tmp_path) = tempfile(UNLINK => 1);
                binmode $tmp;
                print $tmp $stored;
                close $tmp;
                open my $pipe, '-|', "$zstd/zstd", '-dcq', $tmp_path or return (undef, "zstd: $!");
                binmode $pipe;
                local $/;
                $bytes = <$pipe>;
                close $pipe;
                unlink $tmp_path;
            } else {
                $bytes = $stored;
            }
        }
    }
    unless (defined $bytes) {
        my @candidates = ("$blob_root/" . substr($hash, 0, 2) . "/$hash");
        push @candidates, "$blob_root/$f->{path}" if defined $f->{path} && $f->{path} !~ m{\.\.|^/};
        for my $path (@candidates) {
            next unless -f $path;
            $bytes = slurp($path);
            last;
        }
    }
    return (undef, 'its content is not in the archive') unless defined $bytes;
    return (undef, 'its content does not match its checksum') unless sha256_hex($bytes) eq $hash;
    return ($bytes);
}

# The sample's files: msgvault-gpui's fixture content (a one-page PDF, an
# event, a gradient picture, a line of text).
sub synthesize {
    my ($f) = @_;
    my $title = $f->{subject_for_content} // $f->{name};
    my $mime = $f->{mime} // '';
    if ($mime eq 'application/pdf') {
        (my $text = $title) =~ s/[^\x20-\x7e]|[()\\]//g;
        my $stream = "BT /F1 18 Tf 72 720 Td ($text) Tj ET";
        my @objects = (
            '<< /Type /Catalog /Pages 2 0 R >>',
            '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
            '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
            "<< /Length " . length($stream) . " >>\nstream\n$stream\nendstream",
            '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        );
        my $out = "%PDF-1.4\n";
        my @offsets;
        for my $i (0 .. $#objects) {
            push @offsets, length $out;
            $out .= ($i + 1) . " 0 obj\n$objects[$i]\nendobj\n";
        }
        my $xref = length $out;
        $out .= "xref\n0 " . (@objects + 1) . "\n0000000000 65535 f \n";
        $out .= sprintf("%010d 00000 n \n", $_) for @offsets;
        $out .= "trailer\n<< /Size " . (@objects + 1) . " /Root 1 0 R >>\nstartxref\n$xref\n%%EOF\n";
        return ($out);
    }
    if ($mime eq 'text/calendar') {
        my $summary = $title;
        utf8::encode($summary);
        return ("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//msgvault-gpui//fixture//EN\r\nBEGIN:VEVENT\r\nUID:$f->{name}\r\nSUMMARY:$summary\r\nDTSTART:20240920T184500Z\r\nDTEND:20240921T140000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
    }
    if ($mime =~ m{^image/}) {
        my ($w, $h) = (320, 200);
        my $raw = '';
        for my $y (0 .. $h - 1) {
            $raw .= "\0";
            for my $x (0 .. $w - 1) {
                $raw .= pack 'C3', 40 + 180 * $x / $w, 90 + 100 * $y / $h, 200 - 90 * $x / $w;
            }
        }
        my $chunk = sub {
            my ($kind, $body) = @_;
            return pack('N', length $body) . $kind . $body . pack('N', Compress::Zlib::crc32($kind . $body));
        };
        return ("\x89PNG\r\n\x1a\n" . $chunk->('IHDR', pack('NNC5', $w, $h, 8, 2, 0, 0, 0))
            . $chunk->('IDAT', Compress::Zlib::compress($raw)) . $chunk->('IEND', ''));
    }
    my $text = "$title\n";
    utf8::encode($text);
    return ($text);
}

sub find_file {
    my ($aid) = @_;
    for my $f (@{ $data->{files} // [] }) {
        return $f if $f->{aid} eq $aid;
    }
    return undef;
}

# A picture attachment as a file in the image cache, for ui.image; undef
# when it is not a picture, is too large, or can't be read.
my %thumb_failed;
sub thumbnail {
    my ($f) = @_;
    return undef unless file_kind($f) eq 'image' && ($f->{size} // 0) <= $THUMB_LIMIT;
    my ($ext) = lc($f->{name} // '') =~ /\.(png|jpe?g|gif|webp|bmp|tiff?|heic)$/;
    $ext //= (($f->{mime} // '') =~ m{^image/(\w+)} ? $1 : 'png');
    my $key = content_hash($f) // $f->{aid};
    $key =~ s/[^0-9A-Za-z_-]//g;
    my $path = "$images_dir/$key.$ext";
    return $path if -f $path;
    return undef if $thumb_failed{$key};
    my ($bytes) = file_bytes($f);
    unless (defined $bytes) { $thumb_failed{$key} = 1; return undef }
    make_path($images_dir) unless -d $images_dir;
    open my $fh, '>:raw', "$path.tmp" or return undef;
    print $fh $bytes;
    close $fh;
    rename "$path.tmp", $path;
    return $path;
}

sub file_card {
    my ($f, $now) = @_;
    my $kind = file_kind($f);
    return {
        id => $f->{aid},
        name => ($f->{name} // '') ne '' ? $f->{name} : 'attachment',
        size => ($f->{path} // '') =~ m{^https?://} ? 'Remote file' : bytes($f->{size} // 0),
        kind => $kind,
        type => (grep { $_->[0] eq $kind } @FILE_KINDS)[0][1] =~ s/s$//r,
        icon => $KIND_ICON{$kind},
        thumb => thumbnail($f),
        from => $f->{from} // '',
        subject => $f->{subject} // '',
        message => ($f->{msg} // 0) + 0,
        date => $now ? list_date($f->{ts}, $now) : full_date($f->{ts}),
    };
}

# Open (in the system's app for it) or save (to Downloads, then shown in the
# file manager) an attachment: only ever on the user's click.
my $notice_seq = 0;
my $notice;
sub tell_user {
    my ($ok, $text) = @_;
    $notice = { seq => ++$notice_seq, ok => $ok ? $TRUE : $FALSE, text => $text };
}

sub system_open {
    my (@args) = @_;
    my $opener = $^O eq 'darwin' ? 'open' : 'xdg-open';
    my $pid = fork;
    return unless defined $pid;
    if ($pid == 0) {
        open STDIN, '<', '/dev/null';
        open STDOUT, '>', '/dev/null';
        exec $opener, @args;
        exit 1;
    }
    waitpid $pid, 0;
}

sub unique_path {
    my ($dir, $name) = @_;
    my ($stem, $ext) = $name =~ /^(.*?)(\.[^.]*)?$/;
    $ext //= '';
    my $path = "$dir/$name";
    my $n = 1;
    $path = "$dir/$stem (" . $n++ . ")$ext" while -e $path;
    return $path;
}

sub deliver {
    my ($aid, $save) = @_;
    my $f = find_file($aid) or return tell_user(0, 'That attachment is no longer in the archive');
    my $name = safe_filename($f->{name});
    if (($f->{path} // '') =~ m{^https?://} && $mode eq 'archive') {
        system_open($f->{path});
        return tell_user(1, "Opened $name in the browser");
    }
    my ($bytes, $error) = file_bytes($f);
    return tell_user(0, "Can't open $name: $error") unless defined $bytes;
    my $dir = $save ? ($ENV{MSGVAULT_DOWNLOADS} || "$home/Downloads")
        : ($ENV{TMPDIR} || '/tmp') . "/msgvault-moonframe/$f->{aid}";
    make_path($dir) unless -d $dir;
    my $path = $save ? unique_path($dir, $name) : "$dir/$name";
    open my $fh, '>:raw', $path or return tell_user(0, "Can't write " . tilde($path) . ": $!");
    print $fh $bytes;
    close $fh;
    if ($save) {
        system_open('-R', $path) if $^O eq 'darwin' && !$ENV{MSGVAULT_NO_OPEN};
        (my $saved = $path) =~ s{.*/}{};
        return tell_user(1, "Saved $saved to Downloads");
    }
    system_open($path) unless $ENV{MSGVAULT_NO_OPEN};
    return tell_user(1, "Opened $name");
}

# ---- HTML mail as a page ---------------------------------------------------

# Whether the HTML loads anything from the network (pictures, backgrounds).
sub remote_content {
    my ($html) = @_;
    return ($html =~ /(?:src|background|srcset)\s*=\s*["']?\s*(?:https?:)?\/\//i
        || $html =~ /url\(\s*['"]?\s*(?:https?:)?\/\//i) ? 1 : 0;
}

# A message's HTML as a page for the reader's web view: its body and styles
# kept; scripts, frames, forms and refreshes dropped; under a
# Content-Security-Policy that runs no script and loads nothing from the
# network (remote pictures neither, until the user asks); in the window's
# light or dark colours. Returns its file:// address.
sub page {
    my ($id, $html, $dark, $remote) = @_;
    my $file = "$html_dir/$id-" . ($dark ? 'dark' : 'light') . ($remote ? '-remote' : '') . '.html';
    return "file://$file" if -f $file;
    my @styles = $html =~ m{(<style\b.*?</style\s*>)}gis;
    s{\@import[^;]*;}{}gi for @styles;
    my $body = $html =~ m{<body\b[^>]*>(.*)</body\s*>}is ? $1 : $html;
    $body =~ s{<(script|style|head|title|iframe|object|embed|form|noscript)\b.*?</\1\s*>}{}gis;
    $body =~ s{<(script|meta|link|base|iframe|object|embed)\b[^>]*>}{}gi;
    $body =~ s{\son\w+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)}{}gi;
    my $images = $remote ? 'data: cid: https: http:' : 'data: cid:';
    my ($bg, $fg, $muted, $link, $rule) = $dark
        ? ('#0a0a0a', '#fafafa', '#a3a3a3', '#60a5fa', '#262626')
        : ('#ffffff', '#0a0a0a', '#737373', '#2563eb', '#e5e5e5');
    my $scheme = $dark ? 'dark' : 'light';
    my $kept = join "\n", @styles;
    my $page = <<"HTML";
<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src $images; style-src 'unsafe-inline'; font-src data:; form-action 'none'; base-uri 'none'">
<meta name="color-scheme" content="$scheme">
<style>
html, body { margin: 0; padding: 0; background: $bg; color: $fg; }
body { font: 14px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; overflow-wrap: anywhere; }
a { color: $link; }
h1 { font-size: 2.15em; line-height: 1.2; margin: 0.15em 0 0.35em; }
h2, h3 { line-height: 1.3; margin: 0.5em 0 0.3em; }
p { margin: 0 0 0.5em; }
ul, ol { margin: 0 0 0.5em; padding-left: 1.1em; }
blockquote { margin: 0.3em 0 0.5em; padding-left: 1.4em; border-left: 3px solid $rule; color: $muted; }
img { max-width: 100%; height: auto; }
table { border-collapse: collapse; max-width: 100%; }
td, th { padding: 2px 12px 2px 0; vertical-align: top; text-align: left; }
small { color: $muted; }
</style>
$kept
</head><body>$body</body></html>
HTML
    make_path($html_dir) unless -d $html_dir;
    utf8::encode($page) if utf8::is_utf8($page);
    open my $fh, '>:raw', "$file.tmp" or return undef;
    print $fh $page;
    close $fh;
    rename "$file.tmp", $file;
    return "file://$file";
}

# How tall the page draws, roughly: a view can't measure it.
sub page_height {
    my ($html) = @_;
    my $height = 6;
    for my $line (split /\n/, html_to_text($html)) {
        $height += $line =~ /\S/ ? 23 * (1 + int(length($line) / 95)) : 6;
    }
    $height += 38 * (() = $html =~ /<h1\b/gi);
    $height += 12 * (() = $html =~ /<h[23]\b/gi);
    # Pictures: their stated height, else a guess.
    for my $img ($html =~ /<img\b[^>]*>/gi) {
        $height += $img =~ /height\s*=\s*["']?(\d+)/i ? $1 + 8 : 160;
    }
    $height = 60 if $height < 60;
    $height = 1400 if $height > 1400;
    return $height;
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


sub load {
    if (-f $db_path && have_sqlite()) {
        my $archive = eval { read_archive() };
        if ($archive) {
            $archive->{files} = eval { read_files() } || [];
            ($data, $mode, $read_at) = ($archive, 'archive', time);
            decorate($data);
            return;
        }
        print STDERR "vault: reading $db_path failed: $@";
    }
    ($data, $mode, $read_at) = (read_sample(), 'sample', time);
    return unless $data;
    decorate($data);
    $data->{files} = sample_files($data);
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
            files => [ map { file_card(find_file($_->{aid}) // $_) } @{ $m->{files} // [] } ],
            html => html_view($m),
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

# The rendered form of a message with an HTML part: a page for each look,
# with remote pictures only when the user let them load for it.
sub html_view {
    my ($m) = @_;
    return undef unless defined $m->{html} && ($m->{kind} // '') =~ /^(|email)$/;
    my $remote = remote_content($m->{html});
    my $allowed = $remote && $view{remote}{ $m->{id} } ? 1 : 0;
    return {
        light => page($m->{id}, $m->{html}, 0, $allowed),
        dark => page($m->{id}, $m->{html}, 1, $allowed),
        height => page_height($m->{html}),
        remote => $remote ? $TRUE : $FALSE,
        allowed => $allowed ? $TRUE : $FALSE,
    };
}

# The attachments browser: every file, filtered by kind and a search.
sub files_view {
    my ($now) = @_;
    my @all = @{ $data->{files} // [] };
    @all = grep { my $f = $_; grep { $_->{id} == ($f->{msg} // 0) && ($_->{src} // 0) == $view{account} } @{ $data->{messages} } } @all
        if $view{account};
    my %count;
    $count{ file_kind($_) }++ for @all;
    my @kinds = ({ id => 'all', title => 'All', count => scalar @all });
    push @kinds, { id => $_->[0], title => $_->[1], count => $count{ $_->[0] } } for grep { $count{ $_->[0] } } @FILE_KINDS;
    my $kind = $view{fkind} // 'all';
    $kind = 'all' unless grep { $_->{id} eq $kind } @kinds;
    my $query = lc($view{fquery} // '');
    my @rows = grep {
        ($kind eq 'all' || file_kind($_) eq $kind)
            && ($query eq '' || index(lc(join ' ', $_->{name} // '', $_->{from} // '', $_->{subject} // ''), $query) >= 0)
    } @all;
    my $size = 0;
    $size += $_->{size} // 0 for @rows;
    my $more = @rows > $ROWS;
    @rows = @rows[0 .. $ROWS - 1] if $more;
    my ($selected) = grep { $_->{aid} eq ($view{file} // '') } @all;
    return {
        title => 'All Attachments',
        subtitle => plural(scalar @rows, 'file', 'files') . ($more ? '+' : '') . ' · ' . bytes($size),
        kinds => \@kinds,
        kind => $kind,
        query => $view{fquery} // '',
        rows => [ map { file_card($_, $now) } @rows ],
        selected => $selected ? $selected->{aid} : '',
        file => $selected ? file_card($selected) : undef,
    };
}

# ---- the view ------------------------------------------------------------


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
    my $files = $view{mailbox} eq 'files' ? files_view($now) : undef;
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
        total_files => scalar @{ $data->{files} // [] },
        files => $files,
        notice => $notice,
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
    if (defined $msg->{query}) {
        $view{query} = "$msg->{query}";
        # A search is over messages: leave the attachments browser for it.
        $view{mailbox} = 'all' if $view{query} =~ /\S/ && $view{mailbox} eq 'files';
    }
    $view{open} = int $msg->{open} if defined $msg->{open};
    $view{fkind} = "$msg->{kind}" if defined $msg->{kind};
    # The browser's search field sends its text as `value`.
    $view{fquery} = defined $msg->{value} ? "$msg->{value}" : '' if defined $msg->{fquery};
    $view{fquery} = '' if $msg->{fquery_clear};
    $view{file} = "$msg->{file}" if defined $msg->{file};
    if (defined $msg->{show}) {
        # From the attachments browser to the message a file came in.
        $view{mailbox} = 'all';
        $view{query} = '';
        $view{open} = int $msg->{show};
    }
    $view{remote}{ int $msg->{remote} } = 1 if $msg->{remote};
    my $from = $envelope->{from} // '';
    if ($from =~ /^(reader|list)$/) {
        deliver("$msg->{open_file}", 0) if defined $msg->{open_file};
        deliver("$msg->{save_file}", 1) if defined $msg->{save_file};
    }
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

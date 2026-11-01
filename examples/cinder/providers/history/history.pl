#!/usr/bin/env perl
# Cinder's sessions: the coding-agent CLI sessions in ~/.claude/projects, read
# only. Prints one JSON line whenever what it shares changes; reads the app's
# messages, one `{"from": ..., "message": ...}` per line, on stdin. Exits when
# the app closes stdin. It never writes a file.
#
# Output (`app.history`):
#   available, source ("cli" or "sample"), total (sessions found)
#   groups      { name, path, sessions = { id, title, branch, age, pr, gone } },
#               most recent first; `pr` when the session opened a pull request,
#               `gone` when its folder no longer exists
#   transcript  the opened session, or absent:
#               { id, title, project, branch, hidden (earlier turns left out),
#                 turns = { from ("user" or "assistant"),
#                           parts = { kind ("text" or "tools"), text, tools = { name, detail } } } }
#
# Messages: { open = id } opens a transcript, { close = true } drops it,
# { refresh = true } looks again.

use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use File::Basename qw(basename);

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $out = JSON::PP->new->canonical->utf8(0);
my $in = JSON::PP->new->utf8(1);

my $HOME = $ENV{HOME} // '';
my $ROOT = "$HOME/.claude/projects";
my $SAMPLE = ($ENV{CINDER_SAMPLE} // '') ne '' || !-d $ROOT;
my $MOST = 60;          # sessions listed
my $TURNS = 150;        # turns kept from a transcript, the latest
my $TEXT = 5000;        # characters per text part
my $TOOLS = 40;         # tool calls per group

my %cache;              # path => { mtime, size, meta }
my ($open, $total, @groups);
my %shown;              # the transcript last read, by session and mtime

sub clip {
    my ($text, $max) = @_;
    return '' unless defined $text;
    return length($text) > $max ? substr($text, 0, $max - 1) . '…' : $text;
}

sub one_line {
    my ($text) = @_;
    $text //= '';
    $text =~ s/^\s+//;
    $text =~ s/\s+/ /g;
    return clip($text, 90);
}

# The text a person typed, or undef for tool results, commands and reminders.
sub typed {
    my ($entry) = @_;
    return undef if $entry->{isSidechain} || $entry->{isMeta};
    my $content = ref $entry->{message} eq 'HASH' ? $entry->{message}{content} : undef;
    my $text;
    if (!ref $content) {
        $text = $content;
    }
    elsif (ref $content eq 'ARRAY') {
        for my $block (@$content) {
            next unless ref $block eq 'HASH';
            return undef if ($block->{type} // '') eq 'tool_result';
            if (($block->{type} // '') eq 'text') { $text = $block->{text}; last }
        }
    }
    return undef unless defined $text && $text =~ /\S/;
    return undef if $text =~ /^\s*</ || $text =~ /^\s*Caveat:/;
    return $text;
}

# Where a session ran, as a repository name: worktrees go under their repo.
sub project_of {
    my ($cwd, $dir) = @_;
    if (defined $cwd && length $cwd) {
        return $1 if $cwd =~ m{/\.herdr/worktrees/([^/]+)/};
        $cwd =~ s{/\.claude/worktrees/[^/]+/?$}{};
        return basename($cwd);
    }
    my $name = basename($dir);
    $name =~ s/.*-//;
    return $name || 'unknown';
}

sub age {
    my ($mtime) = @_;
    my $seconds = time - $mtime;
    return 'now' if $seconds < 60;
    return int($seconds / 60) . 'm' if $seconds < 3600;
    return int($seconds / 3600) . 'h' if $seconds < 86400;
    return int($seconds / 86400) . 'd' if $seconds < 86400 * 30;
    return int($seconds / (86400 * 30)) . 'mo';
}

# A JSON string's value from a match of its escaped text.
sub unescape {
    my ($text) = @_;
    return undef unless defined $text;
    return eval { $in->decode(qq{"$text"}) };
}

# The line of `$all` holding position `$at`.
sub line_at {
    my ($all, $at) = @_;
    my $end = index($$all, "\n", $at);
    return substr($$all, $at, ($end < 0 ? length $$all : $end) - $at);
}

# A session's title, folder and branch. Transcripts run to tens of
# megabytes and JSON::PP is slow, so this reads the file whole and searches
# it, decoding only the few lines it needs.
sub meta_of {
    my ($path) = @_;
    open my $fh, '<:raw', $path or return undef;
    my $all = do { local $/; <$fh> };
    close $fh;
    return undef unless defined $all;
    my ($ai, $summary, $first, $cwd, $branch);
    my $head = substr($all, 0, 262144);
    $cwd = unescape($1) if $head =~ /"cwd":"((?:[^"\\]|\\.)*)"/;
    $branch = unescape($1) if $head =~ /"gitBranch":"((?:[^"\\]|\\.)*)"/;
    my $pr = index($all, 'gh pr create') >= 0;
    # The latest title the CLI gave it wins.
    my $at = rindex($all, '{"type":"ai-title"');
    if ($at >= 0 && ($at == 0 || substr($all, $at - 1, 1) eq "\n")) {
        my $entry = eval { $in->decode(line_at(\$all, $at)) };
        $ai = $entry->{aiTitle} if ref $entry eq 'HASH';
    }
    $at = index($all, '{"type":"summary"');
    if (!defined $ai && $at >= 0) {
        my $entry = eval { $in->decode(line_at(\$all, $at)) };
        $summary = $entry->{summary} if ref $entry eq 'HASH';
    }
    if (!defined $ai && !defined $summary) {
        # Else the first thing typed, from the first few megabytes.
        for my $line (split /\n/, substr($all, 0, 4 << 20)) {
            next unless length($line) < 65536 && index($line, '"type":"user"') >= 0 && index($line, '"tool_result"') < 0;
            my $entry = eval { $in->decode($line) };
            $first = typed($entry) if ref $entry eq 'HASH';
            last if defined $first;
        }
    }
    my $title = $ai // $summary // $first;
    return undef unless defined $title;
    return { title => one_line($title), cwd => $cwd, branch => $branch, pr => $pr ? 1 : 0 };
}

sub scan {
    if ($SAMPLE) {
        sample();
        return;
    }
    my @files;
    opendir my $root, $ROOT or return;
    for my $dir (grep { !/^\./ } readdir $root) {
        opendir my $dh, "$ROOT/$dir" or next;
        for my $file (grep { /\.jsonl$/ } readdir $dh) {
            my $path = "$ROOT/$dir/$file";
            my @stat = stat $path or next;
            push @files, { path => $path, dir => $dir, mtime => $stat[9], size => $stat[7] };
        }
        closedir $dh;
    }
    closedir $root;
    @files = sort { $b->{mtime} <=> $a->{mtime} } @files;
    $total = scalar @files;

    my (%seen, @order, %by);
    my $listed = 0;
    for my $file (@files) {
        last if $listed >= $MOST;
        my $hit = $cache{ $file->{path} };
        if (!$hit || $hit->{mtime} != $file->{mtime} || $hit->{size} != $file->{size}) {
            $hit = $cache{ $file->{path} } = { mtime => $file->{mtime}, size => $file->{size}, meta => meta_of($file->{path}) };
        }
        my $meta = $hit->{meta} or next;
        my $name = project_of($meta->{cwd}, $file->{dir});
        if (!$seen{$name}++) {
            push @order, $name;
            $by{$name} = { name => $name, path => $meta->{cwd} // '', sessions => [] };
        }
        my $id = basename($file->{path}, '.jsonl');
        push @{ $by{$name}{sessions} }, {
            id => $id,
            title => $meta->{title},
            branch => $meta->{branch} // '',
            age => age($file->{mtime}),
            pr => $meta->{pr} ? JSON::PP::true : JSON::PP::false,
            gone => (defined $meta->{cwd} && !-d $meta->{cwd}) ? JSON::PP::true : JSON::PP::false,
            path => $file->{path},
        };
        $listed++;
    }
    @groups = map { $by{$_} } @order;
}

sub find_session {
    my ($id) = @_;
    for my $group (@groups) {
        for my $session (@{ $group->{sessions} }) {
            return ($session, $group) if $session->{id} eq $id;
        }
    }
    return;
}

sub tool_detail {
    my ($input, $cwd) = @_;
    return '' unless ref $input eq 'HASH';
    my $detail = $input->{description} // $input->{command} // $input->{file_path} // $input->{path}
        // $input->{pattern} // $input->{url} // $input->{query} // $input->{prompt} // '';
    $detail = '' if ref $detail;
    $detail =~ s/^\Q$cwd\E\/// if defined $cwd && length $cwd;
    return one_line($detail);
}

sub transcript {
    my ($id) = @_;
    my ($session, $group) = find_session($id);
    return undef unless $session;
    return sample_transcript($session, $group) if $SAMPLE;
    open my $fh, '<:raw', $session->{path} or return undef;
    my $cwd = $group->{path};
    my @turns;
    while (my $line = <$fh>) {
        # Tool results are left out, and decoding them is the slow part.
        next if index($line, '"tool_result"') >= 0 && index($line, '"type":"user"') >= 0;
        next unless index($line, '"type":"user"') >= 0 || index($line, '"type":"assistant"') >= 0;
        my $assistant = index($line, '"role":"assistant"') >= 0;
        # Thinking alone shows nothing here.
        next if $assistant && index($line, '"type":"thinking"') >= 0 && index($line, '"type":"text"') < 0
            && index($line, '"type":"tool_use"') < 0;
        # A large tool call (a whole file written): its name and a line from
        # its input are enough, without decoding it.
        if ($assistant && length($line) > 16384 && index($line, '"type":"tool_use"') >= 0
            && index($line, '"type":"text"') < 0 && index($line, '"isSidechain":true') < 0) {
            my ($name) = $line =~ /"type":"tool_use","id":"[^"]*","name":"([^"]+)"/;
            my ($detail) = $line =~ /"(?:description|file_path|command)":"((?:[^"\\]|\\.){0,300})/;
            push @turns, { from => 'assistant', parts => [] } unless @turns && $turns[-1]{from} eq 'assistant';
            my $parts = $turns[-1]{parts};
            push @$parts, { kind => 'tools', text => '', tools => [] } unless @$parts && $parts->[-1]{kind} eq 'tools';
            my $value = unescape($detail) // '';
            $value =~ s/^\Q$cwd\E\/// if length $cwd;
            push @{ $parts->[-1]{tools} }, { name => $name // 'Tool', detail => one_line($value) }
                if @{ $parts->[-1]{tools} } < $TOOLS;
            next;
        }
        my $entry = eval { $in->decode($line) };
        next unless ref $entry eq 'HASH' && !$entry->{isSidechain};
        if (($entry->{type} // '') eq 'user') {
            my $text = typed($entry);
            push @turns, { from => 'user', parts => [ { kind => 'text', text => clip($text, $TEXT), tools => [] } ] }
                if defined $text;
            next;
        }
        next unless ($entry->{type} // '') eq 'assistant' && ref $entry->{message} eq 'HASH';
        my $content = $entry->{message}{content};
        next unless ref $content eq 'ARRAY';
        push @turns, { from => 'assistant', parts => [] } unless @turns && $turns[-1]{from} eq 'assistant';
        my $parts = $turns[-1]{parts};
        for my $block (@$content) {
            next unless ref $block eq 'HASH';
            my $type = $block->{type} // '';
            if ($type eq 'text' && ($block->{text} // '') =~ /\S/) {
                push @$parts, { kind => 'text', text => clip($block->{text}, $TEXT), tools => [] };
            }
            elsif ($type eq 'tool_use') {
                push @$parts, { kind => 'tools', text => '', tools => [] } unless @$parts && $parts->[-1]{kind} eq 'tools';
                my $tools = $parts->[-1]{tools};
                push @$tools, { name => $block->{name} // 'Tool', detail => tool_detail($block->{input}, $cwd) }
                    if @$tools < $TOOLS;
            }
        }
    }
    close $fh;
    @turns = grep { @{ $_->{parts} } } @turns;
    my $hidden = @turns > $TURNS ? @turns - $TURNS : 0;
    splice @turns, 0, $hidden;
    return { id => $id, title => $session->{title}, project => $group->{name}, branch => $session->{branch},
        hidden => $hidden, turns => \@turns };
}

# Sessions to show when there are none to read (or CINDER_SAMPLE is set).
sub sample {
    my $n = 0;
    my $s = sub {
        my ($title, $branch, $pr, $gone, $age) = @_;
        return { id => 'sample-' . ++$n, title => $title, branch => $branch, age => $age,
            pr => $pr ? JSON::PP::true : JSON::PP::false, gone => $gone ? JSON::PP::true : JSON::PP::false };
    };
    @groups = (
        { name => 'lantern', path => '~/code/lantern', sessions => [
            $s->('Lantern dashboard and web tasks view', 'feat/dashboard', 1, 0, '12m'),
            $s->('Next-step plan review for the job queue', 'main', 0, 0, '1h'),
        ] },
        { name => 'DragKit', path => '~/code/DragKit', sessions => [
            $s->('Cmd-click drag not working on Sonoma', 'fix/cmd-drag', 0, 0, '3h'),
        ] },
        { name => 'tideline', path => '~/code/tideline', sessions => [
            $s->('Phase 2 string catalogue migration', 'i18n/phase-2', 0, 0, '5h'),
            $s->('tideline-usage catalogue migration', 'i18n/usage', 0, 0, '6h'),
            $s->('Activity bar translations', 'i18n/activity', 0, 0, '8h'),
            $s->('I18N.md phase 2 plan', 'main', 0, 0, '9h'),
            $s->('Multilanguage support for the renderer', 'i18n/core', 0, 0, '1d'),
            $s->('Merge main and resolve conflicts in the store', 'merge/main', 1, 1, '1d'),
            $s->('Merge main and fix the lint score', 'merge/lint', 1, 1, '2d'),
            $s->('PR #904 mods and layouts', 'feat/layouts', 0, 0, '2d'),
            $s->('New release push', 'release/1.8', 0, 0, '3d'),
            $s->('tideline Windows daemon connection', 'fix/windows', 0, 0, '4d'),
        ] },
    );
    $total = 10;
}

sub sample_transcript {
    my ($session, $group) = @_;
    my $tools = sub { { kind => 'tools', text => '', tools => [ map { { name => $_->[0], detail => $_->[1] } } @_ ] } };
    my $text = sub { { kind => 'text', text => $_[0], tools => [] } };
    return { id => $session->{id}, title => $session->{title}, project => $group->{name},
        branch => $session->{branch}, hidden => 0, turns => [
        { from => 'user', parts => [ $text->($session->{title} . '. Start by reading how it works today, then propose the change.') ] },
        { from => 'assistant', parts => [
            $text->("I'll look at the current code first."),
            $tools->([ 'Bash', 'List the source tree' ], [ 'Read', 'src/app.rs' ], [ 'Grep', 'fn render' ]),
            $text->("Here is what I found:\n\n- The view rebuilds its whole tree on every change.\n- The store keeps one list per window.\n\nI'd split the list out and keep it in the store, then render it lazily:\n\n```rust\nlet rows = store.rows(window);\nlist.set_count(rows.len());\n```\n\nShall I go ahead?"),
        ] },
        { from => 'user', parts => [ $text->('Yes, go ahead and run the tests after.') ] },
        { from => 'assistant', parts => [
            $tools->([ 'Edit', 'src/store.rs' ], [ 'Edit', 'src/app.rs' ], [ 'Bash', 'Run the tests' ]),
            $text->('Done: the list lives in the store now, and all 214 tests pass.'),
        ] },
    ] };
}

sub publish {
    my $data = { available => JSON::PP::true, source => $SAMPLE ? 'sample' : 'cli', total => $total // 0,
        groups => [ map { +{ %$_, sessions => [ map { my %s = %$_; delete $s{path}; \%s } @{ $_->{sessions} } ] } } @groups ] };
    if (defined $open) {
        # Read again only when the session's file changed.
        my ($session) = find_session($open);
        my $key = $session ? $open . ':' . ($session->{path} ? (stat $session->{path})[9] // 0 : 0) : '';
        if (!defined $shown{key} || $shown{key} ne $key) {
            %shown = (key => $key, transcript => transcript($open));
        }
        $data->{transcript} = $shown{transcript} if $shown{transcript};
    }
    return $out->encode($data);
}

sub receive {
    my ($line) = @_;
    my $value = eval { $in->decode($line) };
    my $msg = ref $value eq 'HASH' ? $value->{message} : undef;
    return unless ref $msg eq 'HASH';
    if (defined $msg->{open} && !ref $msg->{open} && $msg->{open} =~ /^[A-Za-z0-9_-]{1,80}$/) {
        $open = $msg->{open};
    }
    elsif ($msg->{close}) {
        undef $open;
    }
}

scan();
my $last = '';
my $select = IO::Select->new(\*STDIN);
my $buffer = '';
my $scanned = time;
while (1) {
    my $text = publish();
    if ($text ne $last) {
        print $text, "\n";
        $last = $text;
    }
    if ($select->can_read(30)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        receive($1) while $buffer =~ s/^([^\n]*)\n//;
    }
    if (time - $scanned >= 30) {
        scan();
        $scanned = time;
    }
}

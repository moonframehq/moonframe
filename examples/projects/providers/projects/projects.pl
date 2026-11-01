#!/usr/bin/env perl
# The projects on this machine, for apps that work across many repositories:
# every git repository under the project folders, with its branch and its
# changes, and for the one an app has open, its changed files, its folders
# and one file's text. Prints one JSON line whenever that changes; reads the
# app's messages, one `{"message": ...}` per line, on stdin. Exits when the
# app closes stdin.
#
# Output (`app.projects`):
#   now       the time, in Unix seconds (views have no clock)
#   roots     the project folders looked in, as ~/...
#   projects  up to MAX_PROJECTS, most recently active first:
#             { path, name, home (its project folder, as ~/...), branch,
#               files, added, deleted, ahead, behind, time, subject }
#   current   the open project (the most recently active one until a
#             message selects another), or absent:
#             { path, name, home, branch, scope, added, deleted, count,
#               truncated, files = { "<status>\t<added>\t<deleted>\t<path>" } }
#             status is M, A, D or R; untracked files are A
#   dirs      the open project's folders that are open, by path from its
#             root ("" is the root): { "<kind><name>" }, where kind is d (a
#             folder), f (a file), and D or F when git ignores it
#   file      the file being read: { path, lines, total, truncated, binary,
#             missing }
#
# Messages (any combination of keys in one table):
#   select = "<path>"   open that project
#   scope = "uncommitted" | "branch" | "last"   which changes `current` lists
#   open = "a\na/b"     the open project's open folders, one per line
#   read = "<path>"     read that file of the open project; false stops
#   refresh = true      look again now
#
# The project folders are MOONFRAME_PROJECTS (colon-separated), else those
# of ~/code ~/dev ~/src ~/Projects ~/projects ~/work ~/.herdr/worktrees
# that exist. Each is searched two levels deep. It runs git with your
# access, and reads only inside the open project.
#
# Perl 5 and its core JSON::PP; values are capped to stay well under the
# host's limits (10,000 values and 1 MiB a line).

use strict;
use warnings;
use JSON::PP ();
use IO::Select;
use File::Spec;

$| = 1;

my $MAX_PROJECTS = 40;
my $MAX_FILES    = 300;     # entries in current.files
my $MAX_ENTRIES  = 400;     # entries listed per folder
my $MAX_DIRS     = 60;      # open folders listed
my $MAX_LINES    = 3000;    # lines of a file read
my $MAX_LINE     = 400;     # characters a line keeps
my $MAX_BYTES    = 2 * 1024 * 1024;
my $SCAN_EVERY   = 15;      # seconds between looks for projects
my $LOOK_EVERY   = 3;       # seconds between looks at the open project
my %HIDDEN       = ('.git' => 1, '.DS_Store' => 1);

my $HOME = $ENV{HOME} // '';
my $json = JSON::PP->new->canonical->utf8(0);
binmode STDOUT, ':encoding(UTF-8)';

my @roots = grep { -d } map { s{^~(?=/|$)}{$HOME}r }
    split /:/, ($ENV{MOONFRAME_PROJECTS}
        // join ':', map { "$HOME/$_" } qw(code dev src Projects projects work .herdr/worktrees));

# ------------------------------------------------------------- helpers ----

sub tilde {
    my ($path) = @_;
    return $path =~ s{^\Q$HOME\E(?=/|$)}{~}r if length $HOME;
    return $path;
}

# Runs git in $dir; its output, or undef when it fails.
sub git {
    my ($dir, @args) = @_;
    my $pid = open(my $out, '-|');
    return undef unless defined $pid;
    if (!$pid) {
        open STDERR, '>', File::Spec->devnull;
        $ENV{GIT_OPTIONAL_LOCKS} = '0';
        exec 'git', '-C', $dir, @args;
        exit 127;
    }
    local $/;
    my $text = <$out>;
    close $out;
    return $? == 0 ? ($text // '') : undef;
}

# The git folder of the work tree at $dir: `.git`, or where a worktree's
# `.git` file points.
sub git_dir {
    my ($dir) = @_;
    my $dot = "$dir/.git";
    return $dot if -d $dot;
    if (-f $dot && open my $fh, '<', $dot) {
        my $line = <$fh> // '';
        close $fh;
        if ($line =~ /^gitdir:\s*(.+?)\s*$/) {
            my $target = $1;
            $target = "$dir/$target" unless $target =~ m{^/};
            return $target;
        }
    }
    return undef;
}

sub mtime { my @s = stat $_[0]; return @s ? $s[9] : 0 }

# ------------------------------------------------------------ projects ----

my %summary;    # path -> { key, data }

sub find_projects {
    my @found;
    my %seen;
    for my $root (@roots) {
        for my $depth (1, 2) {
            my @dirs = $depth == 1 ? glob("\Q$root\E/*") : glob("\Q$root\E/*/*");
            for my $dir (@dirs) {
                next if $seen{$dir} || !-d $dir || !-e "$dir/.git";
                next if grep { index($dir, "$_/") == 0 } keys %seen;    # inside another
                $seen{$dir} = 1;
                # A project two deep groups under its parent: the
                # worktrees of one repository, say.
                my $group = $depth == 1 ? $root : ($dir =~ s{/[^/]+$}{}r);
                push @found, { path => $dir, group => $group };
            }
        }
    }
    return @found;
}

# What a project's summary depends on: its git folder's activity, and the
# half minute, since edits to the work tree touch nothing there.
sub activity {
    my ($dir) = @_;
    my $gd = git_dir($dir) // "$dir/.git";
    my $time = 0;
    for my $file ("$gd/index", "$gd/HEAD", "$gd/logs/HEAD") {
        my $t = mtime($file);
        $time = $t if $t > $time;
    }
    return $time;
}

# The script run in each project: status, line counts, and the last
# commit, separated by \001.
my $SUMMARY = q{
    export GIT_OPTIONAL_LOCKS=0
    git -C "$1" status --porcelain=v2 --branch -z --untracked-files=normal
    printf '\001'
    git -C "$1" diff HEAD --numstat --no-renames
    printf '\001'
    git -C "$1" log -1 --format='%ct%x09%s'
};

# Branch, ahead and behind, changed files and lines, and the last commit of
# each project, each project's git running at once with the others'.
sub summarise_all {
    my @projects = @_;
    my (@running, @out);
    for my $project (@projects) {
        my $dir = $project->{path};
        my $time = activity($dir);
        my $key = join ':', $time, int(time / 30);
        my $cached = $summary{$dir};
        if ($cached && $cached->{key} eq $key) {
            push @out, $cached->{data};
            next;
        }
        my $pid = open(my $fh, '-|');
        next unless defined $pid;
        if (!$pid) {
            open STDERR, '>', File::Spec->devnull;
            exec 'sh', '-c', $SUMMARY, 'summary', $dir;
            exit 127;
        }
        push @running, [$project, $fh, $time, $key];
    }
    for my $job (@running) {
        my ($project, $fh, $time, $key) = @$job;
        my $text = do { local $/; <$fh> } // '';
        close $fh;
        my $dir = $project->{path};
        my ($status, $numstat, $log) = split /\001/, $text, 3;
        my %data = (
            path => $dir, name => ($dir =~ m{([^/]+)$})[0], home => tilde($project->{group}),
            branch => '', files => 0, added => 0, deleted => 0, ahead => 0, behind => 0,
            time => $time, subject => '',
        );
        for my $entry (split /\0/, $status // '') {
            if ($entry =~ /^# branch\.head (.+)$/) { $data{branch} = $1 eq '(detached)' ? 'HEAD' : $1 }
            elsif ($entry =~ /^# branch\.ab \+(\d+) -(\d+)$/) { @data{qw(ahead behind)} = ($1 + 0, $2 + 0) }
            elsif ($entry =~ /^[12u?] /) { $data{files}++ }
        }
        for (split /\n/, $numstat // '') {
            my ($a, $d) = split /\t/;
            $data{added} += $a if defined $a && $a =~ /^\d+$/;
            $data{deleted} += $d if defined $d && $d =~ /^\d+$/;
        }
        if (defined $log && $log =~ /^(\d+)\t(.*)/) {
            $data{subject} = substr($2, 0, 200);
            $data{time} = $1 + 0 if $1 > $data{time};
        }
        # JSON::PP writes a value last used as a string as a string.
        $data{$_} += 0 for qw(files added deleted ahead behind time);
        $summary{$dir} = { key => $key, data => \%data };
        push @out, \%data;
    }
    return @out;
}

# ------------------------------------------------------------- current ----

my %state = (select => undef, scope => 'uncommitted', open => [], read => undef);

# What the app asked for, kept across restarts (an edit to this folder, an
# update), so a file being read or a project being shown stays so; the
# mods asked once and will not ask again. In the cache, not this folder,
# whose changes restart the provider. One file per app (profile).
my $CACHE = ($ENV{XDG_CACHE_HOME} && $ENV{XDG_CACHE_HOME} =~ m{^/} ? $ENV{XDG_CACHE_HOME} : "$HOME/.cache")
    . '/moonframe/projects';
my $KEPT = do {
    # The host names the profile it runs; outside one, `default`.
    my $who = $ENV{MOONFRAME_PROFILE} // 'default';
    $who =~ s/[^A-Za-z0-9_-]/-/g;
    "$CACHE/$who.json";
};

sub restore {
    open my $fh, '<', $KEPT or return;
    local $/;
    my $kept = eval { $json->decode(<$fh>) };
    close $fh;
    return unless ref $kept eq 'HASH';
    $state{select} = $kept->{select} if defined $kept->{select} && !ref $kept->{select} && -d $kept->{select};
    $state{scope} = $kept->{scope} if defined $kept->{scope} && $kept->{scope} =~ /^(uncommitted|branch|last)$/;
    $state{open} = [grep { !ref } @{ $kept->{open} }] if ref $kept->{open} eq 'ARRAY';
    $state{read} = $kept->{read} if defined $kept->{read} && !ref $kept->{read};
}

sub keep {
    system 'mkdir', '-p', $CACHE unless -d $CACHE;
    open my $fh, '>', "$KEPT.tmp" or return;
    print $fh $json->encode(\%state);
    close $fh;
    rename "$KEPT.tmp", $KEPT;
}

# The commit a branch's changes count from: where it left the default branch.
sub branch_base {
    my ($dir) = @_;
    for my $ref ('origin/HEAD', 'origin/main', 'origin/master', 'main', 'master') {
        my $base = git($dir, 'merge-base', 'HEAD', $ref);
        if (defined $base && $base =~ /^([0-9a-f]+)/) {
            return $1;
        }
    }
    return undef;
}

sub changes {
    my ($dir, $scope) = @_;
    my @range;
    if ($scope eq 'last') { @range = ('HEAD~1', 'HEAD') }
    elsif ($scope eq 'branch') {
        my $base = branch_base($dir);
        @range = ($base // 'HEAD');
    }
    else { @range = ('HEAD') }

    my (%files, @order);
    my $names = git($dir, 'diff', @range, '--name-status', '-z', '--no-renames');
    if (defined $names) {
        my @parts = split /\0/, $names;
        while (@parts >= 2) {
            my ($status, $path) = splice @parts, 0, 2;
            $status = substr($status, 0, 1);
            $files{$path} = { status => $status, added => 0, deleted => 0 };
            push @order, $path;
        }
    }
    my $numstat = git($dir, 'diff', @range, '--numstat', '-z', '--no-renames');
    if (defined $numstat) {
        for (split /\0/, $numstat) {
            my ($a, $d, $path) = split /\t/, $_, 3;
            next unless defined $path && $files{$path};
            $files{$path}{added} = $a =~ /^\d+$/ ? $a + 0 : 0;
            $files{$path}{deleted} = $d =~ /^\d+$/ ? $d + 0 : 0;
        }
    }
    if ($scope ne 'last') {
        my $untracked = git($dir, 'ls-files', '--others', '--exclude-standard', '-z');
        for my $path (split /\0/, $untracked // '') {
            next if $files{$path};
            my $lines = 0;
            my $full = "$dir/$path";
            if (-f $full && -s _ < 512 * 1024 && open my $fh, '<', $full) {
                $lines++ while <$fh>;
                close $fh;
            }
            $files{$path} = { status => 'A', added => $lines, deleted => 0 };
            push @order, $path;
        }
    }
    my ($added, $deleted) = (0, 0);
    my @list;
    for my $path (sort @order) {
        my $f = $files{$path};
        $added += $f->{added};
        $deleted += $f->{deleted};
        push @list, join "\t", $f->{status}, $f->{added}, $f->{deleted}, $path if @list < $MAX_FILES;
    }
    return {
        files => \@list, count => scalar @order, truncated => (@order > $MAX_FILES ? JSON::PP::true : JSON::PP::false),
        added => $added, deleted => $deleted,
    };
}

# A path inside the project, by a relative path without `..`.
sub inside {
    my ($rel) = @_;
    return 0 if !defined $rel || $rel =~ m{(^|/)\.\.(/|$)} || $rel =~ m{^/};
    return 1;
}

sub list_dir {
    my ($root, $rel) = @_;
    my $base = $rel eq '' ? $root : "$root/$rel";
    opendir(my $dh, $base) or return undef;
    my @names = grep { $_ ne '.' && $_ ne '..' && !$HIDDEN{$_} && !/\n/ } readdir $dh;
    closedir $dh;
    my (@dirs, @files);
    for (@names) { if (-d "$base/$_") { push @dirs, $_ } else { push @files, $_ } }
    @dirs = sort { lc $a cmp lc $b } @dirs;
    @files = sort { lc $a cmp lc $b } @files;
    my @all = ((map { [$_, 1] } @dirs), (map { [$_, 0] } @files));
    splice @all, $MAX_ENTRIES if @all > $MAX_ENTRIES;
    my %ignored;
    if (@all) {
        my $input = join '', map { ($rel eq '' ? '' : "$rel/") . $_->[0] . "\0" } @all;
        my $pid = open(my $io, '-|');
        if (defined $pid && !$pid) {
            open STDERR, '>', File::Spec->devnull;
            open my $in, '|-', 'git', '-C', $root, 'check-ignore', '-z', '--stdin' or exit 1;
            print $in $input;
            close $in;
            exit 0;
        }
        if ($pid) {
            local $/ = "\0";
            while (my $path = <$io>) {
                chomp $path;
                $ignored{($path =~ m{([^/]+)$})[0]} = 1;
            }
            close $io;
        }
    }
    return [map {
        my ($name, $dir) = @$_;
        my $kind = $dir ? 'd' : 'f';
        $kind = uc $kind if $ignored{$name};
        "$kind$name";
    } @all];
}

my %file_cache;    # path -> { mtime, data }

sub read_file {
    my ($root, $rel) = @_;
    my $full = "$root/$rel";
    my $mtime = mtime($full);
    my $cached = $file_cache{$full};
    return $cached->{data} if $cached && $cached->{mtime} == $mtime;
    my %data = (path => $rel, lines => [], total => 0, truncated => JSON::PP::false,
        binary => JSON::PP::false, missing => JSON::PP::false);
    if (!-f $full) {
        $data{missing} = JSON::PP::true;
    }
    elsif (open my $fh, '<:raw', $full) {
        my $bytes = '';
        read $fh, $bytes, $MAX_BYTES;
        close $fh;
        if (index(substr($bytes, 0, 8192), "\0") >= 0) {
            $data{binary} = JSON::PP::true;
        }
        else {
            utf8::decode($bytes);
            my @lines = split /\r?\n/, $bytes, -1;
            pop @lines if @lines && $lines[-1] eq '';
            $data{total} = scalar @lines;
            if (@lines > $MAX_LINES) {
                splice @lines, $MAX_LINES;
                $data{truncated} = JSON::PP::true;
            }
            for (@lines) {
                s/\t/    /g;
                s/[\x00-\x08\x0b-\x1f\x7f]//g;
                $_ = substr($_, 0, $MAX_LINE) if length > $MAX_LINE;
            }
            $data{lines} = \@lines;
        }
    }
    $file_cache{$full} = { mtime => $mtime, data => \%data };
    return \%data;
}

# --------------------------------------------------------------- loop -----

my (@projects, $last_scan, $last_out) = ((), 0, '');

sub emit {
    my ($force) = @_;
    my $now = time;
    if ($force || $now - $last_scan >= $SCAN_EVERY) {
        @projects = sort { $b->{time} <=> $a->{time} } summarise_all(find_projects());
        splice @projects, $MAX_PROJECTS if @projects > $MAX_PROJECTS;
        $last_scan = $now;
        # Until an app picks one, the most recently active project is open.
        $state{select} //= $projects[0]{path} if @projects;
    }
    # To the minute: ages show no finer, and a clock that ticks every look
    # would publish, and redraw, with nothing else new.
    my %out = (available => JSON::PP::true, now => $now - $now % 60, roots => [map { tilde($_) } @roots],
        projects => \@projects);
    my $dir = $state{select};
    if (defined $dir && -d $dir && -e "$dir/.git") {
        my $parent = $dir =~ s{/[^/]+$}{}r;
        my ($summary) = summarise_all({ path => $dir, group => $parent });
        my $changes = changes($dir, $state{scope});
        $out{current} = { %$changes, path => $dir, name => $summary->{name}, home => $summary->{home},
            branch => $summary->{branch}, scope => $state{scope} };
        my %dirs;
        my $count = 0;
        for my $rel ('', @{ $state{open} }) {
            next unless inside($rel) || $rel eq '';
            last if $count++ > $MAX_DIRS;
            my $entries = list_dir($dir, $rel);
            $dirs{$rel} = $entries if $entries;
        }
        $out{dirs} = \%dirs;
        $out{file} = read_file($dir, $state{read}) if defined $state{read} && inside($state{read});
    }
    my $text = $json->encode(\%out);
    if ($force || $text ne $last_out) {
        print $text, "\n";
        $last_out = $text;
    }
}

sub receive {
    my ($line) = @_;
    my $value = eval { $json->decode($line) };
    return unless ref $value eq 'HASH';
    my $msg = $value->{message};
    return unless ref $msg eq 'HASH';
    if (exists $msg->{select} && defined $msg->{select} && !ref $msg->{select}) {
        my $path = $msg->{select} =~ s{^~(?=/|$)}{$HOME}r;
        if (!defined $state{select} || $state{select} ne $path) {
            $state{select} = $path;
            $state{open} = [];
            $state{read} = undef;
        }
    }
    if (defined $msg->{scope} && $msg->{scope} =~ /^(uncommitted|branch|last)$/) {
        $state{scope} = $msg->{scope};
    }
    if (exists $msg->{open} && !ref $msg->{open}) {
        $state{open} = [grep { length } split /\n/, ($msg->{open} // '')];
    }
    if (exists $msg->{read}) {
        $state{read} = (defined $msg->{read} && !ref $msg->{read}) ? $msg->{read} : undef;
    }
    keep();
}

restore();
my $select = IO::Select->new(\*STDIN);
my $buffer = '';
emit(1);
while (1) {
    my $changed = 0;
    if ($select->can_read($LOOK_EVERY)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        while ($buffer =~ s/^([^\n]*)\n//) {
            receive($1);
            $changed = 1;
        }
    }
    emit($changed);
}

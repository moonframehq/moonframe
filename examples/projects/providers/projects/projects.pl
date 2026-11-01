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
#               files, added, deleted, ahead, behind, time, subject,
#               worktrees }
#             worktrees: the repository's work trees, the main one first:
#             { path, branch, primary } (at most MAX_WORKTREES)
#   current   the open project (the most recently active one until a
#             message selects another), or absent:
#             { path, name, home, branch, scope, added, deleted, count,
#               truncated, files = { "<status>\t<added>\t<deleted>\t<path>" },
#               log = { { sha, subject, author, time } } }
#             status is M, A, D or R; untracked files are A; log is its
#             last MAX_LOG commits
#   dirs      the open project's folders that are open, by path from its
#             root ("" is the root): { "<kind><name>" }, where kind is d (a
#             folder), f (a file), and D or F when git ignores it
#   file      the file being read: { path, lines, total, truncated, binary,
#             missing }
#   forge     what `gh` says of the open project, once asked (`forge`):
#             { path, prs = { { number, title, branch, author, draft,
#               updated } }, runs = { { id, title, workflow, branch, status,
#               conclusion, created } }, error? }
#   action    the last write's outcome: { id, action, ok, message | error }
#   running   the write under way, by its action's name, or absent
#
# Messages (any combination of keys in one table):
#   select = "<path>"   open that project
#   scope = "uncommitted" | "branch" | "last"   which changes `current` lists
#   open = "a\na/b"     the open project's open folders, one per line
#   read = "<path>"     read that file of the open project; false stops
#   refresh = true      look again now
#   forge = true        ask `gh` for the open project's pull requests and
#                       workflow runs, again each minute while asked;
#                       false stops
#
# Writes, only from the mods named in MOONFRAME_PROJECTS_WRITERS (comma-
# separated; default `source-control`), one at a time and in the
# background, so a signing key waiting for a tap holds nothing up:
#   commit = "<message>", id = ...   stage everything in the open project
#                                    and commit it
#   pull = true, id = ...            pull the open project (fast-forward)
#   push = true, id = ...            push its branch to origin
#   worktree = "<name>", repo = "<path>", id = ...
#                                    add a work tree of that repository on
#                                    a new branch <name>, in
#                                    ~/.moonframe/worktrees/<repo>/<name>
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
my $MAX_WORKTREES = 12;     # work trees listed per project
my $MAX_LOG      = 30;      # commits in current.log
my $FORGE_EVERY  = 60;      # seconds between asks to gh
my $SCAN_EVERY   = 15;      # seconds between looks for projects
my $LOOK_EVERY   = 3;       # seconds between looks at the open project
my %HIDDEN       = ('.git' => 1, '.DS_Store' => 1);

my $HOME = $ENV{HOME} // '';
# An app opened from the Dock has no shell's PATH; gh is often in Homebrew's.
$ENV{PATH} = join ':', grep { length } $ENV{PATH} // '/usr/bin:/bin', '/opt/homebrew/bin', '/usr/local/bin';
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
    printf '\001'
    git -C "$1" worktree list --porcelain
};

# Branch, ahead and behind, changed files and lines, and the last commit of
# each project, each project's git running at once with the others'.
sub summarise_all {
    my @projects = @_;
    my (@running, @out);
    for my $project (@projects) {
        my $dir = $project->{path};
        my $time = activity($dir);
        # A work tree added or removed touches the main git folder's list.
        my $gd = git_dir($dir) // "$dir/.git";
        my $key = join ':', $time, int(time / 30), mtime("$gd/worktrees"), mtime("$gd/..");
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
        my ($status, $numstat, $log, $trees) = split /\001/, $text, 4;
        my %data = (
            path => $dir, name => ($dir =~ m{([^/]+)$})[0], home => tilde($project->{group}),
            branch => '', files => 0, added => 0, deleted => 0, ahead => 0, behind => 0,
            time => $time, subject => '', worktrees => [],
        );
        # The repository's work trees, the main one first, as git lists them.
        for my $block (split /\n\n/, $trees // '') {
            my ($path) = $block =~ /^worktree (.+)$/m;
            next if !defined $path || $block =~ /^bare$/m;
            last if @{ $data{worktrees} } >= $MAX_WORKTREES;
            my ($ref) = $block =~ m{^branch refs/heads/(.+)$}m;
            push @{ $data{worktrees} }, {
                path => $path, branch => $ref // 'HEAD',
                primary => (@{ $data{worktrees} } ? JSON::PP::false : JSON::PP::true),
            };
        }
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

# The last MAX_LOG commits of the open project.
sub commit_log {
    my ($dir) = @_;
    my $text = git($dir, 'log', "-$MAX_LOG", '--format=%h%x09%ct%x09%an%x09%s') // '';
    my @log;
    for (split /\n/, $text) {
        my ($sha, $time, $author, $subject) = split /\t/, $_, 4;
        next unless defined $subject;
        push @log, { sha => $sha, time => $time + 0, author => substr($author, 0, 60),
            subject => substr($subject, 0, 200) };
    }
    return \@log;
}

# ---------------------------------------------------------- background ----

# Writes and `gh`, which may wait (on the network, on a key to sign with),
# run in a child each; the loop reads their answers as they come.
my $select = IO::Select->new(\*STDIN);
my %jobs;    # fileno -> { fh, pid, text, done }
my $bytes_json = JSON::PP->new->canonical->utf8;

# Starts `$run` in a child; `$done` gets what it returned, in this process.
sub start_job {
    my ($run, $done) = @_;
    my $pid = open(my $fh, '-|');
    return 0 unless defined $pid;
    if (!$pid) {
        open STDERR, '>', File::Spec->devnull;
        my $result = eval { $run->() } // { ok => JSON::PP::false, error => "$@" };
        print $bytes_json->encode($result);
        exit 0;
    }
    $jobs{ fileno $fh } = { fh => $fh, pid => $pid, text => '', done => $done };
    $select->add($fh);
    return 1;
}

# Reads what a job printed; when it is done, hands it on. True when done.
sub read_job {
    my ($fh) = @_;
    my $job = $jobs{ fileno $fh } or return 0;
    my $got = sysread $fh, $job->{text}, 65536, length $job->{text};
    return 0 if $got;
    $select->remove($fh);
    delete $jobs{ fileno $fh };
    close $fh;
    waitpid $job->{pid}, 0;
    my $result = eval { $bytes_json->decode($job->{text}) };
    $job->{done}->(ref $result eq 'HASH' ? $result : { ok => JSON::PP::false, error => 'no answer' });
    return 1;
}

# Runs a command in the child, stderr with stdout: its exit status and text.
sub run {
    my ($dir, @command) = @_;
    my $pid = open(my $out, '-|');
    return (127, '') unless defined $pid;
    if (!$pid) {
        open STDERR, '>&', \*STDOUT;
        chdir $dir if defined $dir;
        $ENV{GIT_TERMINAL_PROMPT} = '0';
        exec @command;
        exit 127;
    }
    local $/;
    my $text = <$out> // '';
    close $out;
    utf8::decode($text);
    $text =~ s/\s+$//;
    return ($? >> 8, $text);
}

# --------------------------------------------------------------- writes ---

my %WRITERS = map { $_ => 1 } grep { length } split /\s*,\s*/,
    ($ENV{MOONFRAME_PROJECTS_WRITERS} // 'source-control,navigator');
my ($action, $running, $rescan);

sub finish {
    my ($name, $id, $result) = @_;
    $running = undef;
    $action = { %$result, action => $name, id => $id // $name };
    $action->{ok} = $action->{ok} ? JSON::PP::true : JSON::PP::false;
    $rescan = 1;    # look again: a commit or a work tree changes what shows
}

sub write_action {
    my ($from, $name, $id, $run) = @_;
    if (!$WRITERS{ $from // '' }) {
        finish($name, $id, { ok => 0, error => "`" . ($from // '?') . "` may not write; see MOONFRAME_PROJECTS_WRITERS" });
        return;
    }
    if (defined $running) {
        finish($name, $id, { ok => 0, error => "$running is still running" });
        return;
    }
    $running = $name;
    start_job($run, sub { finish($name, $id, $_[0]) }) or finish($name, $id, { ok => 0, error => 'could not start' });
}

sub commit {
    my ($dir, $message) = @_;
    my ($status, $text) = run($dir, 'git', 'add', '-A');
    return { ok => 0, error => $text || 'git add failed' } if $status;
    ($status, $text) = run($dir, 'git', 'commit', '-m', $message);
    return { ok => 0, error => $text || 'git commit failed' } if $status;
    my ($first) = split /\n/, $text;
    return { ok => 1, message => $first // 'Committed' };
}

sub sync {
    my ($dir, $what) = @_;
    my @command = $what eq 'pull' ? ('git', 'pull', '--ff-only') : ('git', 'push', '-u', 'origin', 'HEAD');
    my ($status, $text) = run($dir, @command);
    my @lines = grep { /\S/ } split /\n/, $text;
    return { ok => 0, error => $lines[-1] // "git $what failed" } if $status;
    return { ok => 1, message => $lines[-1] // ($what eq 'pull' ? 'Pulled' : 'Pushed') };
}

sub add_worktree {
    my ($repo, $name) = @_;
    my $base = ($repo =~ m{([^/]+)$})[0];
    my $where = "$HOME/.moonframe/worktrees/$base/$name";
    return { ok => 0, error => "$where already exists" } if -e $where;
    my ($status, $text) = run($repo, 'git', 'worktree', 'add', '-b', $name, $where);
    return { ok => 0, error => $text || 'git worktree add failed' } if $status;
    return { ok => 1, message => "Work tree $name at " . tilde($where), path => $where };
}

# ---------------------------------------------------------------- forge ---

# GitHub's pull requests and runs for the open project, through `gh`.
my ($forge, %forge_ask) = (undef, want => 0, at => 0, path => '', busy => 0);

sub stamp {
    my ($iso) = @_;
    return 0 unless defined $iso && $iso =~ /^(\d+)-(\d+)-(\d+)T(\d+):(\d+):(\d+)/;
    require Time::Local;
    return eval { Time::Local::timegm($6, $5, $4, $3, $2 - 1, $1) } // 0;
}

sub ask_forge {
    my ($dir, $now) = @_;
    return unless $forge_ask{want} && !$forge_ask{busy};
    return if $forge_ask{path} eq $dir && $now - $forge_ask{at} < $FORGE_EVERY;
    @forge_ask{qw(path at busy)} = ($dir, $now, 1);
    start_job(sub {
        my %result = (path => $dir, prs => [], runs => []);
        my ($status, $text) = run($dir, 'gh', 'pr', 'list', '--limit', '20',
            '--json', 'number,title,headRefName,author,isDraft,updatedAt');
        if ($status) {
            $result{error} = (split /\n/, $text)[0] // 'gh failed';
            return \%result;
        }
        my $prs = eval { JSON::PP->new->decode($text) } // [];
        $result{prs} = [map { {
            number => $_->{number} + 0, title => $_->{title} // '', branch => $_->{headRefName} // '',
            author => ($_->{author} // {})->{login} // '', draft => $_->{isDraft} ? JSON::PP::true : JSON::PP::false,
            updated => stamp($_->{updatedAt}),
        } } @$prs];
        ($status, $text) = run($dir, 'gh', 'run', 'list', '--limit', '15',
            '--json', 'databaseId,displayTitle,workflowName,headBranch,status,conclusion,createdAt');
        my $runs = $status ? [] : (eval { JSON::PP->new->decode($text) } // []);
        $result{runs} = [map { {
            id => $_->{databaseId} + 0, title => $_->{displayTitle} // '', workflow => $_->{workflowName} // '',
            branch => $_->{headBranch} // '', status => $_->{status} // '', conclusion => $_->{conclusion} // '',
            created => stamp($_->{createdAt}),
        } } @$runs];
        return \%result;
    }, sub {
        $forge_ask{busy} = 0;
        $forge = $_[0] if ($_[0]{path} // '') eq $dir;
    }) or ($forge_ask{busy} = 0);
}

# --------------------------------------------------------------- loop -----

my (@projects, $last_scan, $last_out) = ((), 0, '');

sub emit {
    my ($force) = @_;
    my $now = time;
    if ($force || $rescan || $now - $last_scan >= $SCAN_EVERY) {
        $rescan = 0;
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
            branch => $summary->{branch}, scope => $state{scope}, log => commit_log($dir) };
        ask_forge($dir, $now);
        $out{forge} = $forge if $forge && $forge->{path} eq $dir;
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
    $out{action} = $action if $action;
    $out{running} = $running if defined $running;
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
    if (exists $msg->{forge}) {
        $forge_ask{want} = $msg->{forge} ? 1 : 0;
        $forge_ask{at} = 0 if $msg->{refresh};
    }
    my $from = $value->{from};
    my $id = (defined $msg->{id} && !ref $msg->{id}) ? $msg->{id} : undef;
    if (defined $msg->{commit} && !ref $msg->{commit}) {
        my ($dir, $message) = ($state{select}, $msg->{commit});
        if (!defined $dir || $message !~ /\S/) {
            finish('commit', $id, { ok => 0, error => 'Write a message to commit with' });
        }
        else {
            write_action($from, 'commit', $id, sub { commit($dir, $message) });
        }
    }
    for my $what ('pull', 'push') {
        next unless $msg->{$what};
        my $dir = $state{select};
        if (!defined $dir || !-e "$dir/.git") {
            finish($what, $id, { ok => 0, error => 'No work tree open' });
        }
        else {
            write_action($from, $what, $id, sub { sync($dir, $what) });
        }
    }
    if (defined $msg->{worktree} && !ref $msg->{worktree}) {
        my ($name, $repo) = ($msg->{worktree}, $msg->{repo} // '');
        $repo = ref $repo ? '' : $repo =~ s{^~(?=/|$)}{$HOME}r;
        if ($name !~ m{^[A-Za-z0-9][A-Za-z0-9._/-]*$} || $name =~ m{\.\.} || !-e "$repo/.git") {
            finish('worktree', $id, { ok => 0, error => 'A branch name (letters, digits, . _ / -) in a repository' });
        }
        else {
            write_action($from, 'worktree', $id, sub { add_worktree($repo, $name) });
        }
    }
    keep();
}

restore();
my $buffer = '';
emit(1);
while (1) {
    my $changed = 0;
    for my $fh ($select->can_read($LOOK_EVERY)) {
        if (fileno $fh == fileno STDIN) {
            my $got = sysread STDIN, $buffer, 65536, length $buffer;
            exit 0 unless $got;
            while ($buffer =~ s/^([^\n]*)\n//) {
                receive($1);
                $changed = 1;
            }
        }
        elsif (read_job($fh)) {
            $changed = 1;
        }
    }
    emit($changed);
}

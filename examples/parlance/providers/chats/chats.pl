#!/usr/bin/env perl
# Parlance's conversations, kept in a JSON file, and its scripted demo agent.
# Prints one JSON line whenever what the window shows changes; reads the
# app's messages, one `{"from": ..., "message": ...}` per line, on stdin.
# Exits when the app closes stdin.
#
# There is no model here. A message gets one of a few prepared markdown
# answers, picked by its words and streamed a few words at a time, so the
# window shows a chat working end to end. A real agent would be a provider
# like this one that streams a model's tokens instead.
#
# Output (`app.chats`):
#   current    the open chat's id, or null for a new chat
#   chat       { id, title, messages = { { role, text } } } (the open one)
#   streaming  true while an answer is being written
#   pinned     { id, title }: pinned chats
#   folders    { id, title, chats = { id, title } }: pinned folders
#   projects   { id, title, chats = { id, title } }
#   recents    { id, title, status? }
#   dot        the count beside "Your dot"
#
# Messages (one key each):
#   open = id             open a chat
#   new = true            start a new chat (made on the first message)
#   send = text           the user's message; the demo answers it
#   stop = true           stop the answer being written
#   regenerate = true     write the last answer again
#
# The file is $XDG_DATA_HOME/moonframe/<profile>/chats.json (by default under
# ~/.local/share), started from sample.json the first time.

use strict;
use utf8;
use warnings;
use JSON::PP ();
use IO::Select;
use Time::HiRes ();

$| = 1;
binmode STDOUT, ':encoding(UTF-8)';
my $json = JSON::PP->new->canonical->utf8(0);
my $HOME = $ENV{HOME} // '';
my $DATA = (($ENV{XDG_DATA_HOME} // '') =~ m{^/} ? $ENV{XDG_DATA_HOME} : "$HOME/.local/share")
    . '/moonframe/' . (($ENV{MOONFRAME_PROFILE} // 'parlance') =~ s/[^A-Za-z0-9_-]/-/gr);
my $FILE = "$DATA/chats.json";

my $store;

sub read_json {
    my ($path) = @_;
    open my $fh, '<:encoding(UTF-8)', $path or return undef;
    local $/;
    my $value = eval { $json->decode(<$fh>) };
    close $fh;
    return $value;
}

sub load {
    $store = read_json($FILE);
    if (ref $store ne 'HASH' || ref $store->{chats} ne 'HASH') {
        $store = read_json('sample.json') // { chats => {}, pinned => [], folders => [], projects => [], recents => [] };
        $store->{next} = 1;
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

sub entry {
    my ($id) = @_;
    my $chat = $store->{chats}{$id} or return ();
    my %row = (id => $id, title => $chat->{title});
    my $status = ($store->{status} // {})->{$id};
    $row{status} = $status if defined $status;
    return \%row;
}

# The answer being written: the chat, the words still to come, and when the
# next ones are due.
my $stream;

sub out {
    my $current = $store->{current};
    my $chat = defined $current ? $store->{chats}{$current} : undef;
    my $group = sub {
        [ map { +{ id => $_->{id}, title => $_->{title}, chats => [ map { entry($_) } @{ $_->{chats} // [] } ] } } @{ $_[0] // [] } ];
    };
    return {
        available => JSON::PP::true,
        current => $chat ? $current : undef,
        chat => $chat
            ? { id => $current, title => $chat->{title}, messages => $chat->{messages} }
            : { id => undef, title => 'New chat', messages => [] },
        streaming => $stream ? JSON::PP::true : JSON::PP::false,
        pinned => [ map { entry($_) } @{ $store->{pinned} // [] } ],
        folders => $group->($store->{folders}),
        projects => $group->($store->{projects}),
        recents => [ map { entry($_) } @{ $store->{recents} // [] } ],
        dot => $store->{dot} // 0,
    };
}

# The scripted answers, picked by the first pattern the message matches.
my @ANSWERS = (
    [ qr/\b(code|rust|function|bug|error|panic|script|regex)\b/i, <<'MD' ],
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

Here is a small, safe shape for it:

```rust
fn parse(line: &str) -> Result<Entry, ParseError> {
    let (date, rest) = line.split_once(' ').ok_or(ParseError::NoDate)?;
    Ok(Entry { date: date.parse()?, memo: rest.trim().to_string() })
}
```

A few notes:

- Return a `Result` instead of calling `unwrap`, so one bad line does not stop the run.
- Keep parsing apart from printing; it makes **tests** easy to write.
- `split_once` reads better than indexing into the string.
MD
    [ qr/\b(name|call|term|word|title)\b/i, <<'MD' ],
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

A few directions for the name:

| Name | Why it works |
|---|---|
| Short and plain | Easy to say, easy to search |
| A metaphor | Carries the idea without explaining it |
| A made-up word | Free to own, but needs a story |

My pick: start from the **metaphor** and test it out loud with three people.
MD
    [ qr/\b(plan|trip|ideas?|list|schedule|week|steps?)\b/i, <<'MD' ],
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

A simple plan to start from:

1. **Decide what done looks like** — one sentence you could check.
2. **Block the time** — two short sessions beat one long one.
3. **Start with the smallest step** — it carries the rest.
4. **Review on Friday** — keep what worked, drop one thing.

Want me to turn this into a checklist?
MD
    [ qr/./, <<'MD' ],
_Scripted demo: Parlance has no model behind it, so this is a prepared answer._

Good question. Here is how I would think about it:

- **What you know** — the facts you can check today.
- **What you assume** — write these down; they are where surprises hide.
- **What would change your mind** — decide it before you look.

| Approach | Good for |
|---|---|
| Quick estimate | Deciding whether it is worth more time |
| Careful comparison | Choices you cannot easily undo |

The key point: **pick the approach by how hard the decision is to reverse.**
MD
);

sub answer_for {
    my ($text) = @_;
    for (@ANSWERS) {
        my ($pattern, $answer) = @$_;
        next unless $text =~ $pattern;
        $answer =~ s/\n+$//;
        return $answer;
    }
}

sub title_for {
    my ($text) = @_;
    my @words = grep { length } split /\s+/, $text;
    my $title = join ' ', @words[ 0 .. ($#words < 5 ? $#words : 5) ];
    $title =~ s/[?.!,:;]+$//;
    return length $title ? ucfirst $title : 'New chat';
}

sub begin_answer {
    my ($id, $text) = @_;
    my $chat = $store->{chats}{$id};
    push @{ $chat->{messages} }, { role => 'assistant', text => '' };
    # Words with the spaces after them, so the text grows as it was written.
    my @pieces = (answer_for($text) =~ /(\s*\S+\s*)/g);
    # A short pause first, as if it were thinking.
    $stream = { id => $id, pieces => \@pieces, due => Time::HiRes::time() + 0.6 };
}

sub step {
    return unless $stream;
    my $now = Time::HiRes::time();
    return if $now < $stream->{due};
    my $chat = $store->{chats}{ $stream->{id} };
    my $last = $chat ? $chat->{messages}[-1] : undef;
    unless ($last && $last->{role} eq 'assistant') {
        undef $stream;
        return;
    }
    my $count = 1 + int rand 3;
    $last->{text} .= shift @{ $stream->{pieces} } while $count-- > 0 && @{ $stream->{pieces} };
    if (@{ $stream->{pieces} }) {
        $stream->{due} = $now + 0.03 + rand 0.04;
    }
    else {
        $last->{text} =~ s/\s+$//;
        undef $stream;
        save();
    }
}

sub stop {
    return unless $stream;
    my $chat = $store->{chats}{ $stream->{id} };
    if ($chat && @{ $chat->{messages} } && $chat->{messages}[-1]{role} eq 'assistant') {
        my $last = $chat->{messages}[-1];
        $last->{text} =~ s/\s+$//;
        pop @{ $chat->{messages} } unless length $last->{text};
    }
    undef $stream;
    save();
}

sub receive {
    my ($line) = @_;
    my $value = eval { JSON::PP->new->utf8(1)->decode($line) };
    my $msg = ref $value eq 'HASH' ? $value->{message} : undef;
    return unless ref $msg eq 'HASH';
    if (defined $msg->{open} && !ref $msg->{open} && $store->{chats}{ $msg->{open} }) {
        stop() if $stream && $stream->{id} ne $msg->{open};
        $store->{current} = $msg->{open};
    }
    elsif ($msg->{new}) {
        stop();
        $store->{current} = undef;
    }
    elsif ($msg->{stop}) {
        stop();
        return;
    }
    elsif (defined $msg->{send} && !ref $msg->{send} && $msg->{send} =~ /\S/ && !$stream) {
        my $text = substr($msg->{send}, 0, 8000);
        $text =~ s/^\s+|\s+$//g;
        my $id = $store->{current};
        unless (defined $id && $store->{chats}{$id}) {
            $id = 'n' . ($store->{next}++ // 1);
            $store->{chats}{$id} = { title => title_for($text), messages => [] };
            unshift @{ $store->{recents} }, $id;
            $store->{current} = $id;
        }
        push @{ $store->{chats}{$id}{messages} }, { role => 'user', text => $text };
        begin_answer($id, $text);
    }
    elsif ($msg->{regenerate} && !$stream) {
        my $id = $store->{current};
        my $chat = defined $id ? $store->{chats}{$id} : undef;
        return unless $chat && @{ $chat->{messages} } >= 2 && $chat->{messages}[-1]{role} eq 'assistant';
        pop @{ $chat->{messages} };
        begin_answer($id, $chat->{messages}[-1]{text});
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
    step();
    my $text = $json->encode(out());
    if ($text ne $last) {
        print $text, "\n";
        $last = $text;
    }
    my $wait = $stream ? 0.02 : 60;
    if ($select->can_read($wait)) {
        my $got = sysread STDIN, $buffer, 65536, length $buffer;
        exit 0 unless $got;
        receive($1) while $buffer =~ s/^([^\n]*)\n//;
    }
}

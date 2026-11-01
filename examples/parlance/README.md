# parlance

**Parlance**, a [Moonframe](../..) app: a dark desktop chat app in the
manner of the big assistants' own desktop clients.

- **Title bar:** history arrows, the sidebar toggle, the open chat's title,
  Share (copies the title), more, and a new-chat button.
- **Rail:** home, library, history, explore, more and apps; a download offer
  and the signed-in person at the foot.
- **Sidebar:** New chat, "Your dot" with its count, Pinned chats and
  folders, Projects (folders that open to their chats, five at a time with
  Show more) and Recents, the open chat highlighted, one waiting on you.
  Click a chat to open it; click a folder or project to open or close it.
- **Conversation:** the user's messages as bubbles with copy, share and
  edit; the answers as rich markdown (bold, code, tables, lists, code
  blocks, source chips) drawn by the `chat-markdown` component, with copy,
  share, read aloud, regenerate, feedback and more.
- **Composer:** +, the field, a mode menu (Instant, Thinking, Research),
  the mic, and send, which becomes stop while an answer is written.

The agent is a **scripted demo**, not a model: `providers/chats` (a Perl
process provider) answers with one of a few prepared markdown answers,
picked by the message's words, and streams it a few words at a time. A real
agent would be a provider like it that streams a model's tokens. The chats
live in `~/.local/share/moonframe/parlance/chats.json`, started from
`providers/chats/sample.json`; new chats go to the top of Recents.

```sh
moonframe app link examples/parlance   # from the repository
moonframe --profile parlance
just bundle-app parlance               # target/bundle/Parlance.app
```

Known limits: Luau cannot scroll a column, so a long chat does not follow a
streaming answer to the bottom; the host has no italic face (italic text
shows in the muted colour); a few icons stand in for ones the bundle lacks
(a pencil for "new chat", a square bubble for the round one, a play circle
for read aloud).

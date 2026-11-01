# msgvault

A [Moonframe](../..) app for browsing a [msgvault](https://github.com/wesm/msgvault)
archive, after [msgvault-gpui](https://github.com/penso/msgvault-gpui): every
account, label, thread and chat msgvault keeps, read-only.

- **Title bar:** the search field (msgvault's Gmail-like syntax: free text,
  `from:`, `subject:`, `label:`, `has:attachment`, `is:unread`, `is:starred`,
  `before:` and `after:`), where the archive is read from, refresh, and the
  light/dark switch.
- **Sidebar:** the account in view, the standard mailboxes (Inbox with its
  unread count, Starred, Unread, Important, Sent, Drafts, All Mail,
  Attachments, Spam, Trash), coloured user labels, Gmail's categories, the
  other sources (WhatsApp, Slack, the calendar, messages removed upstream),
  the accounts (a click shows one alone), and the archive's size.
- **List:** one row per conversation, its newest message: unread dot and
  date in blue, avatar, sender, subject with the thread's size, attachment
  and star, the snippet and up to two label chips. Chats lead with the
  conversation's name.
- **Reader:** an email thread reads like Gmail's: earlier messages fold to
  one line (click one to open it; the envelope button or ⌘; opens all),
  the newest is a card with sender, recipients, body and attachments. A
  chat is bubbles, yours on the right.
- **HTML mail** shows rendered, in a web view, with a toggle (the `</>`
  button, ⌘⇧H) to plain text. The provider writes each message's page to
  `~/.cache/moonframe/msgvault/html`, in the window's light or dark colours,
  under a Content-Security-Policy that runs no script and loads nothing from
  the network. Remote pictures stay blocked until "Load remote images" for
  that message. A followed link offers "Open in browser" and the way back.
  Messages without an HTML part show their text.
- **Attachments** are cards in the reader (a thumbnail for pictures, an icon
  by type otherwise) with **Open** (in the system's app for it, from a
  temporary copy) and **Save** (to `~/Downloads`, then shown in Finder), only
  ever on a click. **All Attachments** in the sidebar lists every file in the
  archive, with filters by kind and a search; picking one shows it with its
  message ("Show the message" goes there).

**Where the data comes from** (`providers/vault`, a Perl process provider):
when `$MSGVAULT_HOME/msgvault.db` (else `~/.msgvault/msgvault.db`) exists and
`sqlite3` is installed, it reads the newest 2,000 messages from it, never
writing (`PRAGMA query_only`), and again every minute or on ⌘R. Otherwise it
shows `sample.json`, a snapshot of msgvault-gpui's demo archive with its
times kept relative to now, and the title bar says "Sample". Attachment bytes
are read the way msgvault stores them: a sealed pack found through
`attachment_pack_index` (a zstd frame when flagged, which needs the `zstd`
command), else a loose file at `attachments/<sha[:2]>/<sha>`, checked
against its SHA-256; remote ones open in the browser. The sample's files
are made the way msgvault-gpui's fixture makes them (a one-page PDF, an
event, a gradient picture), and its promotion mails carry one remote picture
to show the blocking. The view (mailbox, account, search, open message) is kept in
`~/.cache/moonframe/msgvault`.

To make `sample.json` again: `msgvault-fixture DIR` (in msgvault-gpui), then
`MSGVAULT_HOME=DIR perl vault.pl --dump > sample.json` in `providers/vault`
(the one here is then made a day older, so its rows read "Yesterday").

```sh
moonframe app link examples/msgvault   # from the repository
moonframe --profile msgvault
just bundle-app msgvault               # target/bundle/msgvault.app
```

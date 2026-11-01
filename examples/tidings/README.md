# tidings

**Tidings**, a [Moonframe](../..) mail client with two layouts of the same
mailbox:

- **A, the list beside the email** (`⌘1`): the emails as cards (unread dot,
  sender and thread size, domain, time, subject, two lines of preview, a
  category and a suggested next step), and the open email on the right.
  With none open, it says how many threads and unread there are, and which
  keys to use.
- **B, one line per email** (`⌘2`): every email on one line across the
  window, grouped by Today, Yesterday and This week; an email opens in its
  place, with a back arrow.

Switch with **L**, the icon beside compose, or the menu under the "Mail"
title. The tabs over the list (Needs you, VIP, Sales, Investors, FYI, All)
filter it, with their unread counts. The title row shows the emails you
opened as tabs, beside sample tabs of the other places.

| Key | Does |
| --- | --- |
| J / K | next / previous email (opens it when one is open) |
| Enter | open the selected email |
| U | back to the list |
| E | mark done (it leaves the list; "Bring back done" in the title menu) |
| ⇧U | mark unread |
| C | new message |
| / | search |
| L | switch layout |

Single keys work while no text field has focus (they are shell menu
shortcuts that type text, which Moonframe offers only then).

The mailbox is **sample data** (`providers/mailbox/sample.json`, published
as it is by a static provider as `app.mailbox`). What you read, mark done
or reply is kept in the mail mod's state, so it comes back next launch.

```sh
moonframe app link examples/tidings   # from the repository
moonframe --profile tidings
just bundle-app tidings               # target/bundle/Tidings.app
```

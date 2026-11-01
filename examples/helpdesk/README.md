# helpdesk

**Tandem**, a [Moonframe](../..) app for an IT help desk inbox, where an
assistant (also called Tandem) answers employees in chat, checks their
devices and opens tickets for the IT team:

- **Left:** the brand, the workspace ("IT Help at Corvo"), a search field,
  the inbox's views with their counts (All tickets, All open, Assigned to me,
  Waiting on employee), the other sections (not in this example), and who is
  signed in. The view and the search filter the list.
- **Middle:** the view's tickets, newest first (or oldest, or by priority),
  under the week in a sentence and a bar split between the assistant, the IT
  team and what is still open. Each row shows its priority as the left edge
  (red high, amber medium, lavender low), its ID and age, the requester, the
  kind and where it came from, and its status, or a green pill when the
  assistant resolved it. The `+` opens a blank ticket.
- **Right:** the picked ticket: chips, watchers, the requester and device,
  the assistant's summary, the SLA's two clocks (red when close), the
  assistant's check on the device with its suggestion, the chat thread it
  came from, and a reply box with Reply and Internal note.

**Interactive:** a check still running finishes a few seconds after the
ticket is on screen; the suggestion's button runs it (a new step spins, then
the assistant posts in the thread); replies and internal notes join the
thread; Assign toggles; "Show 1 earlier message" unfolds the thread. Ticket >
Next and Previous (⌘↓, ⌘↑) step through the list.

Everything is **sample data** (`providers/desk/sample.json`), kept in memory
by a Perl process provider (`providers/desk`), so each launch starts over.
Point the provider at a real help desk's API to make it live.

```sh
moonframe app link examples/helpdesk   # from the repository
moonframe --profile helpdesk
just bundle-app helpdesk               # target/bundle/Tandem.app
```

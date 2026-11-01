# pipeline

**Tally**, a [Moonframe](../..) app for a sales pipeline:

- **Left:** the workspace, its sections (Pipeline and Meetings are in this
  example), the quarter's quota, and who is signed in.
- **Middle:** the quarter's title and controls, a strip of figures (quota,
  weighted pipeline, win rate, cycle, revenue by month), then the deals in
  columns by stage, or as a list (Board / List). Typing in the search field
  filters them.
- **Right:** an agent's panel. It is a **scripted demo**, not a model: it
  reads the deals and answers a few kinds of question by rules (what to do
  first, what is at risk, what will close). A real agent would plug in as a
  provider that publishes its replies.

**Meetings** (in the sidebar, or View > Meetings, ⇧⌘M): a header with
Create, a toolbar (Show Filters for chips by call kind, whose meetings,
list or calendar, search with ⌘K, Import, Schedule), then the calendar
beside a panel. The calendar is the `week-calendar` component: Day, Week or
Month, Today and arrows (⌘T, ⌘[, ⌘]), events as cards across their hours,
overlapping ones side by side, a red line at the time now. Clicking an
event shows it in the panel under the next upcoming meeting: host, what it
relates to, when it was made, participants, notes. Reschedule moves it a
day on; Join Meeting copies its link; Create and Schedule add an hour at
9 AM on the day in view. The meetings live in
`~/.local/share/moonframe/pipeline/meetings.json` (`providers/meetings`, a
Perl process provider that also says what today is), started with sample
meetings around the current week.

**Moving deals:** Luau views cannot take drag and drop yet, so click or
right-click a card for its menu: move it to another stage, mark it won, or
flag it at risk. "Add deal" and "New deal" add one to a column.

The deals live in `~/.local/share/moonframe/pipeline/deals.json`
(`providers/deals`, a Perl process provider), started with sample deals.

```sh
moonframe app link examples/pipeline   # from the repository
moonframe --profile pipeline
just bundle-app pipeline               # target/bundle/Tally.app
```

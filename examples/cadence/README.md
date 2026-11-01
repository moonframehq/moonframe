# cadence

**Cadence**, a [Moonframe](../..) app: a light dashboard for a workflow
automation tool, with a notification drawer behind the bell.

- **Left:** the workspace's pages (only Dashboard is in this example), recent
  workflow runs you can star, the month's usage with Upgrade, and who is
  signed in. Appearance switches light and dark.
- **Top:** where you are, a search field, Ask AI, and the bell (a red dot
  while something is unread; dark while the drawer is open).
- **Main:** a welcome with the date and **Create Workflow**, four figures,
  the active workflows (pause and resume each one), and a carousel of
  templates (click one to add it).
- **The drawer** (`notification-drawer`, in `app/components`): slides in
  from the right under the top bar over the dimmed dashboard. Its tabs (All,
  Error, Success, Info) filter the list, with a pill that slides to the
  chosen one and the list fading over; **Mark all read** clears the unread
  dots, a click on a card marks it read, × on a card dismisses it, and Esc,
  × or a click outside closes the drawer. Creating a workflow or adding a
  template posts a new notification.

Everything is **sample data** kept in the mods' state (so it survives a
restart). **View › Show or Hide Notifications** (⇧⌘B) opens the drawer from
the menu.

```sh
moonframe app link examples/cadence   # from the repository
moonframe --profile cadence
just bundle-app cadence               # target/bundle/Cadence.app
```

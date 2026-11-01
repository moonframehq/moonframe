# storefront

**Storefront**, a [Moonframe](../..) app: a store's admin opened at its
audit log, to show off the data-table components.

- **Title bar:** the sidebar toggle, a search button (it focuses the log's
  search field), back and forward through the pages visited, and the open
  pages as browser-style tabs (`tabs` with `variant = "browser"`): click
  to switch, × to close, + for a new one.
- **Sidebar:** the admin's pages in groups; a click shows that page in the
  tab in front, or switches to a tab already showing it. Only the audit
  log is built; the other pages are stubs.
- **Audit log:** filter chips (`filter-dropdown`) for the kind of activity
  (several at once), the author and the period; a search over the
  activity and its author; a bordered `table` with a box per row (ticking
  the header's box ticks the page), activities as `rich-text` (bold
  subject, muted verb, outlined tag) and authors with an `avatar`; click
  Timestamp to flip the order. Under it, `pagination` with an
  items-per-page menu. **Export as CSV** writes the ticked rows, or every
  row the filters show, to `~/Downloads/storefront-audit-log.csv`.

The activities are **sample data**: `providers/store` (a Perl process
provider) makes 232 of them from templates with a fixed seed, ending on
Oct 30, 2026, and writes the CSV the page exports.

```sh
moonframe app link examples/storefront   # from the repository
moonframe --profile storefront
just bundle-app storefront               # target/bundle/Storefront.app
```

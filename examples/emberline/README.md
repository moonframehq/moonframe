# emberline

**Emberline**, a [Moonframe](../..) app for a CRM's leads report, in the dark:

- **Left:** the workspace switcher beside the traffic lights, a search field
  that filters the sidebar (Esc clears it), the pages (only the Leads Report
  is drawn in this example), favourite companies and people tagged by what
  they are, and saved searches with coloured marks. The panel button hides
  the sidebar; the header offers it back.
- **Right:** the page's name and lead count, then one card of pipeline
  figures, sales revenue by month with growth chips and one month in the
  spotlight (click another month to move it there; the pointer brightens
  the one under it), leads by status on gradient bars beside web visits,
  and the hottest deals beside where leads come from.

It uses two components made for it, both in `app/components`:
`spotlight-bars` (tall rounded columns, one painted with a gradient) and
`level-row` (a label, a gradient fill on a track, a count). Both rest on the
stylesheet's `gradient` property and the `display` text size.

The figures are **sample data**: `providers/leads` (a Perl process provider)
publishes `sample.json` as `app.leads`. Point it at your CRM to make it real.

Numbers are set in Geist Mono (`GeistMono Nerd Font Mono`); without it they
fall back to another monospace face.

```sh
moonframe app link examples/emberline   # from the repository
moonframe --profile emberline
just bundle-app emberline               # target/bundle/Emberline.app
```

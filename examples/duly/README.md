# Duly

Compliance notices for a public buyer, as a Moonframe app. Duly shows what
is due, exceptions against the forecast, the contract register, a payments
notice getting ready to sign, and reports for the audit committee. All the
data is sample data from `providers/duly/sample.json`.

```sh
moonframe app link examples/duly
moonframe --profile duly
```

## Pages

- **Today**: five headline figures, the AI exceptions forecast (click a
  day to read its figures in the card beside it), an AI insight, what is
  due this week (searchable), open exceptions, and the deadlines table
  (click a heading to sort, Open goes to the notice's page, Notice
  calendar shows due dates).
- **Contracts**: the register with filter pills, a search over contracts
  and suppliers, and Open for a contract's details in a side sheet. Add
  contract opens a dialog.
- **Payments**: the Apr to Sep notice: how fast invoices were paid, each
  figure and how it was reached, on time each month, what is ready to send,
  the scope, and a preview of the notice.
- **Reports**: on time by notice type, payment speed by month, gaps in the
  public record, and the export pack, whose files you tick.

The sidebar's search filters the pages and saved views; the folders open;
the reminder card can be dismissed. ⌘1 to ⌘4 go to the four pages, ⌘S
hides the sidebar.

## Pieces

From the catalogue: `kpi-meter`, `line-chart`, `insight-note` (added for
this app), `dashboard-card`, `share-bar`, `avatar`, `menu`, `month-grid`,
`sheet`, `dialog`, `key-values`, `checkbox`, `kbd`, `empty-state`.

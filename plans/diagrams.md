# Diagrams

Diagrams in Moonframe that people draw and agents generate: a freeform
editor, Mermaid and Graphviz fences drawn in any markdown (chat answers
first), and live diagrams an agent builds and updates while it works.
All of it is built in Moonframe; Sheprd gets it when it moves onto
Moonframe's chats.

## Goals

- **Beautiful by default.** Diagrams are drawn with the app's own style
  (colors, fonts, corners, light and dark), not as a pasted Mermaid image.
- **Agents describe, Moonframe lays out.** An agent sends nodes and edges,
  never coordinates. Layout is stable: adding a node does not move the rest.
- **Live.** A diagram grows as an answer streams in or as an agent patches
  it; new nodes grow in, a highlighted path carries tokens.
- **Fast.** 1,000 shapes drag and zoom smoothly; layout runs off the UI
  thread.
- **One view everywhere.** Markdown fences, `ui.diagram` in Luau and the live
  provider all draw through the same Rust view.

## Architecture

```
mermaid / dot text ─┐
agent graph (MCP) ──┼─> moonframe-diagram: model -> layout -> view (GPUI)
.moonframe/diagrams ┘        ^                         │
Luau document (ui.diagram) ──┘           edits as messages back to Luau
```

- **`crates/moonframe-diagram`**: the model (nodes, edges, groups, kinds,
  styles), the parsers, the layout and the GPUI view. It depends only on
  gpui and gpui-kit, never on the `moonframe` crate, so any gpui-kit app can
  embed it. Parsing and layout are plain data with no gpui, tested
  headless. Split into modules by responsibility (`model`, `parse/mermaid/*`,
  `parse/dot`, `layout/*`, `view/*`).
- **`ui.diagram`**: a host element in `crates/moonframe` behind a `diagram`
  cargo feature (as `ui.map` is behind `map`), on in the starter.
- **`diagrams` native provider**: its own crate. Takes diagrams from agents
  and files and shares them as `app.diagrams`.

## Phases

### 0. Luau editor (done)

`app/components/diagram.luau` and `examples/diagram`: shapes, drag, resize,
connectors, selection, pan and zoom, undo, kept across launches. It is the
behaviour spec for the Rust view: every interaction it has, the view keeps.

The spec, as built:

- **Document is the parent's.** Props `shapes` (`id kind x y w h label?
  fill?`, world pixels, bottom first), `links` (`id from to label?
  dashed?`), `selected`, `tool`. The canvas keeps only its view (pan, zoom)
  and the gesture under way, and sends one message per gesture:
  `selected`, `moved` (list of `id x y`), `resized`, `created` (`kind x y
  w h`), `connected` (`from to`), `edited` (`id label`), `deleted` (ids),
  `duplicate` (ids), `tool`, `zoom`. The parent sends it `zoom_by`,
  `zoom_to`, `fit`.
- **Gestures.** Press a shape: select it (shift toggles), drag moves the
  selection. Press empty space: marquee (shift adds). Corner handles
  resize, side ports drag out a connector dropped on another shape. A
  shape tool clicks (default size) or drags (that box) a new shape.
  Double-click edits the label in place. Hand tool or middle button pans;
  scroll pans, pinch or ⌘-scroll zooms about the pointer (10 % to 400 %).
  Keys: delete, escape, ⌘A, ⌘D, arrows nudge (shift ×10), ⌘0, ⌘=, ⌘-.
  Single-letter tool keys (V H R D O T S C) live in the app's menu: a
  row's `keys` never take what types text.
- **Links** route as elbows out of the facing sides; boxes apart on both
  axes get one bend, into the other's top or bottom. Labels sit on the
  middle; a label (or a dot, without one) selects the link.

Measured (release build, M-series Mac, `RUST_LOG=moonframe::perf=trace`, a
60 Hz driver sending what a drag sends to `editor/diagram#main`; medians):

| Shapes on screen | Drag: views per step | Drag: frame build | Pan: views | Pan: frame |
| --- | --- | --- | --- | --- |
| 100 (fit) | 3.3 ms | 1.7 ms | 2.8 ms | 1.3 ms |
| 500 (fit, all visible) | 11.6 ms | 4.8 ms | 9.1 ms | 3.1 ms |
| 500 at 100 % (about 60 visible, the rest culled) | 3.6 ms | 0.8 ms | 4.2 ms | 0.8 ms |
| about 1,000 | no longer works | | | |

What it shows:

- Hundreds of shapes on screen at once miss 60 Hz: 500 cost about 16 ms
  a step (Luau builds every visible node again for each pointer move).
- **A hard ceiling near 700 shapes.** State and props may hold 10,000
  values; at about 1,000 shapes the document is too big to be saved and
  to reach the canvas, which then draws nothing. The canvas instance's
  Luau memory reached 4.1 of its 8 MiB at 500.
- Text cannot scale with zoom (sizes are tokens), so labels hide below
  45 %; corners and strokes don't scale either. Diamonds need a rotated
  shape Luau can't draw: `pill` stands in.
- The Rust view must take the document once and edits as messages, not
  the whole document as props each frame.

Two host fixes came out of measuring: `RUST_LOG` printed nothing for an
app without log layers of its own (an empty layer list turned every event
off), and `measure` was not sent again in a headless window that reused an
id (so a second render drew an empty canvas).

### 1. The model, layout and view (done, with phase 2)

`crates/moonframe-diagram`: `model`, `geometry`, `layout` (`rank`,
`order`, `place`, `layered` and its `route`, `placed`, `sequence`,
`groups`) and, behind its `view` feature, `view` (`camera`, `scene`,
`hit`, `input`, `snap`, `paint`, `style`).

As built:

- Ranks by longest path after breaking cycles with a depth-first walk;
  sources pulled down beside what they point at. Long edges get a
  stand-in per rank. Order by barycenter sweeps (12) with neighbour swaps
  in ranks of up to 48, a Fenwick-tree crossing count, and group members
  kept side by side. Coordinates by isotonic regression (pool adjacent
  violators) toward the neighbours' weighted middle, stand-ins pulling
  hardest so long edges run straight. Edges leave a node's facing side at
  ports spread in the order of where they go, and bends in one gap take
  tracks of their own.
- Incremental: a node keeps at least its previous rank, and ranks start
  in the previous order, which a sweep only replaces with fewer crossings.
- Placed diagrams: routes chosen among elbows and L shapes by what they
  cross (a grid index of the nodes), length, bends and whether they enter
  the target's facing side; detours only around what blocks the plain
  shapes. A node with no place goes beside a placed neighbour.
- The view lays out up to 150 nodes at once and bigger diagrams on a
  background thread, easing nodes to their new places (240 ms) and fading
  edges in. Drags reroute only the edges touching what moves. Labels wrap
  at words to the box; text under 4 px is not drawn.
- Not yet: edges avoiding each other (labels of two edges in one gap can
  touch), a jog where one edge meets a node whose side spreads several
  ports, label editing for edges, `icon`.

Measured (release, M-series Mac):

| Layout (`cargo run --release -p moonframe-diagram --example layout-bench`) | 100 nodes | 500 | 1,000 |
| --- | --- | --- | --- |
| layered (about 1.4 edges a node) | 0.6 ms | 3.8 ms | 9.1 ms |
| placed, routed | 1.9 ms | 21 ms | 58 ms |
| one node added to 500, from the last layout | | 3.8 ms | |

| A frame while dragging (`diagram::bench`, headless, CPU) | idle | one node | 100 nodes |
| --- | --- | --- | --- |
| 1,000 nodes, all in view (21 %, labels too small to draw) | 3.3 ms | 3.7 ms | 4.0 ms |
| 1,000 nodes at 60 % (labels drawn) | 1.5 ms | 1.3 ms | 1.4 ms |
| 500 nodes, all in view (33 %) | 2.4 ms | 2.5 ms | 2.6 ms |

Both targets are met: 500 nodes lay out in 3.8 ms (target 30 ms), and a
drag frame with 1,000 shapes costs under 4 ms, where the Luau canvas
spent about 16 ms on 500.

The plan as first written:

- Model: node `id`, `label`, `sublabel`, `kind` (box, rounded, pill,
  diamond, circle, cylinder, note, text), `icon`, `tone`, `group`; edge
  `from`, `to`, `label`, `kind` (arrow, line, dashed, thick), `ends`;
  groups (nested); optional fixed positions (the editor's), else laid out.
- Layout: layered (Sugiyama-style: ranks, crossing reduction, coordinate
  assignment) for flowcharts, top-down or left-right; lanes for sequence
  diagrams; orthogonal edge routing around nodes with rounded corners.
  Incremental: a new layout keeps existing nodes' ranks and order where it
  can. Runs on a background executor; the view animates to the new spots.
- View: hit-testing, selection (click, shift-click, marquee), drag, resize
  handles, connecting from a node's edge, pan (drag on empty space, scroll),
  zoom (pinch, cmd-scroll, fit), snapping guides, keyboard (delete, arrows,
  cmd-A, cmd-Z handled by Luau). Read-only mode for agent diagrams.
- Benchmarks in the crate (layout of 100 / 500 / 1,000 nodes; frame time
  while dragging). Targets: layout of 500 nodes under 30 ms, drag at the
  display's frame rate with 1,000 shapes.

### 2. `ui.diagram` (done)

As built: a hosted view registered by Moonframe itself behind the
`diagram` feature (on in the starter; `app.capabilities.diagram`), so
`ui.diagram { ... }` is `ui.view { name = "diagram", props = ... }` with
the props spelled out. `crates/moonframe/src/diagram.rs` reads props
(`read.rs`) and turns the view's events into messages (`tell.rs`).

- Props: `shapes`/`links` (the Luau canvas's document, as it is) or
  `document`; `mode`, `tool`, `selected`, `highlight`, `camera` (`fit`,
  `zoom`, `zoom_by`, acted on per `seq`), `scroll = "focus"`.
- Messages keep the Luau canvas's names (`selected`, `moved`, `resized`,
  `created`, `connected`, `deleted`, `edited`, plus `opened`, `zoom`,
  `tool`) rather than the ones first planned, so `examples/diagram` uses
  either canvas with the same `update`.
- `examples/diagram` uses `ui.diagram` when the build has it, else the
  Luau canvas; diamonds draw as pills there.
- `flows` (tokens travelling paths) and `layout` as a prop are left for
  later; `direction` sits in the document.
- Rune draws its "not here" placeholder for any `ui.view`.
  `docs/rune-coverage.md` is generated from components, so it is noted
  here rather than there.
- The 10,000-value limit stays: a document is props, so about 1,000
  shapes is the most a mod can hand over (as for any mod state).
  Diagrams bigger than that come from a provider (phase 5) or from text
  (Mermaid, phase 3), which is one value however many nodes it names.

The plan as first written:

- Props: `document` (model as a table), `mode` (`view`, `edit`),
  `selected`, `highlight` (node and edge ids), `flows` (paths with tokens,
  as `flow-diagram` has), `fit`, `layout` (`down`, `right`), `to`.
- Messages back to the mod: `select`, `move`, `resize`, `connect`, `delete`,
  `edit_label`, `open`, each with ids and values, so Luau owns the document
  and undo. A view never changes its document on its own.
- The Luau editor of phase 0 switches its canvas to `ui.diagram` when the
  feature is built, and keeps its Luau canvas otherwise.
- Catalogue entry in `crates/moonframe-core/src/catalogue/elements.md`,
  types in `types/moonframe.d.luau`, `just components`. Rune draws a
  placeholder with the title; note it in `docs/rune-coverage.md`.

### 3. Mermaid and Graphviz (done)

As built: `crates/moonframe-diagram/src/parse` (`mermaid/{flowchart,
sequence,state,class,er}.rs`, `dot.rs`) and `src/write.rs`, hand-written
(no dependency): front matter, `%%{init}%%` and comments passed over;
labels lose `<br>`, tags, markdown strings and `#quot;` entities.

- Flowchart: every bracket shape, `-->` `---` `-.->` `==>` `--o` `--x`
  `<-->` `~~~`, `|labels|` and `-- inline -->` labels, `&`, chains,
  nested subgraphs, `classDef`/`class`/`:::`/`style` as fills (a class
  named `danger`, `success`... gives that tone), Mermaid 11's `@{ label }`.
- Sequence: participants and actors with aliases, every arrow, `+`/`-`
  activations, notes; `loop`/`alt`/`opt`/`par`/`rect`/`critical`/`box`
  keep their messages but draw no frame yet.
- State (composites as groups, `[*]` as dots, transitions into a
  composite to its start), class (members under the name, every
  relation's heads), ER (attributes; "many" as an open head).
- DOT: `graph`/`digraph`, attribute lists, scoped `node`/`edge` defaults,
  chains, subgraphs as edge ends, `cluster_*` as groups; `rankdir`,
  `shape`, `style`, `fillcolor`, `dir`, `arrowhead`/`arrowtail`.
- `ui.diagram { mermaid = "..." }` or `{ dot = "..." }` draws text; what
  does not parse shows its reason in the view's corner.
- Fixtures in `crates/moonframe-diagram/tests/fixtures` (my own, modelled
  on what agents write) with readable snapshots, each also laid out (no
  overlaps, every edge routed), and round trips through the writer.

What agents write (the fenced diagrams in this Mac's Claude transcripts,
2026-11): about 100 flowcharts and 50 sequence diagrams, nothing else; no
DOT. Two thirds of flowcharts label edges with `|...|`, 43 % use `{}`
decisions, a third quote labels; most sequence diagrams alias
participants. `cargo run --release -p moonframe-diagram --example
parse-corpus -- DIR` over the 129 intact ones: 127 parse (98 %); the
other two are not valid Mermaid (`D>S:`, likely `<<->>` lost on the way).
The transcripts stay out of the repository.

Layout fixes found on these: labels sit on the leg into their target
(their own column, so labels of edges bending in one gap no longer
meet), and a long edge's stand-ins a few pixels off its line are put on
it, so it runs straight.

Not yet: frames for sequence blocks, an edge going back up the ranks
still meets its target where the forward edges leave it.

The plan as first written:

- Our own Mermaid parser (no Node, no mermaid.js), into the model:
  `flowchart`/`graph` (all node shapes, edge kinds, labels, subgraphs,
  `classDef`/`class`/`style` mapped onto tones), `sequenceDiagram`,
  `stateDiagram-v2`, `classDiagram`, `erDiagram`. Others (gantt, pie,
  mindmap, timeline, git graph) later, by what agents write most.
- Graphviz DOT: check whether `layout-rs` parses enough; else a small DOT
  parser of our own. Layout is always ours, so both look alike.
- Unsupported syntax: the fence shows as a normal code block, as math does,
  with a small note why.
- Writer: model to Mermaid, for "copy as Mermaid" and round trips.
- Fixtures: a folder of real agent-written diagrams (collect from
  transcripts) with snapshot tests of parse and layout.

### 4. Fences in markdown (done)

As built: `crates/moonframe/src/render/markdown/diagram.rs`, a block
parser and renderer as `math.rs` is, behind `diagram`, so every
`ui.markdown` (`chat-markdown`, `agent-chat`, docs, notes) draws
` ```mermaid `, ` ```dot `, ` ```graphviz ` fences.

- Drawn with moonframe-diagram's new `view::picture`: stateless, fitted
  to the width (never past 100 %), at most 480 px tall, labels measured
  in the theme's font. No scroll is taken, so a chat scrolls past it.
- Each fence's text is laid out once and cached (256 kept). Streaming:
  a version that parses is laid out from the longest earlier one it grew
  from (nodes keep their places); one that doesn't yet shows that earlier
  one, so it never flickers back to source.
- "Copy as Mermaid" (the writer's text, also for DOT) and "Copy source"
  in the corner.
- What doesn't parse, or has over 300 nodes (laid out as the document
  draws, so a bigger one would hold the window up), shows as code with
  why.
- Not yet: pan and zoom after a click, and "Open" in a pane with "Edit a
  copy". A picture has no state, and a block renderer can't keep a view
  entity safely (gpui keyed state only works while laying out or
  painting); this comes with the chat cards of phase 6, which own their
  views.

The plan as first written:

- A markdown plugin, as `crates/moonframe/src/render/markdown/math.rs` is,
  behind the `diagram` feature: ` ```mermaid ` and ` ```dot ` (and
  ` ```graphviz `) draw as diagrams in every `ui.markdown`, so
  `chat-markdown`, `agent-chat`, docs and notes get them at once.
- Streaming: while the fence is open, keep showing the last version that
  parsed and lay out again as it grows; never flicker to source and back.
- In a chat: full width, a height cap, no scroll hijack (pan and zoom only
  after a click inside); a corner with "Source", "Copy as Mermaid", "Open".
  Open shows it in a pane in `ui.diagram`'s view mode, with "Edit a copy".

### 5. Live diagrams from agents

- The `diagrams` provider takes diagrams two ways:
  - **MCP tools**, from a stdio MCP server Moonframe runs
    (`moonframe mcp diagrams`), passed to the agents it starts:
    `crates/moonframe-acp/src/run.rs` sends `mcpServers: []` today.
    Tools: `diagram_show { id, title, mermaid | graph }`,
    `diagram_patch { id, ops }` (add or remove nodes, edges, groups; set
    labels and tones), `diagram_highlight { id, nodes, edges, note }`,
    `diagram_close { id }`. Short tool descriptions so agents use them
    unprompted when relations matter.
  - **A watched folder**, `.moonframe/diagrams/*.{mmd,dot,json}` in the
    project, for agents Moonframe did not start (a terminal Claude, Codex).
- Shares `app.diagrams`: the open diagrams, each with its source agent,
  revision and highlight, so mods can show them in a pane or a side panel.
- Edits the user makes to an agent's diagram stay local unless they press
  "Send to agent", which sends the new Mermaid as a prompt.

### 6. Chats

- In `agent-chat` and `tool-group`, a `diagram_show` or `diagram_patch`
  tool call draws as the live diagram instead of a folded row; later
  patches update that one card in place, so a turn shows one diagram
  growing, not ten.
- This is what Sheprd uses once it moves onto Moonframe's chats, with no
  diagram code of its own.

### 7. Finish

- Export PNG and SVG; copy as Mermaid; light and dark checked against
  style.luau.
- Screenshots in each PR (`just screenshot`, `just component-shot diagram`).

## Rules

CLAUDE.md applies: 1,000-line files, new features in their own modules,
keep-sorted markers, no hard-coded counts, narrow APIs, no AI attribution.
One PR per phase where it can stand alone (1 and 2 may go together).

## Open questions

- Which Mermaid types after flowchart and sequence? Agents here wrote no
  others (2026-11 survey, above), so state, class and ER stay basic and
  gantt, pie, mindmap and the rest wait for a use.
- Is the watched folder per project (`.moonframe/diagrams`) or global?
- Should the user's edits to an agent's diagram be sent back automatically?
- Should `ui.diagram` name a provider's diagram (`source =
  "diagrams:<id>"`) so documents past the 10,000-value limit never cross
  into Luau? Decide with phase 5, when the provider exists.

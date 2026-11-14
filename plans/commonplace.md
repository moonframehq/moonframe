# Commonplace: a second brain on Moonframe

Status: plan, 2026-11-10. Nothing is built yet.

Commonplace is a personal knowledge store. Your own notes, the sources you
collect and a wiki that an LLM maintains all live as plain markdown files in
a vault that Obsidian can open. Fast local search (BM25 now, qmd-style hybrid
search later) covers all of it, and the agent's work is reviewed and can be
undone.

The foundations come before features. Every decision under "Decided" below
has been confirmed.

---

## 1. Research findings

### 1.1 Karpathy: "The append-and-review note" (blog, 2025)

- Everything goes into **one** Apple Notes note. New items go at the top, as
  plain text: no dates, links or tags.
- The only convention is a prefix (`read:`, `watch:`, `listen:`), so a search
  finds "something to read" when that is what you want.
- **Review:** now and then, scroll down and skim. Copy what still matters
  back to the top, and merge or rework related items. Deleting is rare;
  what you keep ignoring sinks.
- **Why one note:** folders and nested notes cost too much attention. You
  never have to decide where something goes before writing it down, and
  Ctrl+F finds anything. "One text note ftw." Old items "are never lost,
  they just don't deserve the top of mind."
- **For us:** capture must be instant and free of structure (`inbox.md`, a
  global hotkey that adds at the top). Reviewing is something you *do*, so
  it deserves a mode of its own. Structure comes later, from review, links
  or the LLM.

### 1.2 Karpathy: "LLM Knowledge Bases" thread (X, Apr 2026) and the `llm-wiki.md` gist

- **Three layers:**
  - `raw/` holds sources (articles, papers, images, data). They are
    immutable: the LLM reads them and never edits them.
  - The **wiki** is markdown the LLM writes and owns, and you read:
    summaries, entity and concept pages, comparisons, an overview.
  - The **schema** (`AGENTS.md`/`CLAUDE.md`) turns the LLM into "a
    disciplined wiki maintainer rather than a generic chatbot." You and the
    LLM refine it together over time.
- **`index.md`:** a catalog of every page, each with a link, a one-line
  summary and optional metadata. The LLM reads it first. At about 100
  sources and a few hundred pages it is enough, with no RAG.
- **`log.md`:** a chronological, append-only record with greppable entries
  (`## [2026-11-10] ingest | Title`).
- **Operations:**
  - **Ingest:** one source at a time, supervised. One source may touch
    10–15 pages.
  - **Query:** cited answers as markdown, tables, Marp slides or charts.
    **Good answers are filed back into the wiki**, so what you explore
    builds up.
  - **Lint:** health checks for contradictions, stale claims, orphan pages,
    missing concepts and links, and gaps a web search could fill.
- **Tooling he names:**
  - Obsidian as the viewer ("the LLM is the programmer; the wiki is the
    codebase; Obsidian is the IDE")
  - Obsidian Web Clipper for capture, with images downloaded locally
  - qmd for search at scale
  - Marp, Dataview, and git ("the wiki is just a git repo of markdown
    files")
  - He also wrote his own small search engine, which the LLM uses through a
    CLI.
- **Follow-up post:** one question could start a team of LLMs that builds a
  temporary wiki, lints it, loops and writes a report.
- **Principle:** keeping a knowledge base up to date is mostly bookkeeping,
  which is why people abandon them. LLMs make that bookkeeping nearly free;
  the human curates and asks the questions.

### 1.3 The landscape

| App | Model | Strength | Weakness for us |
| --- | --- | --- | --- |
| Obsidian | Markdown files in a folder; page-first | Your own data, `[[links]]`, graph, Canvas (JSON Canvas spec), Bases (frontmatter views), large plugin ecosystem | Closed source; plugin quality varies |
| Logseq | Outliner; ClojureScript + DataScript | Blocks, journals, Datalog queries | See 1.4; the DB version leaves plain files behind |
| Roam Research | Hosted outliner on a Datomic-like graph | Pioneered block refs `((uid))`, daily notes, Datalog queries | Cloud-only, closed |
| Tana | Outliner with "supertags" (typed nodes) | Structured, queryable notes; AI built in | Cloud-only |
| Capacities, Anytype | Object types (Anytype: local-first, P2P) | Typed objects | Proprietary formats |
| Heptabase | Cards on whiteboards | Spatial thinking | Not plain files |
| Bear, Apple Notes | Simple notes | Instant capture | Weak linking, closed storage |

What lasts across all of them:
- plain local files
- links in both directions
- daily notes
- fast capture
- fast search
- structure added later rather than demanded up front, for example typed
  pages through frontmatter (`type: book`), which is our version of
  supertags

### 1.4 Logseq, in depth (what a Clojure codebase teaches)

The repo (`logseq/logseq`, AGPL-3.0) uses ClojureScript, **DataScript** (an
in-memory immutable Datalog database), an **mldoc** parser in OCaml,
isomorphic-git and SCI.

In April 2026 it split in two:
- **Logseq OG**, which keeps files and gets maintenance fixes only.
- **The DB version** (2.0.x beta), which stores a graph in `db.sqlite` and
  has typed properties, classes/tags, table views and real-time
  collaboration (RTC, in alpha).

Its May 2026 update introduced a "Markdown Mirror", which concedes that
users want grep, git and backups.

Lessons:

1. **Two editable copies of the truth do not work.** The file graph parsed
   markdown into DataScript, edited the DB, and wrote files back. Files
   changed by sync, git or other editors then raced the DB, and that caused
   years of duplicated, lost or corrupted blocks. Logseq gave up on files
   because of it.
   - **For us:** one direction only. Every write is a file write, and the
     index re-reads the file. The index is never edited and can always be
     thrown away and rebuilt.
2. **Writing app state into notes pollutes them.** Logseq writes
   `id:: <uuid>` under any block you reference, plus `collapsed:: true` and
   `key:: value` properties, into your markdown.
   - **For us:** block ids are short `^ab12` anchors, added only to blocks
     that are referenced, which is Obsidian's syntax. Fold state lives in
     app state, never in files. Properties are YAML frontmatter.
3. **Keep the graph out of the UI process.** Logseq OG held the whole graph
   in DataScript in the renderer, and big graphs got slow to open and to
   query.
   - **For us:** the index lives in the Rust provider, and the UI asks for
     pages of results (which also fits the host's 1 MiB publish limit).
4. **One parser.** mldoc (OCaml) parsing and the editor's own rendering
   drifted apart.
   - **For us:** the index and `ui.markdown` use the same parser, the
     `markdown` crate.
5. **What to keep:**
   - the journal as the home page (open today's note)
   - outliner editing (indent/outdent, move a block, zoom into a bullet,
     collapse)
   - block references and embeds
   - queries over a graph of entities
   - namespaces (`a/b`)
   - typed tags (the DB version's classes, Tana's supertags)
6. **Datalog.** Its appeal (an entity–attribute–value graph you query with
   Datalog) is real.
   - **For us:** the SQLite index stores links, tags, properties, blocks and
     tasks as relations. A query layer starts as a small Dataview/Bases-like
     DSL compiled to SQL; a Datalog surface (EDN-flavoured) is a later
     option over the same relations.
   - SQLite stays the store. Of the embeddable Rust Datalog engines, Cozo
     looks to have stalled, and a stalled store is the wrong thing to build
     the foundations on.
7. **The license is AGPL.** Moonframe is MIT, so we take ideas only, never
   code.

### 1.5 qmd (tobi/qmd, MIT, TypeScript on Node 22+ or Bun)

- **Storage:** one SQLite file, with FTS5 for BM25 (`documents_fts`) and
  sqlite-vec for vectors. Documents have 6-character docid hashes, and
  query expansion and reranking results are cached (`llm_cache`).
- **Search modes:** `search` (BM25), `vsearch` (vector) and `query`
  (hybrid). Hybrid runs four steps:
  1. LLM query expansion into `lex`, `vec` and `hyde` sub-queries
  2. FTS and vector retrieval
  3. Reciprocal Rank Fusion (k = 60, the original query weighted ×2, a
     bonus for rank 1 and the top 3)
  4. a reranker over the top 30, with **position-aware blending** (ranks
     1–3 keep 75% of their retrieval score, 4–10 keep 60%, 11+ keep 40%),
     so the reranker can't throw out a strong exact match
- **Models:** local GGUF models through llama.cpp: embeddinggemma-300M for
  embeddings, qwen3-reranker-0.6B, and a 1.7B query-expansion model.
- **Chunking:** about 900 tokens with 15% overlap, preferring to break at
  headings, then fences, rules and blank lines, and never inside code.
- **Collections:** folders with globs. **Path contexts** (descriptions per
  path prefix) are returned with each result, so the agent knows what kind
  of document it is reading.
- **Interfaces:** frontmatter metadata filters, a CLI, and an MCP server
  (stdio, or HTTP on localhost with models kept loaded).
- **For us:** qmd is exactly the "search at scale" layer Karpathy points
  to. But it needs Node or Bun, and Commonplace has to ship as a standalone
  app with nothing else to install. Hence the two stages in 3.4.

### 1.6 What Moonframe already has

- **`crates/moonframe-documents`** (Folio): a markdown folder with debounced
  writes and an atomic recovery journal.
  - Limits: 500 documents, 256 KiB per file, 900 KiB published.
  - It notices outside changes by re-scanning the folder on its tick
    instead of being told by the file system.
  - Its journaled writes are the model for ours.
- **`crates/moonframe-acp`:** runs Claude, Codex, Gemini, OpenCode or
  Copilot as agents over ACP, with permission questions and **undo of a
  whole run** (`Keeper`). This is the wiki maintainer.
- **`crates/moonframe-git`** and **`crates/moonframe-history`:** commits,
  and snapshots for undo.
- **UI:**
  - `ui.markdown` (frontmatter, tables, tasks, footnotes; link clicks send a
    message to the app, so wikilinks can hook in)
  - `ui.input` (`prose`, `typewriter`, `code` modes) with the
    `prose-editor` and `markdown-editor` components
  - `ui.list` (a virtual list), `side-panels`, `command-palette`, `outline`,
    `tree`, `tabs`
  - `ui.paths` (lines and arrows) and `ui.view` (a view written in Rust,
    for a graph)
- **Limits to design around:**
  - Provider data is at most 1 MiB, 10k values and depth 16.
  - Saved UI state is at most 40k entries.
  - There is no full-text search, no file-system watching for user data,
    no `[[` completion inside `ui.input`, no wikilink rendering, and no
    graph view.
- **Adding a native feature:** a new crate with `native()`, an optional
  dependency and a feature in `crates/moonframe/Cargo.toml` and
  `crates/moonframe-app/Cargo.toml` (in the keep-sorted lists), and a
  `#[cfg]` constructor in `crates/moonframe/src/natives.rs`.

---

## 2. Decided

1. **The files are the truth.** The index is derived, one way only, and can
   be rebuilt at any time.
2. **The vault layout** is Karpathy's plus a space for your own notes (3.1).
3. **Stable identity:** a ULID `id:` in the frontmatter, links by title,
   renames that rewrite links, and `^id` block anchors.
4. **The engine crate** `moonframe-notes` uses SQLite and FTS5, with
   file-system watching, journaled writes and conflict handling.
5. **The app asks for what it shows.** The provider answers queries and
   never publishes the whole vault.
6. **Agent runs** are each one ACP run, one `log.md` entry and one git
   commit, with the diff reviewed before acceptance and undo for the whole
   run.
7. **Obsidian compatibility:** yes.
8. **Name:** Commonplace (`examples/commonplace`, `crates/moonframe-notes`).
9. **Search:** qmd-style hybrid search (3.4).
10. **Imports** from the well-known apps (3.7). There is no existing vault
    to start from.

11. **Editing model: C, hybrid** (picked 2026-11-10 from the mockups).
    - Notes are documents.
    - Any markdown list can be edited as an outline (Tab/⇧Tab, move, zoom,
      fold).
    - The journal and the inbox open in outline mode.
    - Files stay plain markdown lists, with `^id` only on referenced blocks,
      and fold state lives in app state.

---

## 3. Architecture

### 3.1 The vault format (the contract)

```
vault/
  AGENTS.md          schema: how the LLM maintains the wiki (template shipped)
  index.md           LLM-maintained catalog of wiki pages
  log.md             append-only: "## [YYYY-MM-DD] <op> | <title>"
  inbox.md           append-and-review note (newest at the top)
  journal/           YYYY-MM-DD.md daily notes        (human)
  notes/             your own pages                   (human)
  raw/               clipped and imported sources     (immutable)
    assets/          downloaded images and PDFs
  wiki/              LLM-written pages                (LLM)
  .commonplace/      index.sqlite, settings; gitignored, safe to delete
```

**Who owns what.** The ownership comes from the folder, and frontmatter
`owner:` can override it.

| Path | Owner | What the agent may do |
| --- | --- | --- |
| `wiki/`, `index.md`, `log.md` | LLM | Write directly |
| `raw/` | Nobody | Write only on ingest |
| `notes/`, `journal/`, `inbox.md` | You | Propose changes, which you accept or reject |

**Syntax: Obsidian's, written down in `docs/` as a grammar with test
fixtures.**
- Links: `[[Title]]`, `[[Title|alias]]`, `[[Title#Heading]]`,
  `[[Title#^blockid]]`
- Embeds: `![[...]]`
- Tags: `#tag/nested`
- Frontmatter: YAML, with `id`, `aliases`, `tags`, `type`, `source`,
  `created`, and free keys
- Block anchors: `^blockid`
- Tasks: `- [ ]`
- Callouts: `> [!note]`

**Identity:**
- A note gets a ULID `id:` when it is created. Imported notes get one
  lazily, the first time the app writes to them.
- Links resolve by title, aliases, then path, the way Obsidian does.
- A rename rewrites every link to the note in one journaled batch, which is
  one undo step.
- Duplicate titles are reported, and links to them are disambiguated by
  path.

### 3.2 The engine: `crates/moonframe-notes` (no UI)

```
parse  (markdown crate + wikilink/tag/anchor extension; same parser as ui.markdown)
  -> note { id, path, title, aliases, frontmatter, headings, blocks, links, tags, tasks, hash }
index  (SQLite .commonplace/index.sqlite, schema versioned by append-only migrations)
  notes(id, path, title, type, mtime, size, hash, owner)
  links(src, dst_title, dst_id?, kind[link|embed], anchor, line)   -- resolved lazily
  aliases, tags(note, tag), props(note, key, value_json)
  headings(note, level, text, line), blocks(note, anchor, line, text)
  tasks(note, line, done, text, due?)
  fts5(title, aliases, body, tokenize='unicode61 remove_diacritics 2')  -- BM25
  chunks(note, ord, start, end, heading_path)  -- ready for vectors later
watch  notify (FSEvents/inotify) + startup reconcile by (mtime, size) then hash
write  journal -> atomic rename -> re-index that file
       (the journal comes from moonframe-documents)
conflict  if the file on disk changed since it was opened (hash):
          a 3-way merge when the changes don't overlap, otherwise keep both
          and show the conflict. Never overwrite silently.
```

- **Queries:**
  - `search(q, filters, page)`
  - `quick_switch(prefix)`, which ranks titles, aliases and how recently a
    note was opened
  - `backlinks(id)` and `unlinked_mentions(id)`
  - `outgoing(id)`
  - `tags()`, `by_tag()`, `by_prop()`
  - `tasks(filter)`
  - `orphans()`, `broken_links()`, `stats()`
- **Three ways in, one API:**
  1. the native provider for the app
  2. a CLI, `commonplace search|backlinks|get|lint|reindex --json`
  3. an MCP server for agents, so Claude asks the index instead of grepping
- **Benchmarks** are written before the UI, on generated vaults of 1k, 10k
  and 50k notes:

  | Measure | Target |
  | --- | --- |
  | Cold index of 10k notes | under 5 s |
  | Re-indexing one changed file | under 50 ms |
  | BM25 search, 95th percentile | under 20 ms |
  | Quick switcher | under 16 ms |
  | Backlinks | under 5 ms |

### 3.3 The provider protocol (to stay under the 1 MiB limit)

The app sends `{ query = { id, kind, args, offset, limit } }` and gets a
reply with only that page.

Published state stays small:
- `vault`: path and counts
- `open`: the open note's text and hash, with large notes sent in chunks
- `status`: indexing progress, conflicts, the agent run

The note list, search results, backlinks and tags are all paged queries. A
growing vault never grows what gets published.

### 3.4 Search: qmd-style, in two stages

- **Stage 1 (ships with the engine):**
  - FTS5 BM25 with prefix and phrase search, plus filters: `tag:`, `type:`,
    `path:`, `prop:key=value`, `has:task`, dates.
  - Titles and aliases are ranked above body text.
  - Search runs as you type.
- **Stage 1b: qmd as an option.**
  - If qmd is installed, Commonplace registers the vault as a qmd
    collection.
  - The folder descriptions in `AGENTS.md` become qmd path contexts.
  - Agents get qmd's MCP server.
  - The app offers a "deep search" that calls `qmd query --json`.
  - Nothing breaks without it.
- **Stage 2: hybrid search built in, so the standalone app needs nothing
  installed.**
  - The same index file, adding sqlite-vec and chunk embeddings computed in
    the background (embeddinggemma through llama.cpp bindings, Metal on
    macOS).
  - qmd's fusion: RRF k = 60, the original query weighted ×2, a top-rank
    bonus, an optional reranker over the top 30, position-aware blending.
  - qmd's heading-aware ~900-token chunks.
  - Models download on first use and can be turned off.
  - It has to match qmd on qmd's own `bench` fixtures before it replaces
    stage 1b.

### 3.5 The LLM wiki (Karpathy's loop)

- **Agent:** through `crates/moonframe-acp` (Claude by default).
- **Instructions:** the vault's `AGENTS.md`, plus the mod's own rules about
  who owns what and the formats of the log and index.
- **Tools:** the MCP server's search, backlinks and lint, plus file writes
  that `guard` limits to `wiki/`, `index.md` and `log.md`. Changes to your
  own pages come back as proposals.
- **Operations** (each one run, one `log.md` entry, one git commit):
  - **Ingest** a `raw/` source: summarize it, update entity and concept
    pages, the index and the log.
  - **Ask:** a cited answer, with "file back" saving it as a wiki page.
  - **Lint:** contradictions, stale claims, orphans, missing concept pages,
    broken links. The engine finds the mechanical problems (orphans, broken
    links) itself; the LLM handles meaning.
  - **Review inbox:** suggest what in `inbox.md` should become a note, a
    task or a wiki entry. Suggestions only.
- **Review screen:** what each run changed, page by page, with accept,
  reject or undo for the whole run (`Keeper`, `moonframe-history`).
- **Scheduling:** a nightly lint, using Pilot's scheduled runs as the model.

### 3.6 The app: `examples/commonplace`

- **Layout:** `side-panels`.
  - Left: Inbox, Today, Notes, Wiki, Raw, Tags, saved searches.
  - Middle: the list.
  - Right: backlinks, unlinked mentions, outline, properties and the agent.
- **Home is Today.** The global capture key adds to the top of `inbox.md`,
  in the append-and-review spirit.
- **Review mode** walks the inbox from the bottom up, with *keep* (move to
  top), *make note*, *task* and *let sink*.
- **Keys:**
  - ⌘O quick switcher, ⇧⌘F search, ⌘K command palette
  - ⌘[ / ⌘] back and forward
  - ⌘-click opens a note in a split
- **Editor:** `prose-editor` with live markdown; lists get the outliner
  keys (decision 11).
- **Host gaps to close, each as a small host change of its own:**
  - `[[` / `#` completion at the caret in `ui.input`
  - wikilinks, embeds and local images in `ui.markdown`
  - outliner editing commands on list lines
  - later, a force-directed graph as a Rust `ui.view`

### 3.7 Imports

Each source is a module under `crates/moonframe-notes/src/import/`. Every
import:
- writes a new vault and **never touches the source**
- has a dry run that reports what would be converted and what would be lost
- can be run again (re-importing updates, it doesn't duplicate)

| Source | What it involves |
| --- | --- |
| Obsidian vault | Almost nothing: open in place or copy. Read `.obsidian/daily-notes.json` and the attachment folder setting; leave `.obsidian/` alone |
| Markdown folder (Foam, Dendron, iA Writer, Zettlr) | Open as-is. Dendron's `a.b.c` names become folders and aliases |
| Logseq file graph | `pages/` and `journals/` (rename journal files to dates). `key:: v` page properties become frontmatter. `((uuid))` refs become `[[Page#^id]]` with `^id` added to the target block. Drop `collapsed::` and `id::` lines. `{{embed}}` becomes `![[...]]`. Namespaces become folders plus aliases |
| Logseq DB graph | Through its markdown export ("Markdown Mirror") or reading `db.sqlite` read-only; whichever proves stable |
| Roam JSON export | A block tree with uids; daily titles ("November 10th, 2026") become `journal/2026-11-10.md`; `((uid))` and `{{[[embed]]}}` handled as for Logseq; `attr::` becomes frontmatter |
| Notion export (markdown + CSV) | Strip the ID suffixes from file and link names; databases become notes with frontmatter (one per row) |
| Bear (markdown/TextBundle export) | `#tag/sub#` tags; attachments go to `raw/assets` |
| Apple Notes | Later: export through AppleScript, convert HTML to markdown |
| Evernote ENEX | Convert to markdown, attachments go to `raw/assets` |
| Readwise and Kindle highlights | Go into `raw/` as sources to ingest |

---

## 4. Phases

0. **Spec (a few days).**
   - The vault-format document with the link grammar and ownership rules.
   - An `AGENTS.md` template.
   - Test fixtures: a vault of Obsidian edge cases (aliases, spaces and
     unicode in names, embeds, duplicate titles, block refs).
1. **Engine.**
   - Parse, index, watch, query, write and rename, plus conflicts.
   - The CLI, tests, benchmarks and the generated 50k-note vault.
   - Done when the targets in 3.2 pass in CI.
2. **Minimal app.**
   - Open or create a vault, the list, the editor, the quick switcher, BM25
     search, backlinks.
   - Today and daily notes, the inbox with quick capture and review mode.
   - Screenshots in the PR.
3. **Linking UX.**
   - Close the host gaps: completion, wikilink rendering, embeds.
   - Rename refactoring, unlinked mentions, tags, the properties panel.
   - Outliner mode over lists (decision 11).
4. **Imports:** Obsidian, markdown folders, Logseq and Roam first; the rest
   after.
5. **LLM wiki:** the MCP server, ingest, ask with file-back, lint, the review
   screen, undo, the nightly lint.
6. **Capture:** a URL clipper (fetch, extract the readable text, convert to
   markdown, download images) and PDF import.
7. **Search stage 2:** vectors, the reranker, hybrid fusion, matched against
   qmd's bench.
8. **Later:** the graph view, Bases-style table views, JSON Canvas, a
   Datalog query surface, a multi-agent research mode (Karpathy's follow-up
   post).

Repository rules: the engine is a new crate behind a `notes` feature in the
keep-sorted lists, the app is a new example, and each host gap is its own
small PR. Nothing is appended to shared files.

## 5. Risks

- **External edits racing ours** (Obsidian, iCloud, git pull, the agent).
  Hashes on open, journaled writes and merge-or-keep-both conflicts must be
  in phase 1, not bolted on later.
- **Drift between the index and the renderer.** One parser, plus the shared
  fixtures.
- **Outliner editing on plain markdown** is the hardest piece of UI. Keep it
  a mode over lists, behind its own tests.
- **The agent overwriting your own words.** Ownership by folder, `guard`,
  proposals instead of edits, and undo per run.
- **The publish limits.** A paged query protocol from day one.
- **Model downloads for stage 2.** Opt-in, cached, and the app works fully
  without them.

## Sources

- Karpathy, [The append-and-review note](https://karpathy.bearblog.dev/the-append-and-review-note/)
- Karpathy, [llm-wiki.md gist](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
- [VentureBeat on the LLM Knowledge Bases thread](https://venturebeat.com/data/karpathy-shares-llm-knowledge-base-architecture-that-bypasses-rag-with-an), [thread summary](https://hq-raydata-co.pages.dev/vault/06-reference/2026-04-01-karpathy-llm-knowledge-bases/)
- [tobi/qmd](https://github.com/tobi/qmd)
- [logseq/logseq](https://github.com/logseq/logseq), [What's New with Logseq DB, May 2026](https://discuss.logseq.com/t/whats-new-with-logseq-db-may-16th-2026/35020), [Logseq DB on SQLite](https://preslav.me/2025/10/13/how-to-install-logseq-db-version-on-your-computer-sqlite/)
- [Logseq vs Roam Research (Ponder)](https://ponder.ing/blog/logseq-vs-roam-research), [Travis Chan, Logseq vs Roam](https://travischan.medium.com/logseq-vs-roam-research-65168fe57464) (could not be fetched: 403)
- [Best second-brain apps 2026 (Recall)](https://www.recall.it/compare/best-second-brain-apps)

---

## 6. Progress

**2026-11-10**

- **Phase 0 is done.**
  - The vault format spec is in `examples/commonplace/VAULT.md`.
  - The fixture vault of Obsidian edge cases is in
    `crates/moonframe-notes/tests/fixtures/vault`.
- **Phase 1 engine** (`crates/moonframe-notes`, plus the `commonplace`
  CLI):
  - Built: parsing, the SQLite and FTS5 index, link resolution, scanning,
    journaled atomic writes, three-way merge with conflict copies, and
    rename with link rewriting.
  - 44 tests pass.
  - Not built yet: the file watcher, which belongs in the provider.
- **The app** (`examples/commonplace`):
  - View > Library / Journal / Workspace Layout (⌥⌘1, ⌥⌘2, ⌥⌘3), with the
    choice kept across launches.
  - All three layouts show demo data until the provider exists.

**Benchmarks** (release build, synthetic vaults):

| Measure | 10k notes | 50k notes | Target |
| --- | --- | --- | --- |
| Cold index | 1.6 s | 13 s | under 5 s at 10k |
| Re-index one file | 8 ms | 43 ms | under 50 ms |
| Search, 95th percentile | 12.5 ms | 65 ms | under 20 ms |
| Quick switcher, 95th percentile | 4.9 ms | 30 ms | under 16 ms |
| Backlinks | under 0.1 ms | under 0.1 ms | under 5 ms |

- Every target passes at 10k.
- At 50k, search and the quick switcher miss their targets.
  - The synthetic notes use a vocabulary of 20 words, so every term
    matches every note. That is a worst case for search.
  - The quick switcher reads every title on each keystroke. Keeping the
    titles in memory would fix it.

**2026-11-10, later**

- **The `notes` provider is built** (`crates/moonframe-notes/src/provider`,
  the `notes` feature, on in the starter):
  - It watches the vault and runs **live queries**: a mod sends `watch`,
    and the provider sends the answer again whenever it changes.
  - Saves wait for typing to stop, then are merged against the provider's
    record of each note's last base text.
  - When a save of its own comes back from the watcher, the editor's text
    is left alone.
  - It handles create (with a unique name), rename, trash, capture to the
    inbox or today's note, the inbox review (keep, note, task, done), and
    today's note.
  - It starts a new user with a starter vault (`AGENTS.md`, Welcome).
- **The index now lives in the cache folder**
  (`~/.cache/commonplace/<hash>/`), so nothing syncs it. The vault's
  `.commonplace/` holds only the write journal and the trash.
- **The Notes.app import is built**
  (`crates/moonframe-notes/src/import`):
  - JavaScript for Automation run by `osascript` reads every folder; each
    folder's notes go to their own JSON file, so memory stays bounded.
  - Notes' HTML becomes markdown, and pictures go under
    `raw/assets/apple-notes/`.
  - Created and updated dates and the Notes id go in the frontmatter.
  - A manifest makes importing again update notes changed in Notes.app,
    without overwriting ones edited in the vault.
  - Locked notes are skipped and reported. Attachments other than pictures
    are not imported yet.
  - It runs from File > Import from Notes.app… or
    `commonplace import apple-notes [--dry-run]`.
- **All three layouts run on live data.** The demo provider is gone.
- **Not yet run against a real Notes.app library.**

**2026-11-11**

- **Combined import:** Notes.app's own Markdown export (macOS 26), joined
  with what Notes.app says of each note.
  - The export gives the bodies and every attachment. Notes.app gives the
    folders, dates and ids.
  - Files match notes by title (NFC, lowercase, letters and digits only).
    Notes that share a title match by how many words they have in common.
  - Notes missing from the export fall back to the HTML conversion.
    Exported files that match no note go to `notes/From the export/`.
  - It runs from `commonplace import apple-notes --from-export DIR` or File >
    Import from Notes.app with Its Markdown Export…
- **The export-only path was checked on the user's real export**, 1,372
  notes:
  - All 1,372 imported, and 277 attachments copied.
  - One dangling link remains, to an attachment missing from the export
    itself.
  - URLs with a scheme (`tel:`, `applenotes:`) now count as external.
  - Empty `#` heading lines above titles are skipped.
- **What the export lacks** (seen in that real export): dates (every file
  is stamped with the export time) and folders (it is flat). Links between
  notes come out as `applenotes:note/<uuid>` and cannot be resolved yet.

**Next:**

1. ~~A native provider~~ (done)
2. Titles kept in memory for the quick switcher.
3. The MCP server, and the wiki agent's ingest, ask and lint runs over ACP
   in the Workspace's wiki panel.
4. Outliner editing keys in the journal.

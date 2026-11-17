# The Commonplace vault format

This is the contract between your files, the notes engine
(`crates/moonframe-notes`), the app, and any agent that edits the vault.
It is Obsidian's format. Whatever Obsidian reads, we read; whatever we
write, Obsidian reads.

The rationale behind it is in `plans/commonplace.md`.

## 1. The files are the truth

- A vault is a folder of UTF-8 markdown files (`.md`) and their
  attachments.
- Everything the app derives (the index, search, the link graph) lives in
  the cache folder (`~/.cache/commonplace/<hash of the vault's path>/`),
  outside the vault and whatever syncs it, and can be deleted at any time.
- `.commonplace/` in the vault holds only the write journal and the trash.
- The engine only ever reads files into the index. It never edits the
  index directly: every change is a file write, which is then indexed
  again.
- App state (folded blocks, open tabs, scroll position) is never written
  into notes.

## 2. Layout

```
AGENTS.md      how the LLM keeps the wiki (the schema); the human edits it
index.md       catalog of wiki pages, kept by the LLM
log.md         append-only record of agent runs, kept by the LLM
inbox.md       the append-and-review note: newest at the top
journal/       daily notes, journal/YYYY-MM-DD.md
notes/         your own notes
raw/           sources: clipped, imported, never rewritten
raw/assets/    images and PDFs the sources and notes use
wiki/          pages the LLM writes
.commonplace/  the write journal and the trash; gitignored
```

- Only `.commonplace/` is required. An imported or existing vault with
  other folders works: each folder is treated as `notes/` unless settings
  map it.
- `.obsidian/` is left alone.

## 3. Ownership

| Path | Owner | What an agent may do |
| --- | --- | --- |
| `wiki/`, `index.md`, `log.md` | llm | write |
| `raw/` | source | add new files on ingest; never change an existing one |
| everything else | human | propose a change; the human accepts or rejects it |

A frontmatter key `owner: human | llm` overrides the folder for that one
note.

## 4. A note

- Its file name, minus `.md`, is its **title**. A `title:` in the
  frontmatter is shown instead, but links still resolve by file name and
  aliases, as in Obsidian.
- **Frontmatter** is optional YAML between `---` lines at the very top.
  Known keys:

  | Key | Meaning |
  | --- | --- |
  | `id` | a ULID, written when the app creates the note; never changes on a rename |
  | `aliases` | other titles that resolve to it |
  | `tags` | list or string, without `#` |
  | `type` | a free word (`concept`, `person`, `book`), our "supertag" |
  | `created`, `updated` | ISO dates |
  | `source` | a link or URL the note came from |
  | `owner` | see section 3 |

  Any other key is a property, kept as written. Values are YAML scalars,
  lists, or `"[[links]]"`.

## 5. Inline syntax

| Syntax | Meaning |
| --- | --- |
| `[[Title]]` | link to a note |
| `[[Title\|text]]` | link shown as `text` |
| `[[Title#Heading]]` | link to a heading |
| `[[Title#^ab12cd]]` | link to a block |
| `[[#Heading]]` | link within the same note |
| `![[Title]]`, `![[Title#Heading]]`, `![[Title#^id]]` | embed (transclusion) |
| `![[raw/assets/x.png]]`, `![alt](raw/assets/x.png)` | image |
| `[text](https://…)`, `[text](Other%20note.md)` | markdown links, both resolved |
| `#tag`, `#tag/nested` | tag: letters, digits, `_` `-` `/`, at least one non-digit |
| ` ^ab12cd` at the end of a block | block anchor: 1–32 of `a-z 0-9 -` |
| `- [ ] task`, `- [x] done` | tasks |
| `> [!note] Title` | callout |
| `%% comment %%` | hidden comment |

Links, tags and anchors inside code spans, code blocks and comments
don't count. In frontmatter, `[[links]]` in property values count (as in
Obsidian) and tags come only from the `tags` key.

## 6. Resolving a link

`[[Target]]` resolves in this order, case-insensitively:

1. a path relative to the vault root (with or without `.md`)
2. a file name
3. an alias

When several notes match, the one closest to the linking note's folder
wins, then the shortest path. The engine reports the ambiguity, and the
app offers to rewrite the link as a path. An unresolved link is a
**dangling link**, listed as a page that doesn't exist yet, as in
Obsidian.

## 7. Blocks and the outliner

- A block is a paragraph, a list item (with its children), a heading's
  line, a quote or a fence.
- Outline mode (the journal, the inbox, any list) edits list items as an
  outline: indent, outdent, move, zoom, fold. The file stays a plain
  markdown list: two-space or tab indentation, as it was written.
- A block gets a `^id` anchor only when something links to it. The app
  adds the anchor at the moment the link is made.
- Folding is app state and is never written into the file.

## 8. Daily notes and the inbox

- `journal/YYYY-MM-DD.md`. An Obsidian `.obsidian/daily-notes.json`
  folder or format is honoured on import.
- `inbox.md` holds a list of captures, newest first.
  - Review moves an item to the top (keep), into a note (make note), into
    a task, or leaves it (let it sink).
  - Each item is a list block, so it can be linked like any other.

## 9. The agent's files

- **`log.md`**: each entry starts `## [YYYY-MM-DD] <op> | <title>`. `op` is
  one of `ingest`, `query`, `lint`, `review`. A short body follows: the
  pages touched, and the commit.
- **`index.md`**: one section per category. Each entry is
  `- [[Page]]: one-line summary`.
- **`AGENTS.md`**: the schema. A template is written into a new vault, and
  you and the agent refine it together.
- **Commits**: each run is one git commit when the vault is a git
  repository, with the message `<op>: <title>`.

## 10. Writes

- Every write goes to the journal (`.commonplace/journal/`), then to a
  temporary file, then an atomic rename.
- A write carries the hash of the text it started from. If the file
  changed on disk since then, there are two cases:
  - the changes don't overlap: merge them
  - they do: keep both, the other version as `Title (conflict YYYY-MM-DD
    HHMM).md`, and show the conflict
- A rename rewrites every link to the note in one journaled batch, which
  is one undo step.

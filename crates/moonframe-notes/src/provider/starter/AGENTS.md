# How this vault is kept

This vault is a second brain: your notes, the sources you collect, and a wiki an LLM keeps. It follows Obsidian's markdown format (see Commonplace's VAULT.md).

## Folders and who owns them

- `notes/`, `journal/`, `inbox.md`: the human's. An agent never edits them; it proposes changes instead.
- `raw/`: sources (clipped articles, papers, imports). Never changed once written.
- `wiki/`, `index.md`, `log.md`: the agent's. It writes and maintains them.

## The agent's work

- **Ingest** a source from `raw/`: summarize it, then update the entity and concept pages in `wiki/` it touches, `index.md`, and `log.md`.
- **Ask**: answer from the wiki with links to the pages used. A good answer is filed back as a wiki page.
- **Lint**: look for contradictions, stale claims, orphan pages, concepts without a page, and missing links.

## Conventions

- Link with `[[Page]]`. One concept per page. Page names are plain titles.
- `log.md` entries start `## [YYYY-MM-DD] <ingest|query|lint> | <title>`.
- `index.md` lists every wiki page as `- [[Page]]: one-line summary`, by category.

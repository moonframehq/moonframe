# folio

**Folio**, a [Moonframe](../..) app for writing: a library of markdown
documents beside a quiet page, in the spirit of minimalist writing apps.

- **The library** (`⌘1`): All Documents, Favorites and Trash, with counts,
  and New Document (`⌘N`).
- **The documents** (`⌘2`): the chosen section's documents, newest first,
  each with its title, two lines of what it says, when it changed and how
  long it is. Search filters them; a right-click stars one, moves it to the
  Trash, or (in the Trash) puts it back or deletes it.
- **The page**: the open document in a centred column, markdown styled as
  you type (headings bold, emphasis slanted, the markup faded).

| Key | Does |
| --- | --- |
| ⌘D | focus mode: the sentence, then the paragraph, being written stays in full ink and the rest fades; a third press shows all (Esc too) |
| ⌘T | typewriter mode: the line being written stays in the middle of the window |
| ⌘R | preview: read the document as it will look, and back to writing |
| ⇧⌘F | switch between the monospace and the sans font |
| ⌘N | new document |
| ⌘⌫ | move the open document to the Trash |
| ⌘1 / ⌘2 | show or hide the library / the documents |

The footer shows one figure of the document's length, words, characters,
sentences or reading time; click it for the next.

Documents are plain `.md` files in
`~/.local/share/moonframe/folio/documents` (the profile's folder under
`$XDG_DATA_HOME`), started with a few samples the first time. A Python
provider (`providers/library`) lists them, opens one, and writes your edits
once you stop typing. Each edit first goes to an atomic recovery journal,
so restarting the provider recovers edits still waiting to be written.
Edit the files elsewhere and the list follows.

```sh
moonframe app link examples/folio   # from the repository
moonframe --profile folio
```

It is made of the `prose-editor`, `document-row` and `writing-stats`
components, and the writing modes of `ui.input` (`prose`, `dim`,
`typewriter`) that `prose-editor` draws.

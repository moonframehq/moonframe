# projects

A Moonframe package for apps that work across many repositories, which the
[sessions](../sessions) and [workbench](../workbench) apps require.

- `providers/projects`: every git repository under your project folders
  (`~/code`, `~/dev`, `~/src`, `~/Projects`, `~/work`, herdr worktrees, or
  `MOONFRAME_PROJECTS`), with branch and changes; and for the one an app
  opens, its changed files, folders and a file's text. Shared as
  `app.projects`; see `projects.py` for the shape. Each project lists its
  git work trees, the open one its last commits, and on request its pull
  requests and workflow runs from `gh`. It also commits, pulls, pushes and
  adds work trees, in the background, for the mods named in
  `MOONFRAME_PROJECTS_WRITERS` (by default [trellis](../trellis)'s). A process provider in
  Python: it runs git and reads files with your access.
- `components/`: `file-tree`, `change-tree`, `code` (syntax-coloured
  source) and `file-icon`.
- `style.luau`: the colours `code` paints with.

# research

**Sorrel**, a [Moonframe](../..) app for an agent's research workspace:

- **Left:** the workspace, New and search, Home, Projects, Templates,
  Artifacts and Tasks, recent projects and teams (only "Savings launch, Q1"
  is in this example), the plan's credits as a segmented bar, and who is
  signed in.
- **Top:** the project's name and star, a search field, who is in it, Share.
- **Middle:** the main thread: the ask, the agent's reply, and its plan as a
  card that advances step by step: found apps, then three sub-agents
  (Banks, Neobanks, Apps) with a bar each and what one is reading now, then
  reviews and a summary, with the time, credits used and a note box. The
  round button pauses the agent, resumes it, and replays the run once done.
- **Right:** the sheet it fills. Step 1 adds a row per app found; step 2
  writes APY, monthly fee, minimum deposit and rating cell by cell, the row
  being worked on highlighted and the cell being typed outlined with a
  caret, the cells still to come as grey skeleton bars. The download button
  above it saves the sheet as it stands as an A4 PDF (`app:pdf`, macOS), in
  `~/.cache/moonframe/sorrel/competitors.pdf`, and its tooltip says where. The grid is the
  `spreadsheet` component (`app/components/spreadsheet.luau`).

The agent is a **scripted demo**, not a model: `providers/research`, a Rust
provider built to WebAssembly, builds in the sample apps from `sample.json`
and plays the run on a clock (a tick every 0.4 s), publishing it as
`app.research.run`. A real agent would publish its plan and cells the same
way. `start_tick` in its `provider.toml` starts the run part way through.

```sh
moonframe app link examples/research   # from the repository
moonframe --profile research
just bundle-app research               # target/bundle/Sorrel.app
```

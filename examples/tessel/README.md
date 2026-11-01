# tessel

**Tessel**, a [Moonframe](../..) app for a team's AI agents, each with its own
pixel-art face:

- **Left:** the brand, New Chat, Automations (Agents, Intelligence), projects,
  the latest agent chats with their sprites, the other chats, and who is
  signed in. The fold button (or ⌘S) turns it into a rail of icons.
- **My Agents:** your agents as cards (icon, name, what it does, last used,
  owner, type) or as a table (the grid/list switch), sorted by Latest used,
  Name or Owner, filtered by type (+ Filters) or by name (Find, ⌘F). Under
  them, agents to browse with a Launch button. "Create an Agent" (⌘N) adds
  one with a sprite made from its name; the ··· menu holds the rest.
- **Agent Hub:** the runs' figures (Active, Completed, Needs Review, Pending,
  Failed) for all agents or one (Agent: …), then the latest activity with
  status pills, filtered by status. A row folds open to its steps, newest
  first; a failed step offers **Re-run** and **Manage workflow**.
- **Workflow:** a modal with a chat on the left and the run's steps as cards
  from Start, the failed one outlined in red. The chat folds away, the modal
  fills the window, + and − zoom the cards, and the failed step's fix
  ("Connect to Drive") or Re-run sets the run going again, in the hub too.
- **An agent's page:** click a card. A chat with the agent (with Research,
  Think and Tools), and its details: instructions, guardrails, tone,
  knowledge and tools with switches, or its runs under Activity.

The chats are **scripted demos**, not models: replies come from a few rules.
The agents, runs and chats are **sample data** in
`providers/tessel/sample.json`, published by a small Perl process provider;
point it at your agent platform to make it real. The sprites are drawn by the
`pixel-icon` component from a few rows of digits.

```sh
moonframe app link examples/tessel   # from the repository
moonframe --profile tessel
just bundle-app tessel               # target/bundle/Tessel.app
```

# T3 Overview

A Moonframe app for the sessions in a local T3 Code environment. Fold a group or click a session to see its provider, worktree and last update. Click a PR or issue badge to open it in the browser. The tabs filter sessions to those with linked pull requests, issues or worktrees. The machine and usage cards read Moonframe's system and usage providers.

![T3 Overview rendered with sample sessions and usage](../../docs/screenshots/apps/t3-overview.png)

## Run it

With T3 Code running locally, run these commands from the Moonframe checkout. Setup uses the T3 CLI to issue a token scoped to `orchestration:read`. Python 3 runs the provider. This flow has been tested with T3 Code 0.1.11.

```sh
python3 app/providers/t3/t3.py --connect
cargo run --locked -p moonframe-app -- app link examples/t3-overview
cargo run --locked -p moonframe-app -- --profile t3-overview app
```

Setup saves the token privately in `$T3CODE_HOME/moonframe-token.json`. The default home is `~/.t3`. If a Sheprd pairing already exists for this environment, skip setup and launch Moonframe with `MOONFRAME_T3_TOKEN_FILE` set to the file created by `sheprd-t3 pair --token-file FILE`. The provider checks the environment ID before sending a token and discovers the loopback origin from T3's runtime file. Run setup again when the Moonframe token expires.

## Start chats and send follow-ups

Launch the example with local writes enabled for its shell. Put the T3 CLI on the same PATH as Moonframe.

```sh
MOONFRAME_T3_WRITER=t3-overview/shell cargo run --locked -p moonframe-app -- --profile t3-overview app
```

Choose **New chat**, pick a project (including one without sessions), enter a prompt and select **Start chat**. For a follow-up, select a session, choose **Reply**, and send the message. A busy thread queues it for its next turn. The composer retains the text on an error. If delivery is uncertain, check the thread before retrying.

![T3 Overview with its project picker and composer](../../docs/screenshots/apps/t3-overview-compose.png)

The opt-in names one profile and one sending mod, in the form `profile/mod`. Omit it to run the overview read-only. The provider still uses its paired read token to discover projects and threads. Writes run the local `t3 thread start` or `t3 thread send` command with the machine owner's access. New threads use the project's defaults. The prompt goes to the CLI on stdin.

## Build another overview

Read `app.t3` in a mod's view. Moonframe starts the provider on demand. Subscribe to `provider/t3` when the mod's update function also needs to hear about changes.

```luau
local sessions = use "t3-sessions"

-- Inside view(state, app)
local data = app.t3
if not data then return ui.text "Connecting…" end
if not data.available then return ui.text(data.error or "Unavailable") end
return sessions { threads = data.threads, folded = state.folded, selected = state.selected }
```

Handle `{ fold = group }` and `{ thread = id }` in the mod. Use `use "t3-thread"` for a single row. The provider polls every 15 seconds, publishes changes, and refreshes immediately on `send "provider:t3" "refresh"`.

`app.t3` contains `available`, optional `error`, `total` and `threads`. Each row carries its ID, title, project, raw T3 status, display group, update timestamp, optional provider, branch and worktree, and lists of PR and issue links with labels, titles and URLs. Waiting means a blocking request, a proposed plan needing action, or a pending guest message. Settled reflects T3's explicit settlement flag. Other active sessions include completed runs still on the active list. Raw status stays visible for failed and interrupted runs.

The provider requests the active v2 shell snapshot, omits archived and deleted threads, and publishes the newest 200 sessions within Moonframe's message-size limit. Each row shows up to four PR and four issue labels. Counts and filters in the example apply to the published rows. The integration uses Moonframe's existing process-provider protocol and requires no WIT changes.

### Write messages

Use `t3-composer` in the authorized mod. It sends the write directly from its button. Its project menu emits `{ project = id }`, the input emits `{ msg = "draft", value = text }`, and the picker and cancel controls emit `t3-picker`, `t3-dismiss`, and `t3-cancel`. The example shell shows how to handle each.

For a custom button, send `{ action, target, text, environmentId, ticket, id }` to `provider:t3`. Use `"start"` with a project ID from `app.t3.projects`, or `"send"` with a thread ID from `app.t3.threads`. Bind the draft to the environment where it was opened. Use the current `app.t3.ticket` and a unique ID for each draft revision. Multiple clicks on the same revision use the same ID. Prompt size is limited to 64 KiB.

Subscribe to `provider/t3` and match `results[id]` on the published value. Its status is `sending`, then `started`, `sent`, `queued`, or `error`. Success includes `threadId`. Errors include a display message. The provider retains the latest 16 receipts and consumes a ticket on each accepted attempt. Explicit retries need a fresh ID and the current ticket. The project picker exposes up to 200 unarchived projects within the snapshot size budget.

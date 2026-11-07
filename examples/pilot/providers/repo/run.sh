#!/usr/bin/env bash
# Finds Node, which a window started from the Dock may not have on its PATH,
# and runs the provider. Without one it stays up and says so: a provider that
# exits is a red pill rather than a next step.
find_node() {
	command -v node && return
	local found
	found=$("${SHELL:-/bin/zsh}" -ilc 'command -v node' 2>/dev/null </dev/null | tail -1)
	[ -x "$found" ] && echo "$found" && return
	local candidate
	for candidate in "$HOME"/.local/share/mise/shims/node /opt/homebrew/bin/node /usr/local/bin/node \
		"$HOME"/.volta/bin/node "$HOME"/.nix-profile/bin/node "$HOME"/.nvm/versions/node/*/bin/node; do
		[ -x "$candidate" ] && echo "$candidate" && return
	done
}

node=$(find_node)
if [ -z "$node" ]; then
	echo '{"available":false,"reason":"Node.js was not found. Install it from nodejs.org, then reopen Pilot."}'
	cat >/dev/null # until stdin closes, as the app expects of a provider that stays
	exit 0
fi
PATH="$(dirname "$node"):$PATH" exec "$node" main.mjs

#!/usr/bin/env bash
#
# Script meant to quickly start up an environment for development.
# More could be added for the wokwi or the sim environment. But for now it just starts a tmux 
# session.
#
# Start (or attach to) the openpour tmux environment.
set -euo pipefail

SESSION="openpour"
ROOT="$HOME/repos/openpour"
CLAUDE_CMD="claude"

usage() {
		cat <<EOF
Usage: $(basename "$0") [options]

Attach to the '$SESSION' tmux session via tms, or build it if it isn't running:
	agent   claude (top) + lazygit (bottom)
	editor  nvim
	shell   zsh

Options:
	-c, --continue   when building, resume the last claude conversation (costs tokens)
	-h, --help       show this help
EOF
}

while (($#)); do
		case "$1" in
				-c|--continue) CLAUDE_CMD="claude --continue" ;;
				-h|--help)     usage; exit 0 ;;
				*)             echo "unknown option: $1" >&2; usage >&2; exit 1 ;;
		esac
		shift
done

# Already running -> just attach; claude keeps running untouched in its pane
if tmux has-session -t "=$SESSION" 2>/dev/null; then
		exec tms open-session "$SESSION"
fi

# Window 1: claude on top, lazygit below
top=$(tmux new-session -d -P -F '#{pane_id}' -s "$SESSION" -c "$ROOT" -n agent)
tmux send-keys -t "$top" "$CLAUDE_CMD" C-m
bottom=$(tmux split-window -v -P -F '#{pane_id}' -t "$top" -c "$ROOT" -l 35%)
tmux send-keys -t "$bottom" "lazygit" C-m
tmux select-pane -t "$top"

# Window 2: neovim
tmux new-window -t "$SESSION" -c "$ROOT" -n editor
tmux send-keys -t "$SESSION:editor" "nvim ." C-m

# Window 3: plain shell
tmux new-window -t "$SESSION" -c "$ROOT" -n shell

tmux select-window -t "$SESSION:agent"
exec tms open-session "$SESSION"


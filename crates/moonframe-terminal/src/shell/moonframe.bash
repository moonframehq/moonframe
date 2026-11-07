# Moonframe's shell integration for bash. Nothing but a startup argument can
# load a file into bash, and the terminal starts the login shell without any,
# so this one is asked for, in ~/.bashrc:
#
#   [ -n "$MOONFRAME_SHELL_INTEGRATION" ] && . "$MOONFRAME_SHELL_INTEGRATION/moonframe.bash"
[[ $- == *i* ]] || return 0

_moonframe_prompt() {
	local code=$? folder=${PWD//%/%25}
	printf '\e]133;D;%s\e\\' "$code"
	printf '\e]7;file://%s%s\e\\' "$HOSTNAME" "${folder// /%20}"
	printf '\e]133;A\e\\'
	return "$code"
}

PROMPT_COMMAND="_moonframe_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
# Printed as a command starts (bash 4.4). It cannot name the command.
PS0+=$'\e]133;C\e\\'

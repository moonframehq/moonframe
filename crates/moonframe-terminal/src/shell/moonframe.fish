# Moonframe's shell integration for fish, loaded from XDG_DATA_DIRS. Takes
# its own folder back out, so programs fish starts see the user's value.
if set --query MOONFRAME_FISH_DATA
    set --local kept (string split : -- "$XDG_DATA_DIRS" | string match --invert --entire -- "$MOONFRAME_FISH_DATA")
    if test (count $kept) -gt 0
        set --global --export XDG_DATA_DIRS (string join : -- $kept)
    else
        set --erase XDG_DATA_DIRS
    end
    set --erase MOONFRAME_FISH_DATA
end

status is-interactive; or exit 0

function _moonframe_prompt --on-event fish_prompt
    printf '\e]7;file://%s%s\e\\' (prompt_hostname) (string escape --style=url -- $PWD)
    printf '\e]133;A\e\\'
end

function _moonframe_preexec --on-event fish_preexec
    printf '\e]133;C;cmdline_url=%s\e\\' (string escape --style=url -- (string sub --length 1024 -- $argv[1]))
end

function _moonframe_postexec --on-event fish_postexec
    printf '\e]133;D;%s\e\\' $status
end

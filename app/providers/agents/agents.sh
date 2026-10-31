#!/usr/bin/env bash
# Prints the coding-agent CLIs running on this machine (claude, codex,
# gemini, ...) as one JSON line every four seconds, and at once when a mod
# sends any message (`to = "provider:agents"`). Exits when the app closes
# its stdin.
#
# It reads `ps`, and each agent's working folder from /proc on Linux or
# `lsof` on macOS. What an agent is doing is not visible from outside, so
# `status` is always "running". Plain bash (3.2 or later); no jq or python.

POLL=4
MAX_AGENTS=50
MAX_COMMAND=200

export LC_ALL=C   # `ps` prints 1.5, not 1,5

bs='\'
dq='"'
jstr() {
    local s=$1
    case $s in
        *[\\\"]*)
            s=${s//"$bs"/"$bs$bs"}
            s=${s//"$dq"/"$bs$dq"}
            ;;
    esac
    if [[ $s == *[[:cntrl:]]* ]]; then
        s=$(printf '%s' "$s" | tr '\000-\037' ' ')
    fi
    J="\"$s\""
}

# The agent a command line is, from the program's name or the package a
# runtime (node, bun, python) was given; sets NAME, or leaves it empty.
agent_name() {
    local word base i=0 words
    NAME=
    read -r -a words <<<"$1"
    for word in "${words[@]}"; do
        i=$((i + 1))
        [ "$i" -gt 3 ] && return
        base=${word##*/}
        base=${base%.js}
        base=${base%.mjs}
        base=${base%.cjs}
        case $word in
            *@anthropic-ai/claude-code*) NAME=claude && return ;;
            *@openai/codex*) NAME=codex && return ;;
            *@google/gemini-cli*) NAME=gemini && return ;;
            *@github/copilot*) NAME=copilot && return ;;
            *@sourcegraph/amp*) NAME=amp && return ;;
            *@qwen-code/*) NAME=qwen && return ;;
            *opencode-ai*) NAME=opencode && return ;;
            *@vibe-kit/grok-cli*) NAME=grok && return ;;
        esac
        case $base in
            claude) NAME=claude ;;
            codex) NAME=codex ;;
            gemini) NAME=gemini ;;
            cursor-agent) NAME=cursor ;;
            opencode) NAME=opencode ;;
            aider) NAME=aider ;;
            amp) NAME=amp ;;
            copilot) NAME=copilot ;;
            droid) NAME=droid ;;
            kiro-cli | kiro) NAME=kiro ;;
            qwen) NAME=qwen ;;
            goose) NAME=goose ;;
            crush) NAME=crush ;;
            auggie) NAME=auggie ;;
            cline) NAME=cline ;;
            kimi) NAME=kimi ;;
            grok) NAME=grok ;;
            # A runtime: the agent is its script, the next word.
            node | bun | deno | python | python3 | python3.* | uv | uvx | npx | pnpm | env) continue ;;
            *) return ;;
        esac
        [ -n "$NAME" ] && return
    done
}

# [[dd-]hh:]mm:ss into seconds.
seconds() {
    local t=$1 days=0 h=0 m=0 s=0
    case $t in *-*) days=${t%%-*} && t=${t#*-} ;; esac
    case $t in
        *:*:*) h=${t%%:*} && t=${t#*:} ;;
    esac
    m=${t%%:*}
    s=${t#*:}
    SECS=$(((10#$days * 86400) + (10#$h * 3600) + (10#$m * 60) + 10#$s))
}

# Working folders of PIDs ($@), as "pid<TAB>cwd" lines in CWDS.
read_cwds() {
    local pid line current=
    CWDS=$'\n'
    [ "$#" -gt 0 ] || return
    if [ -d /proc/self ] && [ -e /proc/self/cwd ]; then
        for pid in "$@"; do
            line=$(readlink "/proc/$pid/cwd" 2>/dev/null) && CWDS+="$pid"$'\t'"$line"$'\n'
        done
        return
    fi
    command -v lsof >/dev/null 2>&1 || return
    local list
    list=$(
        IFS=,
        printf '%s' "$*"
    )
    while IFS= read -r line; do
        case $line in
            p*) current=${line#p} ;;
            n*) CWDS+="$current"$'\t'"${line#n}"$'\n' ;;
        esac
    done < <(lsof -a -d cwd -p "$list" -Fn 2>/dev/null)
}

LAST=
look() {
    local force=$1 now pid ppid etime cpu tty args out='' n=0 cwd
    local pids=() names=() ppids=() etimes=() cpus=() ttys=() commands=()
    now=$(date +%s)
    if ! command -v ps >/dev/null 2>&1; then
        OUT='{"available":false,"reason":"ps is not available","agents":[],"count":0}'
    else
        while read -r pid ppid etime cpu tty args; do
            [ "$pid" = "$$" ] && continue
            agent_name "$args"
            [ -n "$NAME" ] || continue
            pids+=("$pid")
            names+=("$NAME")
            ppids+=("$ppid")
            etimes+=("$etime")
            cpus+=("$cpu")
            ttys+=("$tty")
            commands+=("$args")
        done < <(ps -A -o pid= -o ppid= -o etime= -o pcpu= -o tty= -o args= 2>/dev/null)
        read_cwds "${pids[@]}"
        local i j skip
        for i in "${!pids[@]}"; do
            # A wrapper and the agent it starts (node codex -> codex) are one
            # agent: keep the outermost.
            skip=
            for j in "${!pids[@]}"; do
                if [ "${pids[j]}" = "${ppids[i]}" ] && [ "${names[j]}" = "${names[i]}" ]; then
                    skip=1
                    break
                fi
            done
            [ -n "$skip" ] && continue
            [ "$n" -ge "$MAX_AGENTS" ] && break
            n=$((n + 1))
            seconds "${etimes[i]}"
            cwd=
            case $CWDS in
                *$'\n'"${pids[i]}"$'\t'*)
                    cwd=${CWDS#*$'\n'"${pids[i]}"$'\t'}
                    cwd=${cwd%%$'\n'*}
                    ;;
            esac
            [ -n "$out" ] && out+=,
            out+="{\"pid\":${pids[i]},\"ppid\":${ppids[i]},\"name\":\"${names[i]}\""
            local command=${commands[i]}
            [ "${#command}" -gt "$MAX_COMMAND" ] && command="${command:0:MAX_COMMAND}…"
            jstr "$command" && out+=",\"command\":$J"
            if [ -n "$cwd" ]; then jstr "$cwd"; else J=null; fi
            out+=",\"cwd\":$J"
            case ${ttys[i]} in '?' | '??' | '-' | '') J=null ;; *) jstr "${ttys[i]}" ;; esac
            out+=",\"tty\":$J"
            case ${cpus[i]} in '' | *[!0-9.]*) cpu=0 ;; .*) cpu=0${cpus[i]} ;; *) cpu=${cpus[i]} ;; esac
            out+=",\"started\":$((now - SECS)),\"elapsed_seconds\":$SECS,\"cpu\":$cpu,\"status\":\"running\"}"
        done
        OUT="{\"available\":true,\"agents\":[$out],\"count\":$n}"
    fi
    if [ "$OUT" != "$LAST" ] || [ -n "$force" ]; then
        printf '%s\n' "$OUT"
        LAST=$OUT
    fi
}

force=1
while true; do
    look "$force"
    force=
    # bash 3.2 returns 1 for a timeout and for the end of input alike, so a
    # `read` that failed early means the app closed stdin.
    started=$SECONDS
    if IFS= read -r -t "$POLL" _; then
        force=1
    elif [ "$?" -le 128 ] && [ $((SECONDS - started)) -lt $((POLL - 1)) ]; then
        exit 0
    fi
done

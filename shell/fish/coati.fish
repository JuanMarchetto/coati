# coati.fish — Coati shell integration for fish
# Requires: coati binary on $PATH
# Install: source /path/to/shell/fish/coati.fish  (append to ~/.config/fish/config.fish)

set -g _coati_last_cmd ""
set -g _coati_last_exit 0

function _coati_preexec --on-event fish_preexec
    set -g _coati_last_cmd $argv[1]
end

function _coati_postexec --on-event fish_postexec
    set -g _coati_last_exit $status
end

function _coati_context_json
    set -l branch (git rev-parse --abbrev-ref HEAD 2>/dev/null; or echo "")
    set -l branch_field "null"
    if test -n "$branch"
        set branch_field "\"$branch\""
    end
    printf '{"pwd":"%s","last_command":"%s","last_exit":%d,"git_branch":%s,"shell":"fish"}\n' \
        (string escape --style=json -- $PWD) \
        (string escape --style=json -- $_coati_last_cmd) \
        $_coati_last_exit \
        $branch_field
end

function _coati_jget
    set -l key $argv[1]
    sed -n "s/.*\"$key\":\"\([^\"]*\)\".*/\1/p; s/.*\"$key\":\(true\|false\|null\|-\?[0-9]*\).*/\1/p" | head -1
end

function coati
    switch "$argv[1]"
        case "" -h --help ask serve model hw setup propose explain
            command coati $argv
            return $status
    end
    set -l intent (string join " " $argv)
    set -l ctx (_coati_context_json)
    set -l resp (command coati propose --json --context "$ctx" -- "$intent" 2>/dev/null)
    if test $status -ne 0
        echo "coati: agent unreachable" >&2; return 1
    end
    set -l cmd (printf '%s' $resp | _coati_jget command)
    set -l reasoning (printf '%s' $resp | _coati_jget reasoning)
    set -l needs_sudo (printf '%s' $resp | _coati_jget needs_sudo)

    if test -z "$cmd"
        echo "coati: empty proposal" >&2; return 1
    end
    test "$needs_sudo" = "true"; and echo "⚠ needs sudo" >&2
    echo "\$ $cmd" >&2
    test -n "$reasoning"; and echo "  → $reasoning" >&2

    set -l prompt "Run? [y/N] "
    test "$needs_sudo" = "true"; and set prompt "sudo command — run? [y/N] "
    read -P "$prompt" -n 1 reply
    if test "$reply" = "y" -o "$reply" = "Y"
        eval $cmd
    end
end

function '??'
    test -z "$_coati_last_cmd"; and begin; echo "coati: no previous command captured" >&2; return 1; end
    set -l ctx (_coati_context_json)
    command coati explain --command "$_coati_last_cmd" --exit $_coati_last_exit --stderr "" --stdout "" --context "$ctx"
end

# coati.plugin.zsh — Coati shell integration for zsh
# Requires: coati binary on $PATH
# Install: source /path/to/shell/zsh/coati.plugin.zsh
#         (or drop into ~/.oh-my-zsh/custom/plugins/coati/ and enable in .zshrc)

typeset -g _coati_last_cmd=""
typeset -g _coati_last_exit=0

_coati_preexec() { _coati_last_cmd="$1"; }
_coati_precmd()  { _coati_last_exit=$?; }

autoload -Uz add-zsh-hook
add-zsh-hook preexec _coati_preexec
add-zsh-hook precmd  _coati_precmd

_coati_json_escape() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

_coati_context_json() {
    local branch
    branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    printf '{"pwd":"%s","last_command":"%s","last_exit":%d,"git_branch":%s,"shell":"zsh"}\n' \
        "$(_coati_json_escape "$PWD")" \
        "$(_coati_json_escape "$_coati_last_cmd")" \
        "$_coati_last_exit" \
        "$([[ -n "$branch" ]] && printf '"%s"' "$(_coati_json_escape "$branch")" || printf 'null')"
}

# Tiny JSON getter for flat objects — avoids jq dependency.
_coati_jget() {
    local key="$1"
    sed -n "s/.*\"${key}\":\"\([^\"]*\)\".*/\1/p; s/.*\"${key}\":\(true\|false\|null\|-\?[0-9]*\).*/\1/p" | head -1
}

coati() {
    case "$1" in
        ""|-h|--help|ask|serve|model|hw|setup|propose|explain)
            command coati "$@"
            return $?
            ;;
    esac

    local intent="$*"
    local ctx resp
    ctx="$(_coati_context_json)"
    resp="$(command coati propose --json --context "$ctx" -- "$intent" 2>/dev/null)" || {
        print -u2 "coati: agent unreachable or errored"
        return 1
    }

    local cmd reasoning needs_sudo
    cmd="$(printf '%s' "$resp"       | _coati_jget command)"
    reasoning="$(printf '%s' "$resp" | _coati_jget reasoning)"
    needs_sudo="$(printf '%s' "$resp" | _coati_jget needs_sudo)"

    [[ -z "$cmd" ]] && { print -u2 "coati: empty proposal"; return 1; }
    [[ "$needs_sudo" == "true" ]] && print -u2 "needs sudo"
    print -u2 "\$ $cmd"
    [[ -n "$reasoning" ]] && print -u2 "  -> $reasoning"

    local prompt="Run? [y/N] "
    [[ "$needs_sudo" == "true" ]] && prompt="sudo command -- run? [y/N] "
    local reply
    read -k 1 "reply?$prompt"
    print
    if [[ "$reply" == "y" || "$reply" == "Y" ]]; then
        eval "$cmd"
    fi
}

# ?? -- explain the last command
\?\?() {
    if [[ -z "$_coati_last_cmd" ]]; then
        print -u2 "coati: no previous command captured"
        return 1
    fi
    local ctx
    ctx="$(_coati_context_json)"
    command coati explain \
        --command "$_coati_last_cmd" \
        --exit "$_coati_last_exit" \
        --stderr "" \
        --stdout "" \
        --context "$ctx"
}

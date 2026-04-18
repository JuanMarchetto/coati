# coati.bash — Coati shell integration for bash
# Requires: coati binary on $PATH
# Install: source /path/to/shell/bash/coati.bash  (append to ~/.bashrc)

_coati_last_cmd=""
_coati_last_exit=0

_coati_preexec() {
    # Skip our own helpers and the prompt command itself
    case "$BASH_COMMAND" in
        _coati_*) return ;;
        "$PROMPT_COMMAND"*) return ;;
    esac
    _coati_last_cmd="$BASH_COMMAND"
}
trap '_coati_preexec' DEBUG

_coati_precmd() { _coati_last_exit=$?; }
if [[ "$PROMPT_COMMAND" != *_coati_precmd* ]]; then
    PROMPT_COMMAND="_coati_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
fi

_coati_json_escape() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }

_coati_context_json() {
    local branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null)"
    printf '{"pwd":"%s","last_command":"%s","last_exit":%d,"git_branch":%s,"shell":"bash"}\n' \
        "$(_coati_json_escape "$PWD")" \
        "$(_coati_json_escape "$_coati_last_cmd")" \
        "$_coati_last_exit" \
        "$([[ -n "$branch" ]] && printf '"%s"' "$(_coati_json_escape "$branch")" || printf 'null')"
}

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
    local ctx="$(_coati_context_json)"
    local resp
    resp="$(command coati propose --json --context "$ctx" -- "$intent" 2>/dev/null)" || {
        echo "coati: agent unreachable or errored" >&2
        return 1
    }
    local cmd="$(printf '%s' "$resp" | _coati_jget command)"
    local reasoning="$(printf '%s' "$resp" | _coati_jget reasoning)"
    local needs_sudo="$(printf '%s' "$resp" | _coati_jget needs_sudo)"

    [[ -z "$cmd" ]] && { echo "coati: empty proposal" >&2; return 1; }
    [[ "$needs_sudo" == "true" ]] && echo "needs sudo" >&2
    echo "\$ $cmd" >&2
    [[ -n "$reasoning" ]] && echo "  -> $reasoning" >&2

    local prompt="Run? [y/N] "
    [[ "$needs_sudo" == "true" ]] && prompt="sudo command -- run? [y/N] "
    local reply
    read -n 1 -p "$prompt" reply
    echo
    if [[ "$reply" == "y" || "$reply" == "Y" ]]; then
        eval "$cmd"
    fi
}

# ?? isn't a valid bash function name -- alias a standalone command
_coati_qq() {
    [[ -z "$_coati_last_cmd" ]] && { echo "coati: no previous command captured" >&2; return 1; }
    local ctx="$(_coati_context_json)"
    command coati explain \
        --command "$_coati_last_cmd" \
        --exit "$_coati_last_exit" \
        --stderr "" \
        --stdout "" \
        --context "$ctx"
}
alias '??'=_coati_qq

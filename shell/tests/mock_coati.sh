#!/usr/bin/env bash
# Mock coati binary for shell-plugin tests. Emits canned JSON by intent keyword.
case "$1" in
    propose)
        intent=""
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --) shift; intent="$*"; break ;;
                --json|--context) shift 2 ;;
                propose) shift ;;
                *) shift ;;
            esac
        done
        case "$intent" in
            *nginx*) echo '{"command":"sudo systemctl restart nginx","reasoning":"reload nginx","needs_sudo":true}' ;;
            *disk*)  echo '{"command":"df -h","reasoning":"show disk usage","needs_sudo":false}' ;;
            *echo*)  echo '{"command":"echo hi","reasoning":"say hi","needs_sudo":false}' ;;
            *)       echo '{"command":"echo hi","reasoning":"stub","needs_sudo":false}' ;;
        esac ;;
    explain)
        echo '{"text":"mock explanation","fix":"true"}' ;;
    *) echo "mock coati: unknown subcommand $1" >&2; exit 2 ;;
esac

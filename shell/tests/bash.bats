#!/usr/bin/env bats

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    MOCK_DIR="$(mktemp -d)"
    ln -s "$REPO_ROOT/shell/tests/mock_coati.sh" "$MOCK_DIR/coati"
    export PATH="$MOCK_DIR:$PATH"
    PLUGIN="$REPO_ROOT/shell/bash/coati.bash"
}

teardown() { rm -rf "$MOCK_DIR"; }

@test "bash: context_json has pwd and shell" {
    run bash -c "source '$PLUGIN' && _coati_context_json"
    [ "$status" -eq 0 ]
    [[ "$output" == *'"pwd":'* ]]
    [[ "$output" == *'"shell":"bash"'* ]]
}

@test "bash: coati declines without y (default No)" {
    run bash -c "printf '\n' | bash -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'echo hi'* ]]
    [[ "$output" != *$'\nhi\n'* ]]
}

@test "bash: coati executes on y" {
    run bash -c "printf 'y' | bash -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'hi'* ]]
}

@test "bash: sudo intent shows warning" {
    run bash -c "printf '\n' | bash -c \"source '$PLUGIN' && coati restart nginx\""
    [[ "$output" == *'needs sudo'* ]]
    [[ "$output" == *'sudo systemctl restart nginx'* ]]
}

#!/usr/bin/env bats

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    MOCK_DIR="$(mktemp -d)"
    ln -s "$REPO_ROOT/shell/tests/mock_coati.sh" "$MOCK_DIR/coati"
    export PATH="$MOCK_DIR:$PATH"
    PLUGIN="$REPO_ROOT/shell/zsh/coati.plugin.zsh"
}

teardown() { rm -rf "$MOCK_DIR"; }

@test "zsh: context_json has pwd and shell" {
    run zsh -c "source '$PLUGIN' && _coati_context_json"
    [ "$status" -eq 0 ]
    [[ "$output" == *'"pwd":'* ]]
    [[ "$output" == *'"shell":"zsh"'* ]]
}

@test "zsh: coati declines without y (default No)" {
    run bash -c "printf '\n' | zsh -i -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'echo hi'* ]]
    # If it had executed the echo, output would include a line with just 'hi'
    [[ "$output" != *$'\nhi\n'* ]]
}

@test "zsh: coati executes on y" {
    run bash -c "printf 'y' | zsh -i -c \"source '$PLUGIN' && coati echo something\""
    [ "$status" -eq 0 ]
    [[ "$output" == *'hi'* ]]
}

@test "zsh: sudo intent shows warning" {
    run bash -c "printf '\n' | zsh -i -c \"source '$PLUGIN' && coati restart nginx\""
    [[ "$output" == *'needs sudo'* ]]
    [[ "$output" == *'sudo systemctl restart nginx'* ]]
}

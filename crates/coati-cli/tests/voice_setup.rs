#![cfg(feature = "voice")]

use assert_cmd::Command;

#[test]
fn voice_setup_no_accept_prints_help() {
    let mut cmd = Command::cargo_bin("coati").unwrap();
    cmd.args(["voice", "setup", "--model", "tiny.en"]);
    // No --yes, so it should print a prompt banner and exit 1 for a non-TTY stdin.
    let output = cmd.assert();
    output
        .failure()
        .stdout(predicates::str::contains("Would download"));
}

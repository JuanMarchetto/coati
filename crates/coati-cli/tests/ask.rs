use assert_cmd::Command;

#[test]
fn ask_without_args_shows_usage() {
    Command::cargo_bin("coati")
        .unwrap()
        .arg("ask")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn ask_help_mentions_question() {
    let output = Command::cargo_bin("coati")
        .unwrap()
        .args(["ask", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.to_lowercase().contains("question") || stdout.contains("ask"),
        "expected 'question' or 'ask' in help output, got: {stdout}"
    );
}

// Live ollama test — gated with --ignored
#[test]
#[ignore]
fn ask_returns_a_response_with_live_ollama() {
    use predicates::prelude::*;
    Command::cargo_bin("coati")
        .unwrap()
        .args(["ask", "say hello in one word"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains("hello").or(predicate::str::contains("Hello")));
}

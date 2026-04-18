use assert_cmd::Command;

#[test]
fn explain_help_mentions_required_flags() {
    let out = Command::cargo_bin("coati").unwrap()
        .args(["explain", "--help"])
        .output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--command"));
    assert!(s.contains("--exit"));
    assert!(s.contains("--json"));
}

#[test]
fn explain_rejects_missing_command() {
    Command::cargo_bin("coati").unwrap()
        .args(["explain", "--exit", "1"])
        .assert()
        .failure();
}

use assert_cmd::Command;

#[test]
fn propose_help_mentions_json_flag() {
    let out = Command::cargo_bin("coati").unwrap()
        .args(["propose", "--help"])
        .output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("--json"));
}

#[test]
fn propose_rejects_empty_intent() {
    Command::cargo_bin("coati").unwrap()
        .arg("propose")
        .assert()
        .failure();
}

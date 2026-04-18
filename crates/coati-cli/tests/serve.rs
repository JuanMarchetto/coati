use assert_cmd::cargo::CommandCargoExt;
use std::process::{Command, Stdio};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn serve_responds_to_ping() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let sock = std::env::temp_dir().join(format!("coati-test-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let sock_str = sock.to_str().unwrap().to_owned();

    let mut child = Command::cargo_bin("coati")
        .unwrap()
        .args(["serve", "--socket", &sock_str])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // wait for socket to appear
    for _ in 0..40 {
        if sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(sock.exists(), "socket never appeared");

    let mut stream = UnixStream::connect(&sock).await.unwrap();
    stream.write_all(b"{\"type\":\"ping\"}\n").await.unwrap();

    let mut buf = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("read timed out")
        .unwrap();
    let response = std::str::from_utf8(&buf[..n]).unwrap();
    assert!(
        response.contains("pong"),
        "expected pong, got: {}",
        response
    );

    child.kill().unwrap();
    let _ = std::fs::remove_file(&sock);
}

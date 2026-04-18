use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub fn needs_sudo(cmd: &str) -> bool {
    let t = cmd.trim_start();
    t == "sudo" || t.starts_with("sudo ")
}

#[derive(serde::Serialize)]
pub struct ProcessResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn run_confirmed(cmd: &str) -> anyhow::Result<ProcessResult> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut o) = child.stdout.take() {
        o.read_to_string(&mut stdout).await?;
    }
    if let Some(mut e) = child.stderr.take() {
        e.read_to_string(&mut stderr).await?;
    }
    let status = child.wait().await?;
    Ok(ProcessResult {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sudo_detection() {
        assert!(needs_sudo("sudo systemctl restart nginx"));
        assert!(needs_sudo("sudo"));
        assert!(!needs_sudo("ls -la"));
        assert!(!needs_sudo("sudoify"));
    }

    #[tokio::test]
    async fn runs_a_safe_command() {
        let r = run_confirmed("echo hello").await.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
    }
}

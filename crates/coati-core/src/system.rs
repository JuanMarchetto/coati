use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SystemLogError {
    #[error("invalid unit name: {0}")]
    InvalidUnitName(String),
    #[error("log query failed: {0}")]
    QueryFailed(String),
    #[error("not supported on this platform")]
    Unsupported,
}

#[async_trait]
pub trait SystemLogProvider: Send + Sync {
    /// Fetch recent log lines for a named service unit.
    /// Implementations decide what a "unit" means (systemd unit, launchd job, Windows service, etc.).
    async fn query_unit_logs(&self, unit: &str, lines: u32) -> Result<Vec<String>, SystemLogError>;
}

/// Validate a unit name against shell-safe allowlist: [a-zA-Z0-9@._-]+
pub fn is_valid_unit_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-'))
}

#[cfg(target_os = "linux")]
pub struct LinuxJournalLogProvider;

#[cfg(target_os = "linux")]
#[async_trait]
impl SystemLogProvider for LinuxJournalLogProvider {
    async fn query_unit_logs(&self, unit: &str, lines: u32) -> Result<Vec<String>, SystemLogError> {
        if !is_valid_unit_name(unit) {
            return Err(SystemLogError::InvalidUnitName(unit.to_string()));
        }
        let lines = lines.min(500);

        let out = tokio::process::Command::new("journalctl")
            .args(["-u", unit, "-n", &lines.to_string(), "--no-pager", "--output=short"])
            .output()
            .await
            .map_err(|e| SystemLogError::QueryFailed(format!("journalctl: {e}")))?;

        let body = String::from_utf8_lossy(&out.stdout);
        Ok(body.lines().map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_name_validation() {
        assert!(is_valid_unit_name("nginx.service"));
        assert!(is_valid_unit_name("getty@tty1.service"));
        assert!(is_valid_unit_name("system-logind.service"));
        assert!(!is_valid_unit_name("foo; rm -rf /"));
        assert!(!is_valid_unit_name("foo bar"));
        assert!(!is_valid_unit_name(""));
        assert!(!is_valid_unit_name("foo|bar"));
        assert!(!is_valid_unit_name("$(whoami)"));
    }
}

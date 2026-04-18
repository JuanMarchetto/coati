use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct ShellContext {
    #[serde(default)]
    pub pwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_exit: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub shell: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Ask { question: String },
    Propose {
        intent: String,
        #[serde(default)]
        context: ShellContext,
    },
    Explain {
        command: String,
        #[serde(default)]
        stdout: String,
        #[serde(default)]
        stderr: String,
        exit_code: i32,
        #[serde(default)]
        context: ShellContext,
    },
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Pong,
    Answer { content: String },
    Proposal {
        command: String,
        reasoning: String,
        needs_sudo: bool,
    },
    Explanation {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fix: Option<String>,
    },
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_propose_request() {
        let req = Request::Propose {
            intent: "restart nginx".into(),
            context: ShellContext::default(),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"type\":\"propose\""));
        assert!(s.contains("\"intent\":\"restart nginx\""));
    }

    #[test]
    fn deserializes_proposal_response() {
        let s = r#"{"type":"proposal","command":"sudo systemctl restart nginx","reasoning":"nginx service needs reload","needs_sudo":true}"#;
        let r: Response = serde_json::from_str(s).unwrap();
        match r {
            Response::Proposal { needs_sudo, .. } => assert!(needs_sudo),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn shell_context_round_trips() {
        let ctx = ShellContext {
            pwd: "/home/marche/coati".into(),
            last_command: Some("ls /nonexistent".into()),
            last_exit: Some(2),
            git_branch: Some("main".into()),
            shell: "zsh".into(),
        };
        let s = serde_json::to_string(&ctx).unwrap();
        let parsed: ShellContext = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.pwd, ctx.pwd);
        assert_eq!(parsed.last_exit, Some(2));
    }
}

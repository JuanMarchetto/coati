use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::any::Any;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<LlmToolCall>,
}

#[derive(Debug, Deserialize)]
pub struct LlmToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[async_trait]
pub trait LlmProvider: Send + Sync + Any {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse>;

    fn as_any(&self) -> &dyn Any;
}

pub struct OllamaClient {
    base_url: String,
    model: String,
    http: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            http: reqwest::Client::new(),
        }
    }

    /// Like complete() but forces format: json (or a provided schema) and parses
    /// the assistant content as JSON. Returns the parsed Value.
    pub async fn complete_json(
        &self,
        messages: &[ChatMessage],
        schema: Option<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value> {
        let format = schema.unwrap_or_else(|| serde_json::json!("json"));
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "format": format,
            "stream": false,
        });
        let resp: serde_json::Value = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let content = resp["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let parsed: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("llm returned non-json: {content}"))?;
        Ok(parsed)
    }

    pub async fn complete_stream<F>(
        &self,
        messages: Vec<ChatMessage>,
        mut on_chunk: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str),
    {
        #[derive(serde::Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: &'a [ChatMessage],
            stream: bool,
        }
        #[derive(serde::Deserialize)]
        struct Line {
            message: Option<Msg>,
            done: bool,
        }
        #[derive(serde::Deserialize)]
        struct Msg {
            content: String,
        }

        let url = format!("{}/api/chat", self.base_url);
        let req = Req {
            model: &self.model,
            messages: &messages,
            stream: true,
        };
        let resp = self
            .http
            .post(url)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        let mut full = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            buf.extend_from_slice(&bytes);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line = buf.drain(..=pos).collect::<Vec<u8>>();
                let line = std::str::from_utf8(&line[..line.len() - 1])?.trim();
                if line.is_empty() {
                    continue;
                }
                let parsed: Line = serde_json::from_str(line)?;
                if let Some(m) = parsed.message {
                    if !m.content.is_empty() {
                        on_chunk(&m.content);
                        full.push_str(&m.content);
                    }
                }
                if parsed.done {
                    return Ok(full);
                }
            }
        }
        Ok(full)
    }
}

#[async_trait]
impl LlmProvider for OllamaClient {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[(&'static str, &'static str, serde_json::Value)],
    ) -> anyhow::Result<LlmResponse> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });
        // Only include the `tools` field when there are tools to send.
        // Sending an empty array confuses some models (e.g. gemma3 returns 400).
        if !tools.is_empty() {
            let tools_json: Vec<_> = tools
                .iter()
                .map(|(name, desc, schema)| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": name,
                            "description": desc,
                            "parameters": schema,
                        }
                    })
                })
                .collect();
            body.as_object_mut()
                .unwrap()
                .insert("tools".into(), serde_json::Value::Array(tools_json));
        }

        let resp: serde_json::Value = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let msg = &resp["message"];
        let content = msg["content"].as_str().unwrap_or("").to_string();
        let tool_calls: Vec<LlmToolCall> = msg["tool_calls"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|tc| serde_json::from_value(tc["function"].clone()).ok())
            .collect();

        Ok(LlmResponse {
            content,
            tool_calls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn ollama_complete_returns_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "hi" },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma3".into());
        let msg = ChatMessage {
            role: "user".into(),
            content: "hey".into(),
        };
        let resp = client.complete(&[msg], &[]).await.unwrap();

        assert_eq!(resp.content, "hi");
        assert!(resp.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn omits_tools_field_when_empty() {
        use std::sync::{Arc, Mutex};
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        // Custom matcher that captures the request body for later inspection.
        #[derive(Clone)]
        struct BodyCapture(Arc<Mutex<Option<serde_json::Value>>>);

        impl wiremock::Match for BodyCapture {
            fn matches(&self, req: &Request) -> bool {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
                    *self.0.lock().unwrap() = Some(v);
                }
                true // always match; we assert after the call
            }
        }

        let server = MockServer::start().await;
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let capture_matcher = BodyCapture(Arc::clone(&captured));

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .and(capture_matcher)
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "ok" },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma4".into());
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        };
        let resp = client.complete(&[msg], &[]).await.unwrap();
        assert_eq!(resp.content, "ok");

        let body = captured
            .lock()
            .unwrap()
            .clone()
            .expect("body should have been captured");
        assert!(
            !body.as_object().unwrap().contains_key("tools"),
            "tools key must be absent when tools slice is empty, got: {body}"
        );
    }

    #[tokio::test]
    async fn ollama_complete_json_requests_json_format() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": { "role": "assistant", "content": "{\"k\":\"v\"}" },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma4".into());
        let msg = ChatMessage {
            role: "user".into(),
            content: "return json".into(),
        };
        let val: serde_json::Value = client.complete_json(&[msg], None).await.unwrap();
        assert_eq!(val["k"], "v");
    }

    #[tokio::test]
    async fn ollama_parses_tool_calls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [
                        { "function": { "name": "read_file", "arguments": { "path": "/etc/hosts" } } }
                    ]
                },
                "done": true
            })))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma3".into());
        let resp = client.complete(&[], &[]).await.unwrap();

        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn streams_chunks_until_done() {
        let server = MockServer::start().await;
        let body = concat!(
            r#"{"message":{"role":"assistant","content":"hel"},"done":false}"#,
            "\n",
            r#"{"message":{"role":"assistant","content":"lo"},"done":false}"#,
            "\n",
            r#"{"message":{"role":"assistant","content":""},"done":true}"#,
            "\n",
        );
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = OllamaClient::new(server.uri(), "gemma4".into());
        let msg = ChatMessage {
            role: "user".into(),
            content: "hi".into(),
        };
        let mut chunks = vec![];
        let full = client
            .complete_stream(vec![msg], |c| chunks.push(c.to_string()))
            .await
            .unwrap();

        assert_eq!(chunks, vec!["hel", "lo"]);
        assert_eq!(full, "hello");
    }
}

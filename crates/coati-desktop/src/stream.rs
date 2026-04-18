use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use coati_core::ipc::{Request, Response};
use tauri::{AppHandle, Emitter};

pub async fn send_and_stream(
    socket: &Path,
    app: AppHandle,
    question: String,
    conversation_id: Option<String>,
) -> anyhow::Result<()> {
    let s = UnixStream::connect(socket)?;
    let mut writer = s.try_clone()?;
    let req = Request::AskStream {
        question,
        conversation_id,
    };
    writeln!(writer, "{}", serde_json::to_string(&req)?)?;

    let reader = BufReader::new(s);
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let resp: Response = serde_json::from_str(&line)?;
        match resp {
            Response::Chunk { delta } => {
                let _ = app.emit("coati://chunk", delta);
            }
            Response::StreamEnd { full_content } => {
                let _ = app.emit("coati://end", full_content);
                break;
            }
            Response::Error { message } => {
                let _ = app.emit("coati://error", message);
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chunk_frame() {
        let s = r#"{"type":"chunk","delta":"hi"}"#;
        let r: Response = serde_json::from_str(s).unwrap();
        match r {
            Response::Chunk { delta } => assert_eq!(delta, "hi"),
            _ => panic!(),
        }
    }
}

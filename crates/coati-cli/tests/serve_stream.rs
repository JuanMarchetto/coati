//! Locks the wire format: AskStream request in → stream of Chunk frames → StreamEnd.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use coati_core::ipc::{Request, Response};

fn sock_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("coati-stream-test-{}.sock", std::process::id()));
    p
}

#[test]
fn ask_stream_returns_frames_and_end() {
    let sp = sock_path();
    let _ = std::fs::remove_file(&sp);
    let sp_bg = sp.clone();

    thread::spawn(move || {
        let listener = UnixListener::bind(&sp_bg).unwrap();
        let mut s = listener.incoming().next().unwrap().unwrap();
        let mut reader = BufReader::new(s.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let _req: Request = serde_json::from_str(line.trim()).unwrap();
        let c1 = serde_json::to_string(&Response::Chunk {
            delta: "hel".into(),
        })
        .unwrap();
        let c2 = serde_json::to_string(&Response::Chunk { delta: "lo".into() }).unwrap();
        let end = serde_json::to_string(&Response::StreamEnd {
            full_content: "hello".into(),
        })
        .unwrap();
        writeln!(s, "{c1}").unwrap();
        writeln!(s, "{c2}").unwrap();
        writeln!(s, "{end}").unwrap();
    });

    thread::sleep(Duration::from_millis(50));

    let mut client = UnixStream::connect(&sp).unwrap();
    let req = Request::AskStream {
        question: "hi".into(),
        conversation_id: None,
    };
    writeln!(client, "{}", serde_json::to_string(&req).unwrap()).unwrap();

    let reader = BufReader::new(client);
    let mut deltas = vec![];
    let mut full = None;
    for line in reader.lines() {
        let line = line.unwrap();
        let r: Response = serde_json::from_str(&line).unwrap();
        match r {
            Response::Chunk { delta } => deltas.push(delta),
            Response::StreamEnd { full_content } => {
                full = Some(full_content);
                break;
            }
            _ => panic!("unexpected frame"),
        }
    }

    assert_eq!(deltas, vec!["hel", "lo"]);
    assert_eq!(full.as_deref(), Some("hello"));
    let _ = std::fs::remove_file(&sp);
}

//! Model manifest and downloader.

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size_mb: u32,
}

pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        name: "base.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        size_mb: 148,
    },
    ModelSpec {
        name: "tiny.en",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        size_mb: 75,
    },
];

pub fn lookup(name: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.name == name)
}

pub fn default_models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("coati/models")
}

pub fn model_path(name: &str) -> PathBuf {
    default_models_dir().join(format!("ggml-{}.bin", name))
}

pub fn is_installed(name: &str) -> bool {
    model_path(name).is_file()
}

/// Stream-download a model to `dest`, verifying SHA-256 during the stream.
/// `on_progress(bytes_so_far, total_bytes_opt)` is invoked periodically.
pub async fn download<F>(
    spec: &ModelSpec,
    dest: &Path,
    base_url_override: Option<&str>,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, Option<u64>),
{
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let url = match base_url_override {
        Some(base) => format!("{}/ggml-{}.bin", base.trim_end_matches('/'), spec.name),
        None => spec.url.to_string(),
    };

    let client = reqwest::Client::builder().build()?;
    let resp = client.get(&url).send().await?.error_for_status()?;
    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let tmp = dest.with_extension("partial");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha256::new();
    let mut seen: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        hasher.update(&bytes);
        file.write_all(&bytes).await?;
        seen += bytes.len() as u64;
        on_progress(seen, total);
    }
    file.flush().await?;
    drop(file);

    let got = hex_lower(&hasher.finalize());
    if got != spec.sha256 {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(anyhow!(
            "SHA-256 mismatch for {}: expected {}, got {}",
            spec.name,
            spec.sha256,
            got
        ));
    }

    tokio::fs::rename(&tmp, dest).await?;
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn lookup_known_model() {
        let m = lookup("base.en").expect("base.en should exist");
        assert_eq!(m.name, "base.en");
        assert_eq!(m.sha256.len(), 64);
    }

    #[test]
    fn lookup_unknown_is_none() {
        assert!(lookup("huge.xyz").is_none());
    }

    #[test]
    fn model_path_is_under_data_dir() {
        let p = model_path("base.en");
        assert!(p.ends_with("ggml-base.en.bin"));
        assert!(p.to_string_lossy().contains("coati/models"));
    }

    #[tokio::test]
    async fn download_rejects_sha_mismatch() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not-a-model" as &[u8]))
            .mount(&server)
            .await;

        let spec = ModelSpec {
            name: "test.en",
            url: "unused",
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            size_mb: 0,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = tmp.path().join("ggml-test.en.bin");
        let err = download(&spec, &dest, Some(&server.uri()), |_, _| {})
            .await
            .unwrap_err();
        assert!(err.to_string().contains("SHA-256 mismatch"));
        assert!(
            !dest.exists(),
            "partial file must not be promoted on mismatch"
        );
    }

    #[tokio::test]
    async fn download_success_writes_file_and_reports_progress() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let body: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let mut hasher = Sha256::new();
        hasher.update(&body);
        let sha = hex_lower(&hasher.finalize());
        let sha_static: &'static str = Box::leak(sha.into_boxed_str());

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;

        let spec = ModelSpec {
            name: "test.en",
            url: "unused",
            sha256: sha_static,
            size_mb: 0,
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let dest = tmp.path().join("ggml-test.en.bin");
        let mut saw_progress = false;
        download(&spec, &dest, Some(&server.uri()), |n, _| {
            if n > 0 {
                saw_progress = true;
            }
        })
        .await
        .unwrap();
        assert!(dest.is_file());
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), body);
        assert!(saw_progress);
    }
}

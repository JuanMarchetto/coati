use serde::Deserialize;

#[derive(Deserialize)]
struct TagsResp {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    size: u64,
}

pub async fn list_installed(endpoint: &str) -> anyhow::Result<Vec<(String, u64)>> {
    let url = format!("{endpoint}/api/tags");
    let resp: TagsResp = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.models.into_iter().map(|m| (m.name, m.size)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_tags_response() {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"models":[{"name":"gemma4","size":4000000000}]}"#),
            )
            .mount(&s)
            .await;
        let ms = list_installed(&s.uri()).await.unwrap();
        assert_eq!(ms[0].0, "gemma4");
        assert_eq!(ms[0].1, 4_000_000_000);
    }
}

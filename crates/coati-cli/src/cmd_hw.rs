pub async fn run() -> anyhow::Result<()> {
    crate::cmd_model::recommend_cmd().await
}

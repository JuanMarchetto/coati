use coati_core::Config;
use coati_hw::{detect, recommend};

/// Fetch the list of model names currently installed in the local ollama instance.
/// Returns an empty Vec on any error so callers can gracefully degrade.
async fn list_installed_models(endpoint: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    match client.get(format!("{}/api/tags", endpoint)).send().await {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(v) => v["models"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|m| m["name"].as_str().map(str::to_string))
                .collect(),
            Err(_) => vec![],
        },
        _ => vec![],
    }
}

/// Heuristic: a recommendation is considered "installed" if any installed model name
/// either exactly matches the rec's name, or the rec's base name (the part before ':')
/// is a prefix of the installed model's name. This means `gemma3:latest` matches
/// recommendations for `gemma3:4b`, `gemma3:9b-q4`, etc.
fn is_installed(rec_name: &str, installed: &[String]) -> bool {
    let rec_base = rec_name.split(':').next().unwrap_or(rec_name);
    installed
        .iter()
        .any(|n| n == rec_name || n.starts_with(rec_base))
}

pub async fn list() -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("{}/api/tags", cfg.llm.endpoint))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let empty = vec![];
    for m in resp["models"].as_array().unwrap_or(&empty) {
        println!("{}", m["name"].as_str().unwrap_or("?"));
    }
    Ok(())
}

pub async fn pull(name: &str) -> anyhow::Result<()> {
    let status = std::process::Command::new("ollama")
        .args(["pull", name])
        .status()?;
    if !status.success() {
        anyhow::bail!("ollama pull {} failed", name);
    }
    Ok(())
}

pub fn set(name: &str) -> anyhow::Result<()> {
    let mut cfg = Config::load_or_default()?;
    cfg.llm.model = name.to_string();
    cfg.save()?;
    println!("active model set to: {}", name);
    Ok(())
}

pub async fn recommend_cmd() -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    let installed = list_installed_models(&cfg.llm.endpoint).await;

    let hw = detect();
    println!("Hardware detected:");
    println!(
        "  RAM:  {} GB total, {} GB available",
        hw.ram_total_bytes / 1_073_741_824,
        hw.ram_available_bytes / 1_073_741_824
    );
    println!(
        "  CPU:  {} ({} cores, avx2={}, avx512={})",
        hw.cpu_model, hw.cpu_cores, hw.has_avx2, hw.has_avx512
    );
    if hw.gpus.is_empty() {
        println!("  GPU:  none detected");
    } else {
        for gpu in &hw.gpus {
            println!(
                "  GPU:  {} {} ({} GB VRAM)",
                gpu.vendor,
                gpu.name,
                gpu.vram_bytes / 1_073_741_824
            );
        }
    }
    println!("  Disk: {} GB free (in $HOME)", hw.disk_free_bytes / 1_073_741_824);
    println!();
    println!("Recommended models:");
    for rec in recommend(&hw).iter().take(6) {
        let marker = if rec.fits { "  \u{2605}" } else { "  \u{2717}" };
        let installed_tag = if is_installed(&rec.model, &installed) {
            " [installed] "
        } else {
            "             "
        };
        println!("{} {:24}{}\u{2014} {}", marker, rec.model, installed_tag, rec.reason);
    }
    Ok(())
}

pub async fn benchmark() -> anyhow::Result<()> {
    let cfg = Config::load_or_default()?;
    match coati_hw::benchmark(&cfg.llm.endpoint, &cfg.llm.model).await {
        Ok(r) => {
            println!(
                "{} \u{2014} {:.1} tok/s, {}ms first-token",
                cfg.llm.model, r.tok_per_sec, r.latency_ms
            );
            Ok(())
        }
        Err(e) => {
            println!("benchmark not yet available: {}", e);
            Ok(())
        }
    }
}

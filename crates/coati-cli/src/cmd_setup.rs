use coati_core::Config;
use coati_hw::{detect, recommend};
use inquire::Select;

pub async fn run(
    reconfigure: bool,
    yes: bool,
    model_override: Option<String>,
) -> anyhow::Result<()> {
    let config_path = Config::default_path();
    if config_path.exists() && !reconfigure {
        println!("Config already exists at {}.", config_path.display());
        println!("Use `coati setup --reconfigure` to start over.");
        return Ok(());
    }

    println!("Welcome to coati. Detecting hardware...\n");
    let hw = detect();
    println!(
        "  RAM:  {} GB, CPU: {} ({} cores)",
        hw.ram_total_bytes / 1_073_741_824,
        hw.cpu_model,
        hw.cpu_cores
    );
    for gpu in &hw.gpus {
        println!(
            "  GPU:  {} {} ({} GB VRAM)",
            gpu.vendor,
            gpu.name,
            gpu.vram_bytes / 1_073_741_824
        );
    }
    if hw.gpus.is_empty() {
        println!("  GPU:  none detected");
    }
    println!();

    let recs = recommend(&hw);
    let viable: Vec<_> = recs.into_iter().filter(|r| r.fits).collect();
    if viable.is_empty() {
        anyhow::bail!("no viable local models for this hardware — consider remote inference (documented in README)");
    }

    let chosen = if let Some(name) = model_override {
        name
    } else if yes {
        viable[0].model.clone()
    } else {
        let options: Vec<String> = viable
            .iter()
            .map(|r| format!("{:24} — {}", r.model, r.reason))
            .collect();
        let pick = Select::new("Choose a model:", options).prompt()?;
        // Extract model name — it's the first whitespace-separated token
        pick.split_whitespace()
            .next()
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("failed to parse selection"))?
    };

    println!("\nPulling {} via ollama (this may take a while)...", chosen);
    let status = std::process::Command::new("ollama")
        .args(["pull", &chosen])
        .status()?;
    if !status.success() {
        anyhow::bail!("ollama pull {} failed", chosen);
    }

    let mut cfg = Config::default();
    cfg.llm.model = chosen.clone();
    cfg.save()?;

    println!("\n✓ config written to {}", config_path.display());
    println!("✓ model {} ready", chosen);
    println!("\nTry: coati ask \"what is my disk usage\"");
    Ok(())
}

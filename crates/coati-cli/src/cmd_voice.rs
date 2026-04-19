use anyhow::{anyhow, Result};
use coati_voice::model::{self, MODELS};
use coati_voice::transcribe::Transcriber;
use std::io::{self, BufRead, Write};

pub async fn setup(model_name: &str, yes: bool) -> Result<()> {
    let spec = model::lookup(model_name)
        .ok_or_else(|| anyhow!("unknown model '{}' (try: {})", model_name, known_models()))?;

    let dest = model::model_path(spec.name);
    if dest.is_file() {
        println!(
            "Model {} already installed at {}",
            spec.name,
            dest.display()
        );
        return Ok(());
    }

    println!(
        "Would download {} (~{} MB) from {}",
        spec.name, spec.size_mb, spec.url
    );
    println!("  -> {}", dest.display());
    println!("Audio and transcripts stay local. This download is the only network call.");

    if !yes {
        print!("Proceed? [y/N]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        let stdin = io::stdin();
        let n = stdin.lock().read_line(&mut line).unwrap_or(0);
        let answer = line.trim().to_lowercase();
        if n == 0 || (answer != "y" && answer != "yes") {
            return Err(anyhow!("aborted"));
        }
    }

    let pb_total = std::cell::Cell::new(0u64);
    model::download(spec, &dest, None, |seen, total| {
        if let Some(t) = total {
            if pb_total.get() == 0 {
                pb_total.set(t);
            }
            let pct = (seen as f64 / t as f64) * 100.0;
            print!("\rDownloading: {:>5.1}% ({}/{} bytes)", pct, seen, t);
        } else {
            print!("\rDownloading: {} bytes", seen);
        }
        let _ = io::stdout().flush();
    })
    .await?;
    println!("\nInstalled {} at {}", spec.name, dest.display());
    Ok(())
}

pub async fn transcribe_file(path: &std::path::Path, model_name: &str) -> Result<()> {
    let spec =
        model::lookup(model_name).ok_or_else(|| anyhow!("unknown model '{}'", model_name))?;
    let model_path = model::model_path(spec.name);
    if !model_path.is_file() {
        return Err(anyhow!(
            "model {} is not installed — run `coati voice setup --model {}`",
            spec.name,
            spec.name
        ));
    }
    let samples = load_wav_mono16k(path)?;
    let t = Transcriber::new(&model_path)?;
    let text = t.transcribe(&samples)?;
    println!("{}", text);
    Ok(())
}

fn load_wav_mono16k(path: &std::path::Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 {
        return Err(anyhow!(
            "expected 16kHz WAV, got {} Hz — re-record or ffmpeg-convert",
            spec.sample_rate
        ));
    }
    if spec.channels != 1 {
        return Err(anyhow!("expected mono WAV, got {} channels", spec.channels));
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow!("wav read: {}", e))?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample as f32;
            let scale = 2f32.powf(bits - 1.0);
            reader
                .samples::<i32>()
                .map(|r| r.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|e| anyhow!("wav read: {}", e))?
        }
    };
    Ok(samples)
}

fn known_models() -> String {
    MODELS.iter().map(|m| m.name).collect::<Vec<_>>().join(", ")
}

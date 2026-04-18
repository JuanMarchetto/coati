use crate::detect::HardwareInfo;

#[derive(Debug, Clone)]
pub struct ModelRecommendation {
    pub model: String,
    pub estimated_tok_per_sec: f32,
    pub reason: String,
    pub fits: bool,
}

pub fn recommend(hw: &HardwareInfo) -> Vec<ModelRecommendation> {
    let ram_gb = hw.ram_total_bytes / (1024 * 1024 * 1024);
    let vram_gb = hw.gpus.iter().map(|g| g.vram_bytes).max().unwrap_or(0) / (1024 * 1024 * 1024);

    // (model, ram_need_gb, vram_need_gb, cpu_tps, gpu_tps)
    let candidates: &[(&str, u64, Option<u64>, f32, f32)] = &[
        ("gemma3:4b", 3, None, 10.0, 15.0),
        ("qwen2.5:7b", 5, None, 7.0, 14.0),
        ("gemma3:9b-q4", 6, None, 6.0, 12.0),
        ("qwen2.5:14b-q5", 11, Some(8), 6.0, 30.0),
        ("qwen2.5:32b-q4", 22, Some(16), 3.0, 20.0),
        ("llama3.3:70b-q4", 45, Some(24), 1.5, 18.0),
    ];

    let mut out = Vec::new();
    for (model, ram_need_gb, vram_need_gb, cpu_tps, gpu_tps) in candidates {
        let fits_cpu = ram_gb >= ram_need_gb + 2;
        let fits_gpu = vram_need_gb.map(|v| vram_gb >= v).unwrap_or(false);
        let fits = fits_cpu || fits_gpu;

        let tps = if fits_gpu { *gpu_tps } else { *cpu_tps };
        let reason = if !fits {
            format!(
                "needs {} GB RAM or {} GB VRAM — insufficient",
                ram_need_gb + 2,
                vram_need_gb.unwrap_or(0)
            )
        } else if fits_gpu {
            format!("fits in {} GB VRAM, ~{:.0} tok/s", vram_gb, tps)
        } else {
            format!("CPU only, ~{:.0} tok/s", tps)
        };
        out.push(ModelRecommendation {
            model: model.to_string(),
            estimated_tok_per_sec: tps,
            reason,
            fits,
        });
    }
    // Sort: fitting first, then by tok/s descending
    out.sort_by(|a, b| {
        b.fits.cmp(&a.fits).then(
            b.estimated_tok_per_sec
                .partial_cmp(&a.estimated_tok_per_sec)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::HardwareInfo;

    fn mk(ram_gb: u64, vram_gb: Option<u64>) -> HardwareInfo {
        HardwareInfo {
            ram_total_bytes: ram_gb * 1024 * 1024 * 1024,
            ram_available_bytes: ram_gb * 1024 * 1024 * 1024 * 80 / 100,
            cpu_cores: 8,
            cpu_model: "test".into(),
            has_avx2: true,
            has_avx512: false,
            gpus: vram_gb
                .into_iter()
                .map(|v| crate::detect::GpuInfo {
                    vendor: "NVIDIA".into(),
                    name: "test".into(),
                    vram_bytes: v * 1024 * 1024 * 1024,
                })
                .collect(),
            disk_free_bytes: 100 * 1024 * 1024 * 1024,
        }
    }

    #[test]
    fn recommends_small_model_for_8gb_ram_no_gpu() {
        let recs = recommend(&mk(8, None));
        let top = recs
            .iter()
            .find(|r| r.fits)
            .expect("should have at least one fitting model");
        assert!(
            top.model.contains("4b") || top.model.contains("3b"),
            "expected small model, got: {}",
            top.model
        );
    }

    #[test]
    fn recommends_larger_model_when_gpu_available() {
        let recs = recommend(&mk(16, Some(8)));
        let top = recs
            .iter()
            .find(|r| r.fits)
            .expect("should have at least one fitting model");
        assert!(
            top.model.contains("14b") || top.model.contains("9b") || top.model.contains("7b"),
            "expected gpu-tier model, got: {}",
            top.model
        );
    }

    #[test]
    fn excludes_70b_from_8gb_ram() {
        let recs = recommend(&mk(8, None));
        let fitting: Vec<_> = recs.iter().filter(|r| r.fits).collect();
        assert!(fitting.iter().all(|r| !r.model.contains("70b")));
    }
}

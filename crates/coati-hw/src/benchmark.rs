use anyhow::Result;

#[derive(Debug)]
pub struct BenchmarkResult {
    pub tok_per_sec: f32,
    pub latency_ms: u32,
}

/// Benchmark the configured model by issuing a short completion and measuring throughput.
/// Stub — full implementation is a Phase 1.5 / v1.0 concern.
pub async fn benchmark(_endpoint: &str, _model: &str) -> Result<BenchmarkResult> {
    anyhow::bail!("benchmark not yet implemented — stub in Task 15")
}

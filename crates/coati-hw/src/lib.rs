pub mod detect;
pub mod recommend;
pub mod benchmark;

pub use detect::{HardwareInfo, GpuInfo, detect};
pub use recommend::{ModelRecommendation, recommend};
pub use benchmark::{BenchmarkResult, benchmark};

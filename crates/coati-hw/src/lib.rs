pub mod benchmark;
pub mod detect;
pub mod recommend;

pub use benchmark::{benchmark, BenchmarkResult};
pub use detect::{detect, GpuInfo, HardwareInfo};
pub use recommend::{recommend, ModelRecommendation};

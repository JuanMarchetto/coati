//! Local voice capture + whisper.cpp transcription.

pub mod capture;
pub mod model;
pub mod transcribe;

pub use anyhow::{Error, Result};

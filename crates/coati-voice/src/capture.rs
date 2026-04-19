//! cpal audio capture (push-to-talk).

use anyhow::{anyhow, Result};
use std::sync::mpsc;
use std::thread;

pub struct PushToTalk {
    rx: mpsc::Receiver<Vec<f32>>,
    stream_thread: Option<thread::JoinHandle<()>>,
    stop_tx: Option<mpsc::Sender<()>>,
    input_sample_rate: u32,
    input_channels: u16,
}

impl PushToTalk {
    pub fn start() -> Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?;
        let cfg = device.default_input_config()?;
        let input_sample_rate = cfg.sample_rate().0;
        let input_channels = cfg.channels();

        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let sample_format = cfg.sample_format();
        let stream_cfg: cpal::StreamConfig = cfg.into();

        let stream_thread = thread::spawn(move || {
            let err_fn = |e| tracing::error!("cpal stream error: {}", e);
            let stream_result = match sample_format {
                cpal::SampleFormat::F32 => {
                    let tx_f32 = tx.clone();
                    device.build_input_stream(
                        &stream_cfg,
                        move |data: &[f32], _: &_| {
                            let _ = tx_f32.send(data.to_vec());
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let tx_f32 = tx.clone();
                    device.build_input_stream(
                        &stream_cfg,
                        move |data: &[i16], _: &_| {
                            let buf: Vec<f32> =
                                data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                            let _ = tx_f32.send(buf);
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let tx_f32 = tx.clone();
                    device.build_input_stream(
                        &stream_cfg,
                        move |data: &[u16], _: &_| {
                            let buf: Vec<f32> = data
                                .iter()
                                .map(|s| {
                                    (*s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)
                                })
                                .collect();
                            let _ = tx_f32.send(buf);
                        },
                        err_fn,
                        None,
                    )
                }
                other => {
                    tracing::error!("unsupported sample format: {:?}", other);
                    return;
                }
            };
            let stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("build_input_stream: {}", e);
                    return;
                }
            };
            if let Err(e) = stream.play() {
                tracing::error!("stream.play(): {}", e);
                return;
            }
            let _ = stop_rx.recv();
            drop(stream);
        });

        Ok(Self {
            rx,
            stream_thread: Some(stream_thread),
            stop_tx: Some(stop_tx),
            input_sample_rate,
            input_channels,
        })
    }

    /// Stop capture, return 16kHz mono f32 samples suitable for whisper-rs.
    pub fn finish(mut self) -> Vec<f32> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.stream_thread.take() {
            let _ = t.join();
        }
        let mut raw = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            raw.extend(chunk);
        }
        to_mono_16k(&raw, self.input_sample_rate, self.input_channels)
    }
}

/// Downmix to mono and linearly resample to 16kHz.
pub fn to_mono_16k(samples: &[f32], input_rate: u32, channels: u16) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mono: Vec<f32> = if channels <= 1 {
        samples.to_vec()
    } else {
        samples
            .chunks(channels as usize)
            .map(|c| c.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    if input_rate == 16_000 {
        return mono;
    }
    let target = 16_000f64;
    let ratio = target / input_rate as f64;
    let out_len = (mono.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let lo = src.floor() as usize;
        let hi = (lo + 1).min(mono.len() - 1);
        let frac = src - lo as f64;
        let s = mono[lo] as f64 * (1.0 - frac) + mono[hi] as f64 * frac;
        out.push(s as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_stereo_to_16k_mono_roughly_third_length() {
        let stereo: Vec<f32> = (0..4800).flat_map(|i| [i as f32, i as f32]).collect();
        let out = to_mono_16k(&stereo, 48_000, 2);
        assert!((out.len() as i64 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn resample_16k_mono_is_passthrough() {
        let input: Vec<f32> = (0..1600).map(|i| i as f32).collect();
        let out = to_mono_16k(&input, 16_000, 1);
        assert_eq!(out.len(), input.len());
        assert_eq!(out[500], input[500]);
    }

    #[test]
    fn empty_is_empty() {
        assert!(to_mono_16k(&[], 48_000, 2).is_empty());
    }
}

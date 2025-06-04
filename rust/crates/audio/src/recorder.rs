use anyhow::Result;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SizedSample,
};
use hound::{WavSpec, WavWriter};
use std::{
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub type SampleType = i16;

/// A simple, reliable audio recorder that uses CPAL directly
/// Based on the working record.rs example
pub struct SimpleRecorder {
    is_recording: Arc<AtomicBool>,
    output_path: Option<PathBuf>,
    writer: Option<Arc<Mutex<WavWriter<BufWriter<File>>>>>,
    stream: Option<cpal::Stream>,
}

impl SimpleRecorder {
    /// Create a new recorder
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            output_path: None,
            writer: None,
            stream: None,
        }
    }

    /// Start recording to a file
    pub fn start_recording<P, F>(
        &mut self,
        device_id: Option<&str>,
        output_path: P,
        on_peak: F,
    ) -> Result<()>
    where
        P: Into<PathBuf>,
        F: Fn(SampleType) + Send + 'static,
    {
        if self.is_recording.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Already recording"));
        }

        let output_path = output_path.into();

        // Get audio device
        let host = cpal::default_host();
        let device = if let Some(id) = device_id {
            self.find_device_by_name(&host, id)?
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow::anyhow!("No default input device available"))?
        };

        // Get device configuration
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();

        // Validate sample rate
        if sample_rate < 8000 || sample_rate > 192000 {
            return Err(anyhow::anyhow!(
                "Unusual sample rate: {} Hz. Expected range: 8000-192000 Hz",
                sample_rate
            ));
        }

        // Create WAV writer
        let wav_spec = WavSpec {
            channels: 1, // Always output mono
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = WavWriter::create(&output_path, wav_spec)?;
        let writer = Arc::new(Mutex::new(writer));
        self.writer = Some(writer.clone());
        self.output_path = Some(output_path);

        // Build and start stream
        let stream = match sample_format {
            cpal::SampleFormat::I8 => {
                self.build_input_stream::<i8, _>(&device, &config, writer, channels, on_peak)?
            }
            cpal::SampleFormat::I16 => {
                self.build_input_stream::<i16, _>(&device, &config, writer, channels, on_peak)?
            }
            cpal::SampleFormat::I32 => {
                self.build_input_stream::<i32, _>(&device, &config, writer, channels, on_peak)?
            }
            cpal::SampleFormat::F32 => {
                self.build_input_stream::<f32, _>(&device, &config, writer, channels, on_peak)?
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", sample_format));
            }
        };

        stream.play()?;
        self.stream = Some(stream);
        self.is_recording.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Stop recording and finalize the file
    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        if !self.is_recording.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Not currently recording"));
        }

        // Stop recording
        self.is_recording.store(false, Ordering::SeqCst);

        // Drop the stream
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // Finalize WAV file
        if let Some(writer) = self.writer.take() {
            if let Ok(writer) = Arc::try_unwrap(writer) {
                if let Ok(writer) = writer.into_inner() {
                    writer.finalize()?;
                }
            }
        }

        let output_path = self.output_path.take()
            .ok_or_else(|| anyhow::anyhow!("No output path set"))?;

        Ok(output_path)
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    fn build_input_stream<T, F>(
        &self,
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        writer: Arc<Mutex<WavWriter<BufWriter<File>>>>,
        channels: u16,
        on_peak: F,
    ) -> Result<cpal::Stream>
    where
        T: Sample + SizedSample + Send + 'static,
        SampleType: FromSample<T>,
        F: Fn(SampleType) + Send + 'static,
    {
        let is_recording = self.is_recording.clone();

        let stream = device.build_input_stream(
            &config.config(),
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if !is_recording.load(Ordering::SeqCst) {
                    return;
                }

                // Convert to i16 and handle multiple channels
                let samples: Vec<SampleType> = if channels == 1 {
                    // Mono: direct conversion
                    data.iter()
                        .map(|&sample| SampleType::from_sample(sample))
                        .collect()
                } else {
                    // Multi-channel: convert to mono by averaging channels
                    data.chunks_exact(channels as usize)
                        .map(|frame| {
                            // Convert to i16 first, then average
                            let sum: i32 = frame
                                .iter()
                                .map(|&sample| SampleType::from_sample(sample) as i32)
                                .sum();
                            let avg = sum / channels as i32;
                            avg.clamp(SampleType::MIN as i32, SampleType::MAX as i32) as SampleType
                        })
                        .collect()
                };

                // Find peak for callback
                if let Some(&peak) = samples.iter().max_by_key(|&&x| x.abs()) {
                    on_peak(peak);
                }

                // Write to WAV file
                if let Ok(mut writer) = writer.lock() {
                    for sample in samples {
                        if let Err(e) = writer.write_sample(sample) {
                            eprintln!("❌ Error writing sample: {}", e);
                            is_recording.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
            },
            |err| {
                eprintln!("❌ Stream error: {}", err);
            },
            None,
        )?;

        Ok(stream)
    }

    fn find_device_by_name(&self, host: &cpal::Host, name_or_id: &str) -> Result<cpal::Device> {
        let devices: Vec<_> = host.input_devices()?.collect();

        // First try exact name match
        for device in &devices {
            if let Ok(device_name) = device.name() {
                if device_name == name_or_id {
                    return Ok(device.clone());
                }
            }
        }

        // Then try partial name match (case insensitive)
        for device in &devices {
            if let Ok(device_name) = device.name() {
                if device_name.to_lowercase().contains(&name_or_id.to_lowercase()) {
                    return Ok(device.clone());
                }
            }
        }

        Err(anyhow::anyhow!(
            "Device '{}' not found. Use get_microphones() to see available devices.",
            name_or_id
        ))
    }
}

impl Drop for SimpleRecorder {
    fn drop(&mut self) {
        if self.is_recording() {
            let _ = self.stop_recording();
        }
    }
}


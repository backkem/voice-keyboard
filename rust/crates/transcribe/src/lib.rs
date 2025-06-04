use anyhow::Result;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use std::path::Path;

pub struct Transcriber {
    context: WhisperContext,
}

impl Transcriber {
    /// Create a new transcriber with the specified model path
    pub fn new<P: AsRef<Path>>(model_path: P) -> Result<Self> {
        let model_path_str = model_path.as_ref().to_str()
            .ok_or_else(|| anyhow::anyhow!("Model path contains invalid UTF-8"))?;
        
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(model_path_str, params)
            .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {}", e))?;
        
        Ok(Self { context: ctx })
    }

    /// Create a new transcriber with custom parameters
    pub fn new_with_params<P: AsRef<Path>>(
        model_path: P,
        params: WhisperContextParameters,
    ) -> Result<Self> {
        let model_path_str = model_path.as_ref().to_str()
            .ok_or_else(|| anyhow::anyhow!("Model path contains invalid UTF-8"))?;
        
        let ctx = WhisperContext::new_with_params(model_path_str, params)
            .map_err(|e| anyhow::anyhow!("Failed to load Whisper model with params: {}", e))?;
        
        Ok(Self { context: ctx })
    }

    /// Transcribe audio samples (f32, 16kHz)
    pub fn transcribe(&self, audio: &[f32]) -> Result<String> {
        let mut state = self.context.create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create model state: {}", e))?;

        // Configure transcription parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(num_cpus::get() as i32);
        params.set_translate(false); // Don't translate, just transcribe
        params.set_language(Some("en"));
        params.set_token_timestamps(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Run transcription
        state.full(params, audio)
            .map_err(|e| anyhow::anyhow!("Failed to run transcription: {}", e))?;

        // Extract transcribed text
        let num_segments = state.full_n_segments()
            .map_err(|e| anyhow::anyhow!("Failed to get segment count: {}", e))?;

        let mut result = String::new();
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment {}: {}", i, e))?;
            result.push_str(&segment);
        }

        Ok(result.trim().to_string())
    }

    /// Transcribe from a WAV file
    pub fn transcribe_from_wav<P: AsRef<Path>>(&self, wav_path: P) -> Result<String> {
        let audio = load_wav_as_float(wav_path)?;
        self.transcribe(&audio)
    }
}

/// Load a WAV file and convert to f32 audio samples
pub fn load_wav_as_float<P: AsRef<Path>>(path: P) -> Result<Vec<f32>> {
    let reader = hound::WavReader::open(&path)
        .map_err(|e| anyhow::anyhow!("Failed to open WAV file: {}", e))?;
    
    let spec = reader.spec();
    
    // Ensure it's 16kHz for Whisper
    if spec.sample_rate != 16000 {
        return Err(anyhow::anyhow!(
            "Audio must be 16kHz, got {}Hz. Use the resample tool first.",
            spec.sample_rate
        ));
    }

    // Read samples based on bit depth
    let audio = match spec.bits_per_sample {
        16 => {
            let samples: Vec<i16> = reader
                .into_samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("Failed to read i16 samples: {}", e))?;
            
            // Convert i16 to f32
            let mut audio = vec![0.0f32; samples.len()];
            whisper_rs::convert_integer_to_float_audio(&samples, &mut audio)
                .map_err(|e| anyhow::anyhow!("Failed to convert to float audio: {}", e))?;
            audio
        }
        32 => {
            // Assume f32 samples
            reader
                .into_samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("Failed to read f32 samples: {}", e))?
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported bit depth: {}. Only 16-bit and 32-bit are supported.",
                spec.bits_per_sample
            ));
        }
    };

    // Convert stereo to mono if necessary
    let mono_audio = if spec.channels == 2 {
        // Average left and right channels
        audio
            .chunks_exact(2)
            .map(|chunk| (chunk[0] + chunk[1]) / 2.0)
            .collect()
    } else if spec.channels == 1 {
        audio
    } else {
        return Err(anyhow::anyhow!(
            "Unsupported channel count: {}. Only mono and stereo are supported.",
            spec.channels
        ));
    };

    Ok(mono_audio)
}
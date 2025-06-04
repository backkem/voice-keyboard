use anyhow::Result;
use hound::{WavReader, WavSpec, WavWriter};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::path::Path;

/// Resample a WAV file to a new sample rate and channel count
pub fn resample_wav_file<P: AsRef<Path>>(
    input_path: P,
    output_path: P,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<()> {
    // Read input WAV file
    let mut reader = WavReader::open(&input_path)?;
    let input_spec = reader.spec();

    println!(
        "Input: {} Hz, {} channels, {} bits",
        input_spec.sample_rate, input_spec.channels, input_spec.bits_per_sample
    );

    // Read all samples as f32 for processing
    let input_samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|sample| sample as f32 / 32768.0))
        .collect::<Result<Vec<_>, _>>()?;

    // Convert to channel-separated format for resampling
    let input_channels = input_spec.channels as usize;
    let _num_frames = input_samples.len() / input_channels;

    // Separate channels
    let mut channel_data: Vec<Vec<f32>> = vec![Vec::new(); input_channels];
    for (i, &sample) in input_samples.iter().enumerate() {
        let channel = i % input_channels;
        channel_data[channel].push(sample);
    }

    // Resample each channel
    let mut resampled_channels = Vec::new();
    if input_spec.sample_rate != target_sample_rate {
        println!(
            "Resampling from {} Hz to {} Hz",
            input_spec.sample_rate, target_sample_rate
        );

        for channel in &channel_data {
            let resampled = resample_channel(channel, input_spec.sample_rate, target_sample_rate)?;
            resampled_channels.push(resampled);
        }
    } else {
        resampled_channels = channel_data;
    }

    // Handle channel conversion
    let final_channels = if input_channels != target_channels as usize {
        println!(
            "Converting from {} to {} channels",
            input_channels, target_channels
        );
        convert_channels(resampled_channels, target_channels as usize)
    } else {
        resampled_channels
    };

    // Interleave channels back together
    let output_frames = final_channels[0].len();
    let mut output_samples = Vec::with_capacity(output_frames * target_channels as usize);

    for frame in 0..output_frames {
        for channel in 0..target_channels as usize {
            output_samples.push(final_channels[channel][frame]);
        }
    }

    // Convert back to i16 and write output
    let output_spec = WavSpec {
        channels: target_channels,
        sample_rate: target_sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(&output_path, output_spec)?;
    for &sample in &output_samples {
        let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;

    println!(
        "Output: {} Hz, {} channels, {} frames",
        target_sample_rate, target_channels, output_frames
    );

    Ok(())
}

fn resample_channel(input: &[f32], input_rate: u32, output_rate: u32) -> Result<Vec<f32>> {
    if input_rate == output_rate {
        return Ok(input.to_vec());
    }

    // Calculate resampling parameters
    let ratio = output_rate as f64 / input_rate as f64;
    let _output_len = (input.len() as f64 * ratio) as usize;

    // Create resampler
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        ratio,
        1.2, // Max allowed ratio change
        params,
        input.len(),
        1, // Single channel
    )?;

    // Perform resampling
    let input_vec = vec![input.to_vec()];
    let output_vec = resampler.process(&input_vec, None)?;

    Ok(output_vec[0].clone())
}

fn convert_channels(input_channels: Vec<Vec<f32>>, target_channels: usize) -> Vec<Vec<f32>> {
    let input_count = input_channels.len();
    let frame_count = input_channels[0].len();

    match (input_count, target_channels) {
        // Stereo to mono: average left and right channels
        (2, 1) => {
            let mut mono = Vec::with_capacity(frame_count);
            for i in 0..frame_count {
                let avg = (input_channels[0][i] + input_channels[1][i]) / 2.0;
                mono.push(avg);
            }
            vec![mono]
        }
        // Mono to stereo: duplicate mono channel
        (1, 2) => {
            let mono = &input_channels[0];
            vec![mono.clone(), mono.clone()]
        }
        // Multi-channel to mono: average all channels
        (n, 1) if n > 2 => {
            let mut mono = Vec::with_capacity(frame_count);
            for i in 0..frame_count {
                let sum: f32 = input_channels.iter().map(|ch| ch[i]).sum();
                mono.push(sum / n as f32);
            }
            vec![mono]
        }
        // Same channel count or unsupported conversion: return as-is
        _ => input_channels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_conversion() {
        // Test stereo to mono
        let stereo = vec![
            vec![1.0, 0.5, -0.5], // Left channel
            vec![-1.0, 0.5, 0.5], // Right channel
        ];
        let mono = convert_channels(stereo, 1);
        assert_eq!(mono.len(), 1);
        assert_eq!(mono[0], vec![0.0, 0.5, 0.0]); // Averaged

        // Test mono to stereo
        let mono = vec![vec![1.0, 0.5, -0.5]];
        let stereo = convert_channels(mono, 2);
        assert_eq!(stereo.len(), 2);
        assert_eq!(stereo[0], stereo[1]); // Both channels identical
    }
}

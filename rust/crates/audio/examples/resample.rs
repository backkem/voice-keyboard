use audio::resample::resample_wav_file;
use clap::{Arg, Command};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let matches = Command::new("Audio Resampler")
        .version("0.1.0")
        .about("Resample WAV files to different sample rates and channel counts")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("INPUT_FILE")
                .help("Input WAV file")
                .required(true),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("OUTPUT_FILE")
                .help("Output WAV file")
                .required(true),
        )
        .arg(
            Arg::new("rate")
                .short('r')
                .long("rate")
                .value_name("SAMPLE_RATE")
                .help("Target sample rate in Hz")
                .default_value("16000"),
        )
        .arg(
            Arg::new("channels")
                .short('c')
                .long("channels")
                .value_name("CHANNELS")
                .help("Target number of channels (1=mono, 2=stereo)")
                .default_value("1"),
        )
        .get_matches();

    let input_path = PathBuf::from(matches.get_one::<String>("input").unwrap());
    let output_path = PathBuf::from(matches.get_one::<String>("output").unwrap());
    
    let sample_rate: u32 = matches
        .get_one::<String>("rate")
        .unwrap()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid sample rate"))?;
    
    let channels: u16 = matches
        .get_one::<String>("channels")
        .unwrap()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid channel count"))?;

    if channels == 0 || channels > 8 {
        return Err(anyhow::anyhow!("Channel count must be between 1 and 8"));
    }

    if sample_rate < 8000 || sample_rate > 192000 {
        return Err(anyhow::anyhow!("Sample rate must be between 8000 and 192000 Hz"));
    }

    if !input_path.exists() {
        return Err(anyhow::anyhow!("Input file does not exist: {:?}", input_path));
    }

    println!(
        "Resampling {} -> {} ({}Hz, {} channels)",
        input_path.display(),
        output_path.display(),
        sample_rate,
        channels
    );

    resample_wav_file(input_path, output_path, sample_rate, channels)?;
    
    println!("✅ Resampling completed successfully!");
    
    Ok(())
}
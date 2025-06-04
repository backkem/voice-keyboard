use anyhow::Result;
use audio::{get_microphones, SimpleRecorder};
use clap::{Arg, Command};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// Simple audio recording example using the audio library's SimpleRecorder
/// 
/// This example demonstrates:
/// 1. Using the audio library instead of direct CPAL
/// 2. Proper device enumeration 
/// 3. Peak monitoring during recording
/// 4. Graceful shutdown with Ctrl+C

fn main() -> Result<()> {
    let matches = Command::new("Simple Audio Recorder")
        .version("0.1.0")
        .about("Simple audio recording example using the audio library")
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output WAV file path")
                .default_value("recording.wav"),
        )
        .arg(
            Arg::new("device")
                .short('d')
                .long("device")
                .value_name("DEVICE")
                .help("Audio device name (partial match supported)"),
        )
        .arg(
            Arg::new("duration")
                .short('t')
                .long("time")
                .value_name("SECONDS")
                .help("Recording duration in seconds")
                .default_value("5"),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List available audio devices")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Handle device listing
    if matches.get_flag("list") {
        return list_audio_devices();
    }

    // Parse arguments
    let output_path = PathBuf::from(matches.get_one::<String>("output").unwrap());
    let device_name = matches.get_one::<String>("device");
    let duration: u64 = matches
        .get_one::<String>("duration")
        .unwrap()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid duration value"))?;

    if duration == 0 || duration > 300 {
        return Err(anyhow::anyhow!("Duration must be between 1 and 300 seconds"));
    }

    // Set up recording
    println!("🎤 Initializing audio recorder...");
    println!("📁 Output file: {:?}", output_path);
    println!("🎯 Recording for {} seconds...", duration);

    let mut recorder = SimpleRecorder::new();
    let should_stop = Arc::new(AtomicBool::new(false));

    // Set up Ctrl+C handler
    let should_stop_clone = should_stop.clone();
    ctrlc::set_handler(move || {
        println!("\n⏹️  Stopping recording...");
        should_stop_clone.store(true, Ordering::SeqCst);
    })?;

    // Start recording with peak monitoring
    let device_id = device_name.map(|s| s.as_str());
    recorder.start_recording(device_id, &output_path, |peak| {
        // Show audio level bar
        let bar_length = (peak.abs() as usize) / 1640; // Scale for display
        let bar = "█".repeat(bar_length.min(20));
        print!("\r🔊 Level: [{:<20}] {:5}", bar, peak);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
    })?;

    println!("🔴 Recording started! Press Ctrl+C to stop early.");

    // Record for specified duration or until stopped
    let start_time = std::time::Instant::now();
    while start_time.elapsed() < Duration::from_secs(duration) {
        if should_stop.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Stop recording
    let final_path = recorder.stop_recording()?;
    println!("\n✅ Recording saved to: {:?}", final_path);

    // Show file information
    show_file_info(&final_path)?;

    Ok(())
}

fn list_audio_devices() -> Result<()> {
    println!("🎤 Available audio input devices:");
    
    let microphones_json = get_microphones()?;
    let microphones: Vec<serde_json::Value> = serde_json::from_str(&microphones_json)?;
    
    if microphones.is_empty() {
        println!("   No input devices found.");
    } else {
        for (i, mic) in microphones.iter().enumerate() {
            println!(
                "   {}. {} (ID: {})", 
                i + 1,
                mic["name"].as_str().unwrap_or("Unknown"),
                mic["id"].as_str().unwrap_or("unknown")
            );
        }
    }
    
    Ok(())
}

fn show_file_info(path: &PathBuf) -> Result<()> {
    if let Ok(reader) = hound::WavReader::open(path) {
        let spec = reader.spec();
        let duration = reader.len() as f64 / (spec.sample_rate as f64 * spec.channels as f64);

        println!("📊 Recording info:");
        println!("   Duration: {:.2} seconds", duration);
        println!("   Sample rate: {} Hz", spec.sample_rate);
        println!("   Channels: {}", spec.channels);
        println!("   Bits per sample: {}", spec.bits_per_sample);
        println!("   Total samples: {}", reader.len());

        if let Ok(metadata) = std::fs::metadata(path) {
            println!("   File size: {} bytes", metadata.len());
        }
    }

    Ok(())
}
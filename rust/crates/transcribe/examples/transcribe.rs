use transcribe::{Transcriber, load_wav_as_float};
use clap::{Arg, Command};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = Command::new("Whisper Transcriber")
        .version("0.1.0")
        .about("Transcribe audio files using Whisper")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("INPUT_FILE")
                .help("Input WAV file (must be 16kHz)")
                .required(true),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .value_name("MODEL_PATH")
                .help("Path to Whisper model file (.bin)")
                .default_value("../../../models/ggml-tiny.en.bin"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let input_path = PathBuf::from(matches.get_one::<String>("input").unwrap());
    let model_path = PathBuf::from(matches.get_one::<String>("model").unwrap());
    let verbose = matches.get_flag("verbose");

    if !input_path.exists() {
        return Err(anyhow::anyhow!("Input file does not exist: {:?}", input_path));
    }

    if !model_path.exists() {
        return Err(anyhow::anyhow!(
            "Model file does not exist: {:?}\n\
            Download a model from: https://huggingface.co/ggerganov/whisper.cpp/tree/main",
            model_path
        ));
    }

    if verbose {
        println!("Loading Whisper model from: {:?}", model_path);
    }

    // Create transcriber
    let transcriber = Transcriber::new(&model_path)?;

    if verbose {
        println!("Loading audio from: {:?}", input_path);
    }

    // Load and validate audio
    let audio = load_wav_as_float(&input_path)?;
    
    if verbose {
        println!("Audio loaded: {} samples ({:.2} seconds)", 
                 audio.len(), 
                 audio.len() as f32 / 16000.0);
        println!("Transcribing...");
    }

    // Transcribe
    let start = std::time::Instant::now();
    let text = transcriber.transcribe(&audio)?;
    let duration = start.elapsed();

    if verbose {
        println!("Transcription completed in {:.2}s", duration.as_secs_f32());
        println!("---");
    }

    // Output the transcribed text
    println!("{}", text);

    Ok(())
}
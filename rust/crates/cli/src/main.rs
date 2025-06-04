use anyhow::Result;
use audio::{resample::resample_wav_file, SimpleRecorder};
use enigo::{Enigo, Keyboard, Settings};
use keyctl::{listen, Key};
use std::{
    env,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use transcribe::{load_wav_as_float, Transcriber};

// Configuration constants
const MODEL_NAME: &str = "ggml-base.en.bin";

fn main() -> Result<()> {
    println!("🎤 Voice Keyboard CLI");
    println!("Press and hold Quote key to record audio...");

    // Model path based on build type
    let model_path = if cfg!(debug_assertions) {
        // Debug build: use repo models directory
        PathBuf::from("../../models").join(MODEL_NAME)
    } else {
        // Release build: use executable directory
        let exe_dir = env::current_exe()?.parent().unwrap().to_path_buf();
        exe_dir.join("whisper-cpp").join(MODEL_NAME)
    };

    if !model_path.exists() {
        return Err(anyhow::anyhow!(
            "Model file not found: {:?}\n\
            Please ensure the Whisper model is available.",
            model_path
        ));
    }

    // Initialize transcriber
    println!("📚 Loading Whisper model...");
    let transcriber = Transcriber::new(&model_path)?;
    println!("✅ Model loaded successfully");

    // Create shared state
    let is_recording = Arc::new(AtomicBool::new(false));
    let recorder = Arc::new(Mutex::new(SimpleRecorder::new()));
    let enigo = Arc::new(Mutex::new(
        Enigo::new(&Settings::default()).expect("Failed to create Enigo instance"),
    ));

    let recording_start_time = Arc::new(Mutex::new(None::<Instant>));

    // Clone references for the callback
    let is_recording_clone = Arc::clone(&is_recording);
    let recorder_clone = Arc::clone(&recorder);
    let enigo_clone = Arc::clone(&enigo);
    let transcriber = Arc::new(transcriber);
    let transcriber_clone = Arc::clone(&transcriber);
    let recording_start_clone = Arc::clone(&recording_start_time);

    if let Err(error) = listen(Key::Quote, true, move |is_pressed| {
        if is_pressed {
            // Key pressed - start recording
            if !is_recording_clone.load(Ordering::SeqCst) {
                println!("🔴 Recording started...");
                is_recording_clone.store(true, Ordering::SeqCst);

                // Record start time
                if let Ok(mut start_time) = recording_start_clone.lock() {
                    *start_time = Some(Instant::now());
                }

                // Start recording
                if let Ok(mut recorder) = recorder_clone.lock() {
                    let temp_path = PathBuf::from("temp_recording.wav");
                    if let Err(e) = recorder.start_recording(None, &temp_path, |peak| {
                        // Optional: Show audio level during recording
                        let bar_length = (peak.abs() as usize) / 3280; // Scale for display
                        let bar = "█".repeat(bar_length.min(10));
                        print!("\r🔊 [{:<10}]", bar);
                        std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    }) {
                        eprintln!("Failed to start recording: {}", e);
                        is_recording_clone.store(false, Ordering::SeqCst);
                    }
                } else {
                    eprintln!("Failed to acquire recorder lock");
                    is_recording_clone.store(false, Ordering::SeqCst);
                }
            }
        } else {
            // Key released - stop recording and transcribe
            if is_recording_clone.load(Ordering::SeqCst) {
                is_recording_clone.store(false, Ordering::SeqCst);

                // Check recording duration
                let recording_duration = if let Ok(start_time) = recording_start_clone.lock() {
                    start_time.map(|t| t.elapsed())
                } else {
                    None
                };

                println!("\n⏹️  Recording stopped");

                // Stop recording
                let audio_path = if let Ok(mut recorder) = recorder_clone.lock() {
                    match recorder.stop_recording() {
                        Ok(path) => Some(path),
                        Err(e) => {
                            eprintln!("Failed to stop recording: {}", e);
                            None
                        }
                    }
                } else {
                    eprintln!("Failed to acquire recorder lock");
                    None
                };

                if let Some(path) = audio_path {
                    // Check if recording is too short (minimum 100ms)
                    if let Some(duration) = recording_duration {
                        if duration < Duration::from_millis(100) {
                            println!("⚠️  Recording too short, skipping transcription");
                            if let Err(e) = std::fs::remove_file(&path) {
                                eprintln!("Failed to clean up temp file: {}", e);
                            }
                            return;
                        }
                    }

                    println!("🔍 Processing audio...");

                    // Create resampled file path
                    let resampled_path = PathBuf::from("temp_recording_16khz.wav");

                    // Resample to 16kHz mono for Whisper
                    match resample_wav_file(&path, &resampled_path, 16000, 1) {
                        Ok(_) => {
                            println!("🔄 Audio resampled to 16kHz");

                            // Load and transcribe resampled audio
                            match load_wav_as_float(&resampled_path) {
                                Ok(mut audio) => {
                                    // Pad audio to at least 1.1 seconds (17600 samples at 16kHz) to ensure we exceed 1000ms
                                    let min_samples = 17600; // 1.1 seconds at 16kHz for safety margin
                                    if audio.len() < min_samples {
                                        println!(
                                            "🔧 Padding audio to minimum length ({} -> {} samples)",
                                            audio.len(),
                                            min_samples
                                        );
                                        audio.resize(min_samples, 0.0);
                                    }
                                    match transcriber_clone.transcribe(&audio) {
                                        Ok(text) => {
                                            let trimmed_text = text.trim();

                                            // Check if transcription is empty, whitespace-only, or blank audio
                                            if trimmed_text.is_empty()
                                                || trimmed_text == "[BLANK_AUDIO]"
                                            {
                                                if trimmed_text == "[BLANK_AUDIO]" {
                                                    println!("🔇 No speech detected");
                                                } else {
                                                    println!("⚠️  No text transcribed");
                                                }
                                            } else {
                                                println!("📝 Transcribed: \"{}\"", trimmed_text);

                                                // Wait a moment before typing
                                                std::thread::sleep(Duration::from_millis(100));

                                                // Type the transcribed text
                                                if let Ok(mut enigo) = enigo_clone.lock() {
                                                    if let Err(e) = enigo.text(trimmed_text) {
                                                        eprintln!("Failed to type text: {}", e);
                                                    } else {
                                                        println!("✅ Text typed successfully");
                                                    }
                                                } else {
                                                    eprintln!("Failed to acquire enigo lock");
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Transcription failed: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to load resampled audio: {}", e);
                                }
                            }

                            // Clean up resampled file
                            if let Err(e) = std::fs::remove_file(&resampled_path) {
                                eprintln!("Failed to clean up resampled file: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to resample audio: {}", e);
                        }
                    }

                    // Clean up temporary file
                    if let Err(e) = std::fs::remove_file(&path) {
                        eprintln!("Failed to clean up temp file: {}", e);
                    }
                }

                println!("🎤 Ready for next recording...");
            }
        }
    }) {
        return Err(anyhow::anyhow!(
            "Error listening for key events: {:?}",
            error
        ));
    }

    Ok(())
}

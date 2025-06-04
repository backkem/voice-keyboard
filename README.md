# Voice Keyboard

A voice-to-text application that types transcribed speech directly into any application using push-to-talk.

## Features

- **Push-to-talk**: Hold the Quote key to record audio
- **Voice transcription**: Uses Whisper for speech-to-text
- **System-wide typing**: Works in any application that accepts text input
- **Portable**: Self-contained executable with relative model paths

## Usage

1. Download a Whisper model (e.g., `ggml-base.en.bin`)
2. Place it in the `models/` directory (dev) or `whisper-cpp/` directory (release)
3. Run the application:
   ```bash
   voicekb
   ```
4. Hold the Quote key to record, release to transcribe and type

The release build looks for ./whisper-cpp/ggml-base.en.bin relative to the binary,
enabling execution form the PATH.

## Building

```bash
cd rust
cargo build --release --bin voicekb
```

The binary will be output as `voicekb` (or `voicekb.exe` on Windows).

## Future Improvements

- A Tauri-based tray icon with basic configuration options

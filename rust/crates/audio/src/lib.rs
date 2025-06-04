pub mod device;
pub mod peaks;
pub mod recorder;
pub mod resample;

pub type SampleType = i16;

pub use device::{get_input_device, get_microphones, AudioDevice};
pub use peaks::send_peaks;
pub use recorder::SimpleRecorder;
pub use resample::resample_wav_file;

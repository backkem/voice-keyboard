use crate::SampleType;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

const THROTTLE_DURATION: Duration = Duration::from_millis(10);

pub async fn send_peaks<F>(mut peaks_rx: broadcast::Receiver<Vec<SampleType>>, mut callback: F)
where
    F: FnMut(SampleType) + Send + 'static,
{
    let mut last_send_time = Instant::now();

    while let Ok(samples) = peaks_rx.recv().await {
        let current_peak = samples.iter().fold(0 as SampleType, |peak, &sample| {
            if sample > 0 {
                peak.max(sample.min(SampleType::MAX))
            } else {
                peak.min(sample.max(SampleType::MIN))
            }
        });
        if last_send_time.elapsed() >= THROTTLE_DURATION {
            callback(current_peak);
            last_send_time = Instant::now();
        }
    }
}

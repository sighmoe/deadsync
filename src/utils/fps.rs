use std::time::{Duration, Instant};

pub struct FPSCounter {
    last_update_time: Instant,     // Renamed for clarity
    frames_since_last_update: u32, // Renamed for clarity
}

impl FPSCounter {
    pub fn new() -> Self {
        FPSCounter {
            last_update_time: Instant::now(),
            frames_since_last_update: 0,
        }
    }

    /// Updates the FPS counter. Returns `Some(fps)` once per second (approximately).
    /// Call this once per rendered frame.
    pub fn update(&mut self) -> Option<u32> {
        self.frames_since_last_update += 1;
        let now = Instant::now();
        let elapsed_duration = now.duration_since(self.last_update_time);

        if elapsed_duration >= Duration::from_secs(1) {
            // Calculate FPS based on frames and actual elapsed time for more accuracy
            // let fps_precise = self.frames_since_last_update as f64 / elapsed_duration.as_secs_f64();
            // let fps_to_report = fps_precise.round() as u32;
            // For simplicity, just use frame count over ~1s interval
            let fps_to_report = self.frames_since_last_update;

            self.frames_since_last_update = 0;
            // Adjust last_update_time to maintain a more consistent 1-second interval
            // rather than just setting it to 'now'. This prevents drift if update intervals are not exact.
            self.last_update_time += Duration::from_secs(1);
            // However, if we've fallen behind significantly, reset to now to avoid large jumps.
            if self.last_update_time < now - Duration::from_secs(1) {
                self.last_update_time = now;
            }
            Some(fps_to_report)
        } else {
            None
        }
    }
}

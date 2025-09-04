// ===== FILE: src/core/timing.rs =====
use rssp::bpm::{parse_bpm_map, normalize_float_digits};
use log::warn;

#[derive(Debug, Clone, Default)]
pub struct TimingData {
    /// A pre-calculated list of points where BPM changes, with the time at that point.
    points: Vec<BeatTimePoint>,
    /// A list of beats where the song stops, and for how long.
    stops_at_beat: Vec<(f32, f32)>,
    /// The song's initial offset from time 0 in seconds.
    song_offset_sec: f32,
}

#[derive(Debug, Clone, Default)]
struct BeatTimePoint {
    beat: f32,
    time_sec: f32,
    bpm: f32,
}

impl TimingData {
    pub fn from_chart_data(
        song_offset_sec: f32,
        chart_bpms: Option<&str>,
        global_bpms: &str,
        chart_stops: Option<&str>,
        global_stops: &str,
    ) -> Self {
        let mut new_timing = Self { song_offset_sec, ..Default::default() };

        let bpms_str = chart_bpms.filter(|s| !s.is_empty()).unwrap_or(global_bpms);
        let normalized_bpms = normalize_float_digits(bpms_str);
        let mut parsed_bpms = parse_bpm_map(&normalized_bpms).into_iter()
            .map(|(b, v)| (b as f32, v as f32))
            .collect::<Vec<_>>();

        if parsed_bpms.is_empty() { parsed_bpms.push((0.0, 120.0)); }
        if parsed_bpms[0].0 != 0.0 { parsed_bpms.insert(0, (0.0, parsed_bpms[0].1)); }

        let mut current_time = 0.0;
        let mut last_beat = 0.0;
        let mut last_bpm = parsed_bpms[0].1;

        for (beat, bpm) in &parsed_bpms {
            if *beat > last_beat && last_bpm > 0.0 {
                current_time += (*beat - last_beat) * (60.0 / last_bpm);
            }
            new_timing.points.push(BeatTimePoint {
                beat: *beat,
                time_sec: song_offset_sec + current_time,
                bpm: *bpm,
            });
            last_beat = *beat;
            last_bpm = *bpm;
        }

        let stops_str = chart_stops.filter(|s| !s.is_empty()).unwrap_or(global_stops);
        if let Ok(stops) = parsing::simfile::parse_stops(stops_str) {
             new_timing.stops_at_beat = stops;
        }

        new_timing
    }

    pub fn get_beat_for_time(&self, target_time_sec: f32) -> f32 {
        if self.points.is_empty() { return 0.0; }

        // Adjust target time for stops that have already passed
        let mut adjusted_target_time = target_time_sec;
        for (stop_beat, stop_duration) in &self.stops_at_beat {
            let time_of_stop = self.get_time_for_beat(*stop_beat);
            if time_of_stop < target_time_sec {
                adjusted_target_time -= stop_duration;
            }
        }
        
        let point = match self.points.binary_search_by(|p| p.time_sec.partial_cmp(&adjusted_target_time).unwrap()) {
            Ok(i) => &self.points[i],
            Err(i) => &self.points[i.saturating_sub(1)],
        };
        
        let time_since_point = adjusted_target_time - point.time_sec;
        if point.bpm <= 0.0 {
            point.beat
        } else {
            point.beat + time_since_point / (60.0 / point.bpm)
        }
    }

    pub fn get_time_for_beat(&self, target_beat: f32) -> f32 {
        if self.points.is_empty() { return 0.0; }

        let point = match self.points.binary_search_by(|p| p.beat.partial_cmp(&target_beat).unwrap()) {
            Ok(i) => &self.points[i],
            Err(i) => &self.points[i.saturating_sub(1)],
        };

        let beats_since_point = target_beat - point.beat;
        let mut time = point.time_sec;

        if point.bpm > 0.0 {
            time += beats_since_point * (60.0 / point.bpm);
        }

        for (stop_beat, stop_duration) in &self.stops_at_beat {
            if *stop_beat > point.beat && *stop_beat < target_beat {
                time += stop_duration;
            }
        }
        time
    }
}

// Dummy module for parsing stops, since it's not exposed by rssp but used in your old file.
// This is a simplified version.
mod parsing {
    pub(super) mod simfile {
        pub fn parse_stops(s: &str) -> Result<Vec<(f32, f32)>, &'static str> {
            if s.is_empty() { return Ok(Vec::new()); }
            s.split(',')
                .map(|pair| {
                    let mut parts = pair.split('=');
                    let beat_str = parts.next().ok_or("Missing beat")?.trim();
                    let duration_str = parts.next().ok_or("Missing duration")?.trim();
                    let beat = beat_str.parse::<f32>().map_err(|_| "Invalid beat")?;
                    let duration = duration_str.parse::<f32>().map_err(|_| "Invalid duration")?;
                    Ok((beat, duration))
                })
                .collect()
        }
    }
}

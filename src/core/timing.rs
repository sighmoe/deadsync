// ===== FILE: src/core/timing.rs =====
use rssp::bpm::{parse_bpm_map, normalize_float_digits};
use log::info;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct TimingData {
    /// A pre-calculated mapping from a note row index to its precise beat.
    row_to_beat: Arc<Vec<f32>>,
    /// A pre-calculated mapping from a beat to its precise time in seconds.
    beat_to_time: Arc<Vec<BeatTimePoint>>,
    stops_at_beat: Vec<(f32, f32)>,
    song_offset_sec: f32,
    global_offset_sec: f32,
}

#[derive(Debug, Clone, Default, Copy)]
struct BeatTimePoint {
    beat: f32,
    time_sec: f32,
    bpm: f32,
}

impl TimingData {
    pub fn from_chart_data(
        song_offset_sec: f32,
        global_offset_sec: f32,
        chart_bpms: Option<&str>,
        global_bpms: &str,
        chart_stops: Option<&str>,
        global_stops: &str,
        raw_note_bytes: &[u8],
    ) -> Self {
        // --- PASS 1: Calculate beat-to-time mapping from BPMs and Stops ---
        let bpms_str = chart_bpms.filter(|s| !s.is_empty()).unwrap_or(global_bpms);
        let normalized_bpms = normalize_float_digits(bpms_str);
        let mut parsed_bpms = parse_bpm_map(&normalized_bpms)
            .into_iter()
            .map(|(b, v)| (b as f32, v as f32))
            .collect::<Vec<_>>();

        if parsed_bpms.is_empty() { parsed_bpms.push((0.0, 120.0)); }
        if parsed_bpms.first().map_or(true, |(b, _)| *b != 0.0) {
            parsed_bpms.insert(0, (0.0, parsed_bpms[0].1));
        }

        let mut beat_to_time = Vec::with_capacity(parsed_bpms.len());
        let mut current_time = 0.0;
        let mut last_beat = 0.0;
        let mut last_bpm = parsed_bpms[0].1;

        for &(beat, bpm) in &parsed_bpms {
            if beat > last_beat && last_bpm > 0.0 {
                current_time += (beat - last_beat) * (60.0 / last_bpm);
            }
            beat_to_time.push(BeatTimePoint {
                beat,
                time_sec: song_offset_sec + current_time,
                bpm,
            });
            last_beat = beat;
            last_bpm = bpm;
        }

        let stops_str = chart_stops.filter(|s| !s.is_empty()).unwrap_or(global_stops);
        let stops_at_beat = match parsing::simfile::parse_stops(stops_str) {
            Ok(stops) => stops,
            Err(_) => vec![],
        };

        // --- PASS 2: Calculate row-to-beat mapping from the raw note data ---
        let mut row_to_beat = Vec::new();
        let mut measure_index = 0;
        
        for measure_bytes in raw_note_bytes.split(|&b| b == b',') {
            let num_rows_in_measure = measure_bytes.split(|&b| b == b'\n').filter(|line| !line.is_empty() && !line.iter().all(|c| c.is_ascii_whitespace())).count();
            if num_rows_in_measure == 0 { continue; }
            
            for row_in_measure in 0..num_rows_in_measure {
                let beat = (measure_index as f32 * 4.0) + (row_in_measure as f32 / num_rows_in_measure as f32 * 4.0);
                row_to_beat.push(beat);
            }
            measure_index += 1;
        }
        info!("TimingData processed {} note rows.", row_to_beat.len());


        Self {
            row_to_beat: Arc::new(row_to_beat),
            beat_to_time: Arc::new(beat_to_time),
            stops_at_beat,
            song_offset_sec,
            global_offset_sec,
        }
    }

    pub fn get_beat_for_row(&self, row_index: usize) -> Option<f32> {
        self.row_to_beat.get(row_index).copied()
    }

    pub fn get_beat_for_time(&self, target_time_sec: f32) -> f32 {
        let points = &self.beat_to_time;
        if points.is_empty() { return 0.0; }

        // Start with the time we want the beat for, including global offset.
        let mut time_for_beat_calc = target_time_sec + self.global_offset_sec;

        // Now, remove the duration of any stops that have already occurred.
        // The stops are defined in the song's timeline, so we check against target_time_sec.
        for (stop_beat, stop_duration) in &self.stops_at_beat {
            let time_of_stop = self.get_time_for_beat_internal(*stop_beat);
            if time_of_stop < target_time_sec {
                time_for_beat_calc -= stop_duration;
            }
        }

        let point_idx = match points.binary_search_by(|p| p.time_sec.partial_cmp(&time_for_beat_calc).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let point = &points[point_idx];
        
        let time_since_point = time_for_beat_calc - point.time_sec;
        if point.bpm <= 0.0 {
            point.beat
        } else {
            point.beat + time_since_point / (60.0 / point.bpm)
        }
    }

    pub fn get_time_for_beat(&self, target_beat: f32) -> f32 {
        self.get_time_for_beat_internal(target_beat) - self.global_offset_sec
    }

    fn get_time_for_beat_internal(&self, target_beat: f32) -> f32 {
        let points = &self.beat_to_time;
        if points.is_empty() { return 0.0; }

        let point_idx = match points.binary_search_by(|p| p.beat.partial_cmp(&target_beat).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let point = &points[point_idx];

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

    pub fn get_bpm_for_beat(&self, target_beat: f32) -> f32 {
        let points = &self.beat_to_time;
        if points.is_empty() { return 120.0; } // Fallback BPM

        let point_idx = match points.binary_search_by(|p| p.beat.partial_cmp(&target_beat).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        points[point_idx].bpm
    }
}


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
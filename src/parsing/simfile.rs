use super::bpm;
use super::parse;
use super::stats::{
    compute_stream_counts, generate_breakdown, minimize_chart_and_count, ArrowStats, BreakdownMode,
    StreamCounts,
};

use log::{debug, error, info, warn};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteChar {
    Empty = b'0' as isize,
    Tap = b'1' as isize,
    HoldStart = b'2' as isize,
    HoldEnd = b'3' as isize,
    RollStart = b'4' as isize,
    Mine = b'M' as isize,
    Lift = b'L' as isize,
    Fake = b'F' as isize,
    Unsupported,
}
impl From<u8> for NoteChar {
    fn from(byte: u8) -> Self {
        match byte {
            b'0' => NoteChar::Empty,
            b'1' => NoteChar::Tap,
            b'2' => NoteChar::HoldStart,
            b'3' => NoteChar::HoldEnd,
            b'4' => NoteChar::RollStart,
            b'M' => NoteChar::Mine,
            b'L' => NoteChar::Lift,
            b'F' => NoteChar::Fake,
            _ => NoteChar::Unsupported,
        }
    }
}
pub type NoteLine = [NoteChar; 4];

#[derive(Debug, Clone, Default)]
pub struct ProcessedChartData {
    pub measures: Vec<Vec<NoteLine>>,
    pub stats: ArrowStats,
    pub measure_densities: Vec<usize>,
    pub measure_nps_vec: Vec<f32>,
    pub max_nps: f32,
    // pub median_nps: f32, // Optional
    pub stream_counts: StreamCounts,
    pub breakdown_detailed: String,
    pub breakdown_simplified: String,
}

#[derive(Debug, Clone)]
pub struct ChartInfo {
    pub stepstype: String,
    pub description: String, // Original description from file
    pub difficulty: String,
    pub meter: String,
    pub credit: String, // Original credit from file (mainly for SSC)
    pub stepartist_display_name: String, // Combined and cleaned name for UI
    pub notes_data_raw: String,
    pub bpms_chart: Option<String>,
    pub stops_chart: Option<String>,
    pub processed_data: Option<ProcessedChartData>,
    pub calculated_length_sec: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct SongInfo {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub title_translit: String,
    pub subtitle_translit: String,
    pub artist_translit: String,
    pub offset: f32,
    pub bpms_header: Vec<(f32, f32)>,
    pub stops_header: Vec<(f32, f32)>,
    pub charts: Vec<ChartInfo>,
    pub simfile_path: PathBuf,
    pub folder_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub banner_path: Option<PathBuf>,
    pub sample_start: Option<f32>,
    pub sample_length: Option<f32>,
}

#[derive(Debug)]
pub enum ParseError {
    Io(io::Error),
    NotFound(PathBuf),
    UnsupportedExtension(String),
    Utf8Error { tag: String, source: str::Utf8Error },
    InvalidFormat(String),
    MissingTag(String),
    NoCharts,
}

pub fn parse_bpms(bpm_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
    let mut bpms = Vec::new();
    if bpm_string.trim().is_empty() {
        return Ok(bpms);
    }
    for part in bpm_string.split(',') {
        let components: Vec<&str> = part.split('=').collect();
        if components.len() == 2 {
            let beat = components[0].trim().parse::<f32>().map_err(|e| {
                ParseError::InvalidFormat(format!("BPM beat value '{}': {}", components[0], e))
            })?;
            let bpm_val = components[1].trim().parse::<f32>().map_err(|e| {
                ParseError::InvalidFormat(format!("BPM value '{}': {}", components[1], e))
            })?;
            if bpm_val <= 0.0 {
                warn!(
                    "Ignoring non-positive BPM value: {} at beat {}",
                    bpm_val, beat
                );
                continue;
            }
            bpms.push((beat, bpm_val));
        } else if !part.trim().is_empty() {
            warn!("Malformed BPM segment: '{}', skipping.", part);
        }
    }
    bpms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(bpms)
}

pub fn parse_stops(stop_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
    let mut stops = Vec::new();
    if stop_string.trim().is_empty() {
        return Ok(stops);
    }
    for part in stop_string.split(',') {
        let components: Vec<&str> = part.split('=').collect();
        if components.len() == 2 {
            let beat = components[0].trim().parse::<f32>().map_err(|e| {
                ParseError::InvalidFormat(format!("Stop beat value '{}': {}", components[0], e))
            })?;
            let duration = components[1].trim().parse::<f32>().map_err(|e| {
                ParseError::InvalidFormat(format!("Stop duration value '{}': {}", components[1], e))
            })?;
            if duration <= 0.0 {
                warn!(
                    "Ignoring non-positive STOPS duration value: {} at beat {}",
                    duration, beat
                );
                continue;
            }
            stops.push((beat, duration));
        } else if !part.trim().is_empty() {
            warn!("Malformed STOPS segment: '{}', skipping.", part);
        }
    }
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(stops)
}

fn parse_minimized_bytes_to_measures(minimized_bytes: &[u8]) -> Vec<Vec<NoteLine>> {
    let mut all_measures: Vec<Vec<NoteLine>> = Vec::new();
    let mut current_measure_lines: Vec<NoteLine> = Vec::new();

    for line_segment_bytes in minimized_bytes.split(|&b| b == b'\n') {
        if line_segment_bytes.is_empty() {
            continue;
        }

        if line_segment_bytes == b"," {
            all_measures.push(std::mem::take(&mut current_measure_lines));
        } else if line_segment_bytes.starts_with(b"//") {
            // Comments should ideally be stripped by minimize_chart_and_count
        } else if line_segment_bytes.len() >= 4 {
            let mut note_line_arr: NoteLine = [NoteChar::Empty; 4];
            for (i, &byte_char) in line_segment_bytes.iter().take(4).enumerate() {
                note_line_arr[i] = NoteChar::from(byte_char);
            }
            current_measure_lines.push(note_line_arr);
        }
    }
    if !current_measure_lines.is_empty() {
        all_measures.push(current_measure_lines);
    }
    all_measures
}

fn find_top_level_tag_value<'a>(data_bytes: &'a [u8], tag_to_find: &[u8]) -> Option<&'a [u8]> {
    data_bytes
        .windows(tag_to_find.len())
        .position(|window| window == tag_to_find)
        .and_then(|tag_pos| {
            let value_start = tag_pos + tag_to_find.len();
            if value_start >= data_bytes.len() {
                return None;
            }
            let value_slice = &data_bytes[value_start..];
            value_slice
                .iter()
                .position(|&b| b == b';')
                .map(|end_idx| &value_slice[..end_idx])
        })
}

pub fn parse_simfile(simfile_path: &Path) -> Result<SongInfo, ParseError> {
    info!("Parsing simfile: {:?}", simfile_path);
    let content_bytes_vec = fs::read(simfile_path)?;

    let extension_str = simfile_path
        .extension()
        .and_then(|os_str| os_str.to_str())
        .unwrap_or("")
        .to_lowercase();

    let (
        title_bytes_opt,
        subtitle_bytes_opt,
        artist_bytes_opt,
        titletranslit_bytes_opt,
        subtitletranslit_bytes_opt,
        artisttranslit_bytes_opt,
        offset_bytes_opt,
        bpms_header_bytes_opt,
        raw_charts_list,
    ) = parse::extract_sections(&content_bytes_vec, &extension_str).map_err(|e| {
        ParseError::InvalidFormat(format!(
            "Failed to extract sections from {:?}: {}",
            simfile_path, e
        ))
    })?;

    let stops_header_bytes_opt = find_top_level_tag_value(&content_bytes_vec, b"#STOPS:");
    let music_path_bytes_opt = find_top_level_tag_value(&content_bytes_vec, b"#MUSIC:");
    let banner_path_bytes_opt = find_top_level_tag_value(&content_bytes_vec, b"#BANNER:");
    let samplestart_bytes_opt = find_top_level_tag_value(&content_bytes_vec, b"#SAMPLESTART:");
    let samplelength_bytes_opt = find_top_level_tag_value(&content_bytes_vec, b"#SAMPLELENGTH:");

    let bytes_to_string_cleaned = |opt_bytes: Option<&[u8]>| {
        opt_bytes
            .and_then(|b| str::from_utf8(b).ok().map(parse::clean_tag))
            .unwrap_or_default()
    };
    let bytes_to_string_trimmed = |opt_bytes: Option<&[u8]>| {
        opt_bytes
            .and_then(|b| str::from_utf8(b).ok().map(|s| s.trim().to_string()))
            .unwrap_or_default()
    };
    let bytes_to_opt_string_trimmed = |opt_bytes: Option<&[u8]>| {
        opt_bytes.and_then(|b| str::from_utf8(b).ok().map(|s| s.trim().to_string()))
    };

    let title = bytes_to_string_cleaned(title_bytes_opt);
    let subtitle = bytes_to_string_cleaned(subtitle_bytes_opt);
    let artist = bytes_to_string_cleaned(artist_bytes_opt);
    let title_translit = bytes_to_string_cleaned(titletranslit_bytes_opt);
    let subtitle_translit = bytes_to_string_cleaned(subtitletranslit_bytes_opt);
    let artist_translit = bytes_to_string_cleaned(artisttranslit_bytes_opt);

    let offset_str = bytes_to_string_trimmed(offset_bytes_opt);
    let bpms_header_str = bytes_to_string_trimmed(bpms_header_bytes_opt);
    let stops_header_str = bytes_to_string_trimmed(stops_header_bytes_opt);
    let audio_filename = bytes_to_opt_string_trimmed(music_path_bytes_opt);
    let banner_filename = bytes_to_opt_string_trimmed(banner_path_bytes_opt);
    let sample_start_str = bytes_to_opt_string_trimmed(samplestart_bytes_opt);
    let sample_length_str = bytes_to_opt_string_trimmed(samplelength_bytes_opt);

    let offset = offset_str.parse::<f32>().unwrap_or(0.0);
    let bpms_header = parse_bpms(&bpms_header_str)?;
    let stops_header = parse_stops(&stops_header_str)?;
    let sample_start = sample_start_str
        .as_deref()
        .and_then(|s| s.parse::<f32>().ok());
    let sample_length = sample_length_str
        .as_deref()
        .and_then(|s| s.parse::<f32>().ok());

    let mut charts: Vec<ChartInfo> = Vec::new();
    for (chart_content_block_bytes, ssc_chart_bpms_bytes_opt) in raw_charts_list {
        let (metadata_fields_byte_slices, actual_notes_data_bytes) =
            parse::split_notes_fields(&chart_content_block_bytes);

        if metadata_fields_byte_slices.len() < 5 {
            warn!("Chart in {:?} has incomplete metadata fields after split_notes_fields (found {}), skipping: {:?}", 
                simfile_path,
                metadata_fields_byte_slices.len(),
                metadata_fields_byte_slices.iter().map(|s| String::from_utf8_lossy(s)).collect::<Vec<_>>()
            );
            continue;
        }

        let stepstype = String::from_utf8_lossy(metadata_fields_byte_slices[0])
            .trim()
            .to_string();
        let description_raw = String::from_utf8_lossy(metadata_fields_byte_slices[1])
            .trim()
            .to_string();
        let difficulty = String::from_utf8_lossy(metadata_fields_byte_slices[2])
            .trim()
            .to_string();
        let meter = String::from_utf8_lossy(metadata_fields_byte_slices[3])
            .trim()
            .to_string();
        let credit_or_radar_bytes = metadata_fields_byte_slices[4];

        let credit_raw = if extension_str == "ssc" {
            String::from_utf8_lossy(credit_or_radar_bytes)
                .trim()
                .to_string()
        } else {
            String::new() // .sm files use radar values here, credit is not standardly in this field for .sm
                          // If credit is desired for .sm, it would typically be in the description.
        };

        // Combine credit and description for stepartist display name
        // Prioritize credit if it exists and is different from description.
        // Otherwise, use description. If both are empty, it will be empty.
        let stepartist_display_name = {
            let c = credit_raw.trim();
            let d = description_raw.trim();
            if !c.is_empty() {
                if !d.is_empty() && c.to_lowercase() != d.to_lowercase() {
                    format!("{} {}", c, d) // Combine if different and both non-empty
                } else {
                    c.to_string() // Use credit if description is same or empty
                }
            } else {
                d.to_string() // Use description if credit is empty
            }
        };

        let notes_data_raw = String::from_utf8_lossy(actual_notes_data_bytes).to_string();

        let chart_bpms = ssc_chart_bpms_bytes_opt
            .map(|b_vec| String::from_utf8_lossy(&b_vec).trim().to_string());

        let chart_stops: Option<String> = if extension_str == "ssc" {
            parse::parse_subtag(&chart_content_block_bytes, b"#STOPS:")
                .map(|b_vec| String::from_utf8_lossy(&b_vec).trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        };

        if stepstype.is_empty() || difficulty.is_empty() || meter.is_empty() {
            warn!("Skipping chart in {:?} (type: '{}', desc: '{}', diff: '{}', meter: '{}') due to missing essential metadata fields post-split.",
                simfile_path, stepstype, description_raw, difficulty, meter);
            continue;
        }

        charts.push(ChartInfo {
            stepstype,
            description: description_raw,
            difficulty,
            meter,
            credit: credit_raw,
            stepartist_display_name,
            notes_data_raw,
            bpms_chart: chart_bpms,
            stops_chart: chart_stops,
            processed_data: None,
            calculated_length_sec: None,
        });
    }

    if title.is_empty() && !simfile_path.to_string_lossy().contains("EditCourses") {
        return Err(ParseError::MissingTag(format!(
            "#TITLE in {:?}",
            simfile_path
        )));
    }
    if bpms_header.is_empty()
        && charts
            .iter()
            .all(|c| c.bpms_chart.as_ref().map_or(true, |s| s.is_empty()))
    {
        if !charts.is_empty() {
            return Err(ParseError::MissingTag(format!(
                "#BPMS (neither header nor chart-specific, but charts exist) in {:?}",
                simfile_path
            )));
        }
    }
    if charts.is_empty() && !simfile_path.to_string_lossy().contains("EditCourses") {
        return Err(ParseError::NoCharts);
    }

    let folder_path = simfile_path
        .parent()
        .ok_or_else(|| {
            ParseError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid simfile path",
            ))
        })?
        .to_path_buf();
    let audio_path = audio_filename
        .map(|f| folder_path.join(f))
        .filter(|p| p.exists());
    let banner_path = banner_filename
        .map(|f| folder_path.join(f))
        .filter(|p| p.exists());

    let mut song_info = SongInfo {
        title,
        subtitle,
        artist,
        title_translit,
        subtitle_translit,
        artist_translit,
        offset,
        bpms_header,
        stops_header,
        charts,
        simfile_path: simfile_path.to_path_buf(),
        folder_path,
        audio_path,
        banner_path,
        sample_start,
        sample_length,
    };

    for chart_idx in 0..song_info.charts.len() {
        let (
            notes_data_raw_clone,
            chart_stepstype_clone,
            chart_difficulty_clone,
            chart_specific_bpms_str_opt_clone,
            chart_specific_stops_str_opt_clone,
        ) = {
            let chart_ref = &song_info.charts[chart_idx];
            if chart_ref.notes_data_raw.trim().is_empty() {
                if let Some(chart_mut) = song_info.charts.get_mut(chart_idx) {
                    chart_mut.processed_data = Some(ProcessedChartData::default());
                    chart_mut.calculated_length_sec = Some(0.0);
                }
                continue;
            }
            (
                chart_ref.notes_data_raw.clone(),
                chart_ref.stepstype.clone(),
                chart_ref.difficulty.clone(),
                chart_ref.bpms_chart.clone(),
                chart_ref.stops_chart.clone(),
            )
        };

        let (minimized_bytes_for_measure_parsing, stats, measure_densities) =
            minimize_chart_and_count(notes_data_raw_clone.as_bytes());

        let measures = parse_minimized_bytes_to_measures(&minimized_bytes_for_measure_parsing);

        if measures.is_empty()
            && !notes_data_raw_clone.trim().is_empty()
            && (!measure_densities.is_empty() && measure_densities.iter().any(|&d| d > 0))
        {
            warn!("Chart (type: '{}', difficulty: '{}') in {:?} resulted in zero measures after processing. Raw notes len: {}. Minimized bytes len: {}. Densities (first 5): {:?}", 
                chart_stepstype_clone, chart_difficulty_clone, simfile_path, notes_data_raw_clone.len(), minimized_bytes_for_measure_parsing.len(), measure_densities.iter().take(5).collect::<Vec<_>>());
        }

        let mut bpm_map_for_this_chart_f32 = song_info.bpms_header.clone();
        if let Some(chart_bpms_s) = chart_specific_bpms_str_opt_clone {
            if !chart_bpms_s.trim().is_empty() {
                if let Ok(parsed_chart_bpms) = parse_bpms(&chart_bpms_s) {
                    if !parsed_chart_bpms.is_empty() {
                        bpm_map_for_this_chart_f32 = parsed_chart_bpms;
                    }
                }
            }
        }
        let bpm_map_for_this_chart_f64: Vec<(f64, f64)> = bpm_map_for_this_chart_f32
            .iter()
            .map(|(b, v)| (*b as f64, *v as f64))
            .collect();

        let measure_nps_vec_f64 =
            bpm::compute_measure_nps_vec(&measure_densities, &bpm_map_for_this_chart_f64);
        let (max_nps_f64, _median_nps_f64) = bpm::get_nps_stats(&measure_nps_vec_f64);

        let measure_nps_vec_f32: Vec<f32> = measure_nps_vec_f64
            .into_iter()
            .map(|nps| nps as f32)
            .collect();
        let max_nps_f32 = max_nps_f64 as f32;

        let stream_counts = compute_stream_counts(&measure_densities);
        let breakdown_detailed = generate_breakdown(&measure_densities, BreakdownMode::Detailed);
        let breakdown_simplified =
            generate_breakdown(&measure_densities, BreakdownMode::Simplified);

        if let Some(chart_mut) = song_info.charts.get_mut(chart_idx) {
            chart_mut.processed_data = Some(ProcessedChartData {
                measures,
                stats,
                measure_densities,
                measure_nps_vec: measure_nps_vec_f32,
                max_nps: max_nps_f32,
                stream_counts,
                breakdown_detailed,
                breakdown_simplified,
            });

            if let Some(pd) = &chart_mut.processed_data {
                let current_chart_bpms = if let Some(cb_str) = &chart_mut.bpms_chart {
                    parse_bpms(cb_str).unwrap_or_else(|_| song_info.bpms_header.clone())
                } else {
                    song_info.bpms_header.clone()
                };
                if !current_chart_bpms.is_empty() {
                    chart_mut.calculated_length_sec = Some(self::calculate_chart_duration_seconds(
                        pd,
                        &current_chart_bpms,
                    ));
                } else {
                    warn!(
                        "No BPMs available for chart '{} {}' in {:?}. Cannot calculate duration.",
                        chart_mut.stepstype, chart_mut.difficulty, simfile_path
                    );
                    chart_mut.calculated_length_sec = None;
                }
            }
        }
    }

    song_info.charts.retain(|c| {
        c.processed_data.as_ref().map_or(false, |pd| {
            !pd.measures.is_empty() || pd.stats.total_arrows > 0
        })
    });
    if song_info.charts.is_empty()
        && !song_info
            .simfile_path
            .to_string_lossy()
            .contains("EditCourses")
    {
        // If, after retaining, there are no charts, then return an error
        // unless it's an EditCourses file which might legitimately have no charts of its own.
        return Err(ParseError::NoCharts);
    }

    Ok(song_info)
}

pub fn get_bpm_at_beat(bpm_map: &[(f32, f32)], beat: f32) -> f32 {
    let mut current_bpm = if !bpm_map.is_empty() {
        bpm_map[0].1
    } else {
        120.0
    };
    for &(b_beat, b_bpm) in bpm_map {
        if beat >= b_beat {
            current_bpm = b_bpm;
        } else {
            break;
        }
    }
    current_bpm
}

pub fn calculate_chart_duration_seconds(
    processed_data: &ProcessedChartData,
    bpm_map: &[(f32, f32)],
) -> f32 {
    if bpm_map.is_empty() {
        return processed_data.measures.len() as f32 * 2.0;
    }
    let mut total_length_seconds = 0.0;
    for (i, _measure_lines) in processed_data.measures.iter().enumerate() {
        let measure_start_beat = i as f32 * 4.0;
        let current_bpm = get_bpm_at_beat(bpm_map, measure_start_beat);
        if current_bpm <= 0.0 {
            warn!("Invalid BPM ({}) found at measure {}, beat {}. Skipping measure for duration calculation.", current_bpm, i, measure_start_beat);
            continue;
        }
        let measure_length_s = (4.0 / current_bpm) * 60.0;
        total_length_seconds += measure_length_s;
    }
    total_length_seconds
}

pub fn parse_song_folder(folder_path: &Path) -> Result<SongInfo, ParseError> {
    debug!("Scanning song folder: {:?}", folder_path);
    let mut ssc_path: Option<PathBuf> = None;
    let mut sm_path: Option<PathBuf> = None;

    if folder_path.is_dir() {
        for entry_res in fs::read_dir(folder_path)? {
            let entry = entry_res?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext_os) = path.extension() {
                    if let Some(ext_str) = ext_os.to_str() {
                        match ext_str.to_lowercase().as_str() {
                            "ssc" => {
                                ssc_path = Some(path.clone());
                                break;
                            }
                            "sm" => {
                                if ssc_path.is_none() {
                                    sm_path = Some(path.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    } else {
        return Err(ParseError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Song folder not found: {:?}", folder_path),
        )));
    }

    if let Some(path) = ssc_path {
        parse_simfile(&path)
    } else if let Some(path) = sm_path {
        parse_simfile(&path)
    } else {
        Err(ParseError::NotFound(folder_path.join("*.ssc/*.sm")))
    }
}
pub fn scan_pack(pack_path: &Path) -> Vec<Result<SongInfo, ParseError>> {
    info!("Scanning pack: {:?}", pack_path);
    let mut songs = Vec::new();
    if !pack_path.is_dir() {
        error!("Pack path is not a directory: {:?}", pack_path);
        return songs;
    }
    match fs::read_dir(pack_path) {
        Ok(entries) => {
            for entry_res in entries {
                if let Ok(entry) = entry_res {
                    let path = entry.path();
                    if path.is_dir() {
                        songs.push(parse_song_folder(&path));
                    }
                } else if let Err(e) = entry_res {
                    error!("Error reading entry in pack {:?}: {}", pack_path, e);
                }
            }
        }
        Err(e) => {
            error!("Failed to read pack directory {:?}: {}", pack_path, e);
        }
    }
    songs
}
pub fn scan_packs(packs_root_dir: &Path) -> Vec<SongInfo> {
    info!("Scanning for packs in: {:?}", packs_root_dir);
    let mut all_songs = Vec::new();
    if !packs_root_dir.is_dir() {
        error!(
            "Packs root directory not found or is not a directory: {:?}",
            packs_root_dir
        );
        return all_songs;
    }
    match fs::read_dir(packs_root_dir) {
        Ok(entries) => {
            for entry_res in entries {
                if let Ok(entry) = entry_res {
                    let path = entry.path();
                    if path.is_dir() {
                        let pack_songs_results = scan_pack(&path);
                        for result in pack_songs_results {
                            match result {
                                Ok(song_info) => {
                                    if song_info.charts.is_empty() {
                                        warn!("Song '{}' from {:?} ultimately has no playable charts after processing, skipping.", song_info.title, song_info.simfile_path);
                                    } else {
                                        debug!(
                                            "Successfully parsed and processed song: {} from {:?}",
                                            song_info.title, song_info.simfile_path
                                        );
                                        all_songs.push(song_info);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse a song file: {}", e);
                                }
                            }
                        }
                    }
                } else if let Err(e) = entry_res {
                    error!(
                        "Error reading entry in packs root {:?}: {}",
                        packs_root_dir, e
                    );
                }
            }
        }
        Err(e) => {
            error!(
                "Failed to read root songs directory {:?}: {}",
                packs_root_dir, e
            );
        }
    }
    info!(
        "Finished scanning. Found {} processable songs.",
        all_songs.len()
    );
    all_songs.sort_by(|a, b| {
        let pack_a_osstr = a
            .folder_path
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default();
        let pack_b_osstr = b
            .folder_path
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default();
        let pack_a = pack_a_osstr.to_string_lossy();
        let pack_b = pack_b_osstr.to_string_lossy();
        pack_a
            .cmp(&pack_b)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    all_songs
}
impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> Self {
        ParseError::Io(err)
    }
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "IO Error: {}", e),
            ParseError::NotFound(path) => write!(f, "File Not Found: {:?}", path),
            ParseError::UnsupportedExtension(ext) => {
                write!(f, "Unsupported file extension: {}", ext)
            }
            ParseError::Utf8Error { tag, source } => {
                write!(f, "UTF-8 decoding error for {}: {}", tag, source)
            }
            ParseError::InvalidFormat(desc) => write!(f, "Invalid Simfile Format: {}", desc),
            ParseError::MissingTag(tag) => write!(f, "Missing required Simfile tag: {}", tag),
            ParseError::NoCharts => write!(f, "No charts found in simfile"),
        }
    }
}

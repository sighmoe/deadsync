use super::stats::{ // ADDED
    minimize_chart_and_count, ArrowStats, BreakdownMode,
    compute_stream_counts, generate_breakdown, StreamCounts, // RunDensity not directly used here but could be
};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str;
use std::fmt;

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
    pub stream_counts: StreamCounts,
    pub breakdown_detailed: String,
    pub breakdown_simplified: String,
}

#[derive(Debug, Clone)]
pub struct ChartInfo {
    pub stepstype: String,
    pub description: String,
    pub difficulty: String,
    pub meter: String,
    pub credit: String,
    pub notes_data_raw: String, // Renamed from notes_data
    pub bpms_chart: Option<String>,
    pub stops_chart: Option<String>,
    pub processed_data: Option<ProcessedChartData>, // ADDED
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
}

// This is YOUR ParseError
pub enum ParseError {
    Io(io::Error), // This variant is key
    NotFound(PathBuf),
    UnsupportedExtension(String),
    Utf8Error {
        tag: String,
        source: str::Utf8Error,
    },
    InvalidFormat(String),
    MissingTag(String),
    NoCharts,
}

fn clean_tag(tag_content: &str) -> String {
    tag_content
        .chars()
        .filter(|c| !c.is_control() && *c != '\u{200b}')
        .collect::<String>()
        .trim()
        .to_string()
}

fn parse_simple_tag<'a>(lines: &mut impl Iterator<Item = &'a str>, tag: &str) -> Option<String> {
    lines
        .find(|line| line.trim_start().starts_with(tag))
        .map(|line| line.trim_start()[tag.len()..].trim_end_matches(';').trim().to_string())
}

pub fn parse_bpms(bpm_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
    let mut bpms = Vec::new();
    for part in bpm_string.split(',') {
        let components: Vec<&str> = part.split('=').collect();
        if components.len() == 2 {
            let beat = components[0]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidFormat("#BPMS beat".to_string()))?;
            let bpm = components[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidFormat("#BPMS value".to_string()))?;
            if bpm <= 0.0 {
                 warn!("Ignoring non-positive BPM value: {} at beat {}", bpm, beat);
                 continue;
            }
            bpms.push((beat, bpm));
        } else {
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
            let beat = components[0]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidFormat("#STOPS beat".to_string()))?;
            let duration = components[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidFormat("#STOPS duration".to_string()))?;
             if duration <= 0.0 {
                 warn!("Ignoring non-positive STOPS duration value: {} at beat {}", duration, beat);
                 continue;
             }
            stops.push((beat, duration));
        } else {
            warn!("Malformed STOPS segment: '{}', skipping.", part);
        }
    }
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(stops)
}


fn parse_simfile_content(content: &str, simfile_path: &Path) -> Result<SongInfo, ParseError> {
    // ... (Existing logic to parse header tags like #TITLE, #ARTIST, #BPMS, etc.) ...
    // ... (This part remains mostly the same, collecting raw chart strings) ...
    let mut title = String::new();
    let mut subtitle = String::new();
    let mut artist = String::new();
    let mut title_translit = String::new();
    let mut subtitle_translit = String::new();
    let mut artist_translit = String::new();
    let mut offset_str = String::from("0.0");
    let mut bpms_str = String::new();
    let mut stops_str = String::new();
    let mut audio_filename: Option<String> = None;
    let mut banner_filename: Option<String> = None;
    let mut charts_raw_tuples = Vec::new(); // Stores (HashMap<String, String>, notes_data_string, bpms_string_opt, stops_string_opt)

    let mut current_chart_map_opt: Option<HashMap<String, String>> = None;

    for line_untrimmed in content.lines() {
        let trimmed_line = line_untrimmed.trim();
        if trimmed_line.is_empty() || trimmed_line.starts_with("//") {
            continue;
        }

        if trimmed_line.starts_with('#') {
            // End of a multi-line #NOTES block for .sm files
            if trimmed_line == ";" && current_chart_map_opt.is_some() {
                if let Some(mut chart_map) = current_chart_map_opt.take() {
                    if chart_map.contains_key("#NOTES") { // Ensure #NOTES was started
                        let notes_data_str = chart_map.remove("#NOTES").unwrap_or_default();
                        let chart_bpms_str = chart_map.remove("#BPMS");
                        let chart_stops_str = chart_map.remove("#STOPS");
                        charts_raw_tuples.push((chart_map, notes_data_str, chart_bpms_str, chart_stops_str));
                    } else {
                        warn!("Chart definition ended with ';' but no #NOTES content was accumulated in {:?}, skipping chart.", simfile_path);
                    }
                }
                continue;
            }

            let parts: Vec<&str> = trimmed_line.splitn(2, ':').collect();
            if parts.len() == 2 {
                let tag_raw = parts[0];
                let tag = tag_raw.to_uppercase();
                let value = parts[1].trim_end_matches(';').trim();

                if let Some(ref mut chart_map_mut) = current_chart_map_opt {
                    // We are inside a chart definition
                    chart_map_mut.insert(tag.clone(), value.to_string());
                    if tag == "#NOTES" && value.contains(';') { // SSC single-line #NOTES or end of notes
                        let notes_value_cleaned = value.trim_end_matches(';').trim();
                        chart_map_mut.insert(tag.clone(), notes_value_cleaned.to_string()); // update with cleaned value

                        let final_chart_map = current_chart_map_opt.take().unwrap(); // We know it's Some
                        let notes_data_str = final_chart_map.get("#NOTES").cloned().unwrap_or_default(); // Should exist
                        let chart_bpms_str = final_chart_map.get("#BPMS").cloned();
                        let chart_stops_str = final_chart_map.get("#STOPS").cloned();
                        // Create a new map for metadata, excluding #NOTES, #BPMS, #STOPS
                        let mut metadata_map = final_chart_map;
                        metadata_map.remove("#NOTES");
                        metadata_map.remove("#BPMS");
                        metadata_map.remove("#STOPS");
                        charts_raw_tuples.push((metadata_map, notes_data_str, chart_bpms_str, chart_stops_str));
                    } else if tag == "#NOTES" {
                        // Start of a multi-line .sm #NOTES, value is usually empty after colon or contains first few fields.
                        // The actual note data will follow on subsequent lines.
                        // Store the partial value, it will be appended.
                         chart_map_mut.insert(tag.clone(), value.to_string());
                    }
                } else {
                    // Header tags
                    match tag.as_str() {
                        "#TITLE" => title = clean_tag(value),
                        "#SUBTITLE" => subtitle = clean_tag(value),
                        "#ARTIST" => artist = clean_tag(value),
                        "#TITLETRANSLIT" => title_translit = clean_tag(value),
                        "#SUBTITLETRANSLIT" => subtitle_translit = clean_tag(value),
                        "#ARTISTTRANSLIT" => artist_translit = clean_tag(value),
                        "#OFFSET" => offset_str = value.to_string(),
                        "#BPMS" => bpms_str = value.to_string(),
                        "#STOPS" => stops_str = value.to_string(),
                        "#MUSIC" => audio_filename = Some(value.to_string()),
                        "#BANNER" => banner_filename = Some(value.to_string()),
                        "#NOTES" | "#NOTEDATA" => { // Start of a new chart
                            if current_chart_map_opt.is_some() {
                                warn!("Unexpected start of new chart block ('{}') before previous one finished in {:?}. Discarding previous.", tag_raw, simfile_path);
                            }
                            let mut new_chart_map = HashMap::new();
                            if (tag == "#NOTES" || tag == "#NOTEDATA") && !value.is_empty() && value.contains(':') { // Likely .sm chart definition line
                                let note_parts : Vec<&str> = value.split(':').collect();
                                if note_parts.len() >= 5 { // StepsType:Description:Difficulty:Meter:GrooveRadarAndNotes
                                    new_chart_map.insert("#STEPSTYPE".to_string(), note_parts[0].trim().to_string());
                                    new_chart_map.insert("#DESCRIPTION".to_string(), note_parts[1].trim().to_string());
                                    new_chart_map.insert("#DIFFICULTY".to_string(), note_parts[2].trim().to_string());
                                    new_chart_map.insert("#METER".to_string(), note_parts[3].trim().to_string());
                                    // new_chart_map.insert("#CREDIT".to_string(), ???); // .sm doesn't have #CREDIT in this line.
                                    // The actual note data is after the 5th colon (index 4 of parts split by ':')
                                    // This will be handled by subsequent lines if it's multi-line.
                                    // For now, just initialize #NOTES field.
                                    if note_parts.len() >= 6 { // if there's content after radar
                                        let notes_content = note_parts[5..].join(":");
                                        new_chart_map.insert("#NOTES".to_string(), notes_content.trim().to_string());
                                    } else {
                                        new_chart_map.insert("#NOTES".to_string(), String::new()); // Init empty for appending
                                    }
                                } else {
                                    warn!("Malformed legacy #NOTES tag line in {:?}: {}", simfile_path, value);
                                    // Don't set current_chart_map_opt if malformed
                                    continue;
                                }
                            } else { // SSC style or empty #NOTES:
                               new_chart_map.insert("#NOTES".to_string(), String::new()); // Init empty for appending
                            }
                            current_chart_map_opt = Some(new_chart_map);
                        }
                        _ => {} // Unknown header tag
                    }
                }
            }
        } else if let Some(ref mut chart_map_mut_ref) = current_chart_map_opt {
            // This line is part of a multi-line #NOTES block (typically .sm)
            if let Some(notes_val_mut) = chart_map_mut_ref.get_mut("#NOTES") {
                if !notes_val_mut.is_empty() { // Add newline if not the first line of notes
                    notes_val_mut.push('\n');
                }
                notes_val_mut.push_str(trimmed_line); // Append the whole trimmed line
            } else {
                // This case should ideally not be hit if #NOTES was initialized
                warn!("Appending to non-existent #NOTES field in chart in {:?}, line: '{}'", simfile_path, trimmed_line);
            }
        }
    }

    // Finalize any pending chart that might not have ended with ';' (e.g. EOF)
    if let Some(mut chart_map) = current_chart_map_opt.take() {
        warn!("Simfile {:?} ended mid-chart definition. Attempting to finalize.", simfile_path);
        if chart_map.contains_key("#NOTES") {
            let notes_data_str = chart_map.remove("#NOTES").unwrap_or_default();
            let chart_bpms_str = chart_map.remove("#BPMS");
            let chart_stops_str = chart_map.remove("#STOPS");
            charts_raw_tuples.push((chart_map, notes_data_str, chart_bpms_str, chart_stops_str));
        } else {
            warn!("Chart definition at EOF in {:?} had no #NOTES content, skipping.", simfile_path);
        }
    }


    let offset = offset_str.parse::<f32>()
        .map_err(|e| ParseError::InvalidFormat(format!("#OFFSET ('{}'): {}", offset_str, e)))?;
    let bpms_header = parse_bpms(&bpms_str)?;
    let stops_header = parse_stops(&stops_str)?;

    let mut charts = Vec::new();
    for (metadata_map, notes_data_raw_str, chart_bpms_str_opt, chart_stops_str_opt) in charts_raw_tuples {
        let chart_info = ChartInfo {
            stepstype: metadata_map.get("#STEPSTYPE").cloned().unwrap_or_default(),
            description: metadata_map.get("#DESCRIPTION").cloned().unwrap_or_default(),
            difficulty: metadata_map.get("#DIFFICULTY").cloned().unwrap_or_default(),
            meter: metadata_map.get("#METER").cloned().unwrap_or_default(),
            credit: metadata_map.get("#CREDIT").cloned().unwrap_or_default(),
            notes_data_raw: notes_data_raw_str,
            bpms_chart: chart_bpms_str_opt,
            stops_chart: chart_stops_str_opt,
            processed_data: None, // Will be filled later
        };
        if chart_info.stepstype.is_empty() || chart_info.difficulty.is_empty() || chart_info.meter.is_empty() || chart_info.notes_data_raw.trim().is_empty() {
            warn!("Skipping chart in {:?} (type: '{}', diff: '{}', meter: '{}') due to missing essential fields or empty notes.",
                simfile_path, chart_info.stepstype, chart_info.difficulty, chart_info.meter);
            continue;
        }
        charts.push(chart_info);
    }

    if title.is_empty() { return Err(ParseError::MissingTag("#TITLE".to_string())); } // Explicitly qualify
    if bpms_header.is_empty() && charts.iter().all(|c| c.bpms_chart.as_deref().unwrap_or("").trim().is_empty()) {
        if charts.is_empty() { return Err(ParseError::MissingTag("#BPMS".to_string())); } // Explicitly qualify
        else {
            warn!("Simfile {:?} has no #BPMS defined in the header or any chart.", simfile_path);
            return Err(ParseError::MissingTag("#BPMS".to_string())); // Explicitly qualify
        }
    }
    if charts.is_empty() { return Err(ParseError::NoCharts); } // Explicitly qualify

    let folder_path = simfile_path.parent()
        .ok_or_else(|| ParseError::Io(io::Error::new(io::ErrorKind::InvalidInput, "Invalid simfile path")))? // Remove semicolon here
        .to_path_buf(); // Now this can chain
    let audio_path = audio_filename.map(|f| folder_path.join(f)).filter(|p| p.exists());
    let banner_path = banner_filename.map(|f| folder_path.join(f)).filter(|p| p.exists());

    Ok(SongInfo {
        title, subtitle, artist, title_translit, subtitle_translit, artist_translit,
        offset, bpms_header, stops_header, charts,
        simfile_path: simfile_path.to_path_buf(), folder_path, audio_path, banner_path,
    })
}


fn parse_minimized_bytes_to_measures(minimized_bytes: &[u8]) -> Vec<Vec<NoteLine>> {
    let mut all_measures: Vec<Vec<NoteLine>> = Vec::new();
    let mut current_measure_lines: Vec<NoteLine> = Vec::new();

    // Assuming minimized_bytes contains lines separated by '\n', and measures by ",\n"
    // Splitting by '\n' will give individual lines or comma for separator
    for line_segment in minimized_bytes.split(|&b| b == b'\n') {
        if line_segment.is_empty() {
            continue; // Skip empty segments that might arise from multiple newlines
        }
        if line_segment == b"," { // Measure separator
            if !current_measure_lines.is_empty() {
                all_measures.push(std::mem::take(&mut current_measure_lines));
            }
            // New measure starts after this, current_measure_lines is now empty
        } else if line_segment.starts_with(b"//") {
            // Comment line, ignore
        } else if line_segment.len() >= 4 {
            // Assumed to be a note data line (e.g., "1000", "0M00")
            let mut note_line_arr: NoteLine = [NoteChar::Empty; 4];
            for (i, &byte_char) in line_segment.iter().take(4).enumerate() {
                note_line_arr[i] = NoteChar::from(byte_char);
            }
            current_measure_lines.push(note_line_arr);
        }
        // Silently ignore other malformed lines for robustness
    }

    // Add the last measure if it has any lines and wasn't followed by a separator
    if !current_measure_lines.is_empty() {
        all_measures.push(current_measure_lines);
    }

    all_measures
}

pub fn parse_simfile(simfile_path: &Path) -> Result<SongInfo, ParseError> { // Explicitly qualify
    info!("Parsing simfile: {:?}", simfile_path);
    let content_bytes = fs::read(simfile_path)?;

    let mut song_info_result = match str::from_utf8(&content_bytes) {
        Ok(content_utf8) => parse_simfile_content(content_utf8, simfile_path),
        Err(e) => {
            warn!("UTF-8 decoding failed for {:?}, trying latin1: {}", simfile_path, e);
            let content_latin1: String = content_bytes.iter().map(|&byte| byte as char).collect();
            match parse_simfile_content(&content_latin1, simfile_path) {
                Ok(info) => {
                    warn!("Successfully parsed {:?} using latin1 fallback.", simfile_path);
                    Ok(info)
                }
                Err(parse_err) => {
                    error!("Failed to parse {:?} even with latin1 fallback: {}", simfile_path, parse_err);
                    Err(ParseError::Utf8Error { // Explicitly qualify
                        tag: "file content".to_string(),
                        source: e,
                    })
                }
            }
        }
    };

    if let Ok(ref mut song_info) = song_info_result {
        for chart in song_info.charts.iter_mut() {
            if chart.notes_data_raw.trim().is_empty() {
                warn!("Chart (type: '{}', difficulty: '{}') in {:?} has empty notes data, skipping processing.",
                    chart.stepstype, chart.difficulty, simfile_path);
                // Populate with default empty ProcessedChartData to avoid Option::unwrap() issues later
                chart.processed_data = Some(ProcessedChartData::default());
                continue;
            }

            let (minimized_bytes, stats, measure_densities) =
                minimize_chart_and_count(chart.notes_data_raw.as_bytes());

            let measures = parse_minimized_bytes_to_measures(&minimized_bytes);
            let stream_counts = compute_stream_counts(&measure_densities);
            let breakdown_detailed = generate_breakdown(&measure_densities, BreakdownMode::Detailed);
            let breakdown_simplified = generate_breakdown(&measure_densities, BreakdownMode::Simplified);

            chart.processed_data = Some(ProcessedChartData {
                measures,
                stats,
                measure_densities,
                stream_counts,
                breakdown_detailed,
                breakdown_simplified,
            });
            debug!("Processed chart (type: '{}', difficulty: '{}') for song '{}': {} measures, {} arrows. Detailed Breakdown: [{}], Simplified: [{}]",
                chart.stepstype, chart.difficulty, song_info.title,
                chart.processed_data.as_ref().unwrap().measures.len(),
                chart.processed_data.as_ref().unwrap().stats.total_arrows,
                chart.processed_data.as_ref().unwrap().breakdown_detailed,
                chart.processed_data.as_ref().unwrap().breakdown_simplified,
            );
        }
    }
    song_info_result
}


pub fn parse_song_folder(folder_path: &Path) -> Result<SongInfo, ParseError> {
    debug!("Scanning song folder: {:?}", folder_path);
    let mut ssc_path: Option<PathBuf> = None;
    let mut sm_path: Option<PathBuf> = None;

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match ext.to_lowercase().as_str() {
                    "ssc" => ssc_path = Some(path),
                    "sm" => sm_path = Some(path),
                    _ => {}
                }
            }
        }
    }

    if let Some(path) = ssc_path {
        parse_simfile(&path)
    } else if let Some(path) = sm_path {
        parse_simfile(&path)
    } else {
        Err(ParseError::NotFound(folder_path.join("*.ssc/*.sm"))) // Explicitly qualify
    }
}

pub fn scan_pack(pack_path: &Path) -> Vec<Result<SongInfo, ParseError>> {
    info!("Scanning pack: {:?}", pack_path);
    let mut songs = Vec::new();
    match fs::read_dir(pack_path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        songs.push(parse_song_folder(&path));
                    }
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
    match fs::read_dir(packs_root_dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let pack_songs_results = scan_pack(&path);
                        for result in pack_songs_results {
                            match result {
                                Ok(song_info) => {
                                    if song_info.charts.is_empty() {
                                        warn!("Song '{}' parsed successfully but has no charts, skipping.", song_info.title);
                                    } else if song_info.charts.iter().all(|c| c.processed_data.is_none() || c.processed_data.as_ref().unwrap().measures.is_empty() && c.processed_data.as_ref().unwrap().stats.total_arrows == 0) {
                                        warn!("Song '{}' parsed, but all its charts resulted in empty processed data or no arrows, skipping.", song_info.title);
                                    }
                                    else {
                                        debug!("Successfully parsed and processed song: {}", song_info.title);
                                        all_songs.push(song_info);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse song: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to read root songs directory {:?}: {}", packs_root_dir, e);
        }
    }
    info!("Finished scanning. Found {} processable songs.", all_songs.len());
    all_songs
}

impl From<io::Error> for ParseError { // This is the implementation
    fn from(err: io::Error) -> Self {
        ParseError::Io(err) // This looks correct
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
                write!(f, "UTF-8 decoding error in tag '{}': {}", tag, source)
            }
            ParseError::InvalidFormat(tag) => write!(f, "Invalid format for tag '{}'", tag),
            ParseError::MissingTag(tag) => write!(f, "Missing required tag: {}", tag),
            ParseError::NoCharts => write!(f, "No charts found in simfile"),
        }
    }
}
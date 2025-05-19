// deadsync/src/parsing/simfile.rs
use super::stats::{
    minimize_chart_and_count, ArrowStats, BreakdownMode,
    compute_stream_counts, generate_breakdown, StreamCounts,
};
use super::parse; // Using the module name 'parse' as you mentioned

use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str;
use std::fmt;

// Structs and Enums (NoteChar, NoteLine, ProcessedChartData, ChartInfo, SongInfo, ParseError)
// remain the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteChar { Empty = b'0' as isize, Tap = b'1' as isize, HoldStart = b'2' as isize, HoldEnd = b'3' as isize, RollStart = b'4' as isize, Mine = b'M' as isize, Lift = b'L' as isize, Fake = b'F' as isize, Unsupported }
impl From<u8> for NoteChar { 
    fn from(byte: u8) -> Self {
        match byte {
            b'0' => NoteChar::Empty, b'1' => NoteChar::Tap, b'2' => NoteChar::HoldStart, b'3' => NoteChar::HoldEnd,
            b'4' => NoteChar::RollStart, b'M' => NoteChar::Mine, b'L' => NoteChar::Lift, b'F' => NoteChar::Fake,
            _ => NoteChar::Unsupported,
        }
    }
}
pub type NoteLine = [NoteChar; 4];
#[derive(Debug, Clone, Default)]
pub struct ProcessedChartData { pub measures: Vec<Vec<NoteLine>>, pub stats: ArrowStats, pub measure_densities: Vec<usize>, pub stream_counts: StreamCounts, pub breakdown_detailed: String, pub breakdown_simplified: String }
#[derive(Debug, Clone)]
pub struct ChartInfo { pub stepstype: String, pub description: String, pub difficulty: String, pub meter: String, pub credit: String, pub notes_data_raw: String, pub bpms_chart: Option<String>, pub stops_chart: Option<String>, pub processed_data: Option<ProcessedChartData>, pub calculated_length_sec: Option<f32> }
#[derive(Debug, Clone)]
pub struct SongInfo { pub title: String, pub subtitle: String, pub artist: String, pub title_translit: String, pub subtitle_translit: String, pub artist_translit: String, pub offset: f32, pub bpms_header: Vec<(f32, f32)>, pub stops_header: Vec<(f32, f32)>, pub charts: Vec<ChartInfo>, pub simfile_path: PathBuf, pub folder_path: PathBuf, pub audio_path: Option<PathBuf>, pub banner_path: Option<PathBuf>, pub sample_start: Option<f32>, pub sample_length: Option<f32> }
#[derive(Debug)]
pub enum ParseError { Io(io::Error), NotFound(PathBuf), UnsupportedExtension(String), Utf8Error { tag: String, source: str::Utf8Error }, InvalidFormat(String), MissingTag(String), NoCharts }


pub fn parse_bpms(bpm_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
    let mut bpms = Vec::new();
    if bpm_string.trim().is_empty() { return Ok(bpms); }
    for part in bpm_string.split(',') {
        let components: Vec<&str> = part.split('=').collect();
        if components.len() == 2 {
            let beat = components[0].trim().parse::<f32>().map_err(|_| ParseError::InvalidFormat(format!("BPM beat value: '{}'", components[0])))?;
            let bpm = components[1].trim().parse::<f32>().map_err(|_| ParseError::InvalidFormat(format!("BPM value: '{}'", components[1])))?;
            if bpm <= 0.0 { warn!("Ignoring non-positive BPM value: {} at beat {}", bpm, beat); continue; }
            bpms.push((beat, bpm));
        } else if !part.trim().is_empty() { warn!("Malformed BPM segment: '{}', skipping.", part); }
    }
    bpms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(bpms)
}

pub fn parse_stops(stop_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
    let mut stops = Vec::new();
    if stop_string.trim().is_empty() { return Ok(stops); }
    for part in stop_string.split(',') {
        let components: Vec<&str> = part.split('=').collect();
        if components.len() == 2 {
            let beat = components[0].trim().parse::<f32>().map_err(|_| ParseError::InvalidFormat(format!("Stop beat value: '{}'", components[0])))?;
            let duration = components[1].trim().parse::<f32>().map_err(|_| ParseError::InvalidFormat(format!("Stop duration value: '{}'", components[1])))?;
            if duration <= 0.0 { warn!("Ignoring non-positive STOPS duration value: {} at beat {}", duration, beat); continue; }
            stops.push((beat, duration));
        } else if !part.trim().is_empty() { warn!("Malformed STOPS segment: '{}', skipping.", part); }
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
            // Comments should be stripped by minimize_chart_and_count if it's based on rssp
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

// Helper to find a specific tag value in the remaining data_bytes
// This is different from parse::parse_tag as it searches the whole slice.
fn find_tag_value<'a>(data: &'a [u8], tag_to_find: &[u8]) -> Option<&'a [u8]> {
    let mut i = 0;
    while i < data.len() {
        if let Some(pos) = data[i..].iter().position(|&b| b == b'#') {
            i += pos; // Move to the '#'
            if data[i..].starts_with(tag_to_find) {
                // Found the tag, now extract its value until ';'
                let value_start = i + tag_to_find.len();
                return data.get(value_start..)
                           .and_then(|d_slice| d_slice.iter().position(|&b| b == b';').map(|end| &d_slice[..end]));
            }
            i += 1; // Move past the current '#' to continue search
        } else {
            break; // No more '#' found
        }
    }
    None
}


pub fn parse_simfile(simfile_path: &Path) -> Result<SongInfo, ParseError> {
    info!("Parsing simfile: {:?}", simfile_path);
    let content_bytes_vec = fs::read(simfile_path)?;
    
    let extension_str = simfile_path.extension().and_then(|os_str| os_str.to_str()).unwrap_or("").to_lowercase();

    // Call rssp's extract_sections
    let (
        title_bytes_opt, 
        subtitle_bytes_opt, 
        artist_bytes_opt,
        titletranslit_bytes_opt, 
        subtitletranslit_bytes_opt, 
        artisttranslit_bytes_opt,
        offset_bytes_opt, 
        bpms_header_bytes_opt, 
        // `extract_sections` from rssp/parse.rs returns Vec<(Vec<u8>, Option<Vec<u8>>)> as the 9th element
        // It does NOT return STOPS, MUSIC, BANNER etc. directly in this tuple.
        raw_charts_list, 
    ) = parse::extract_sections(&content_bytes_vec, &extension_str)
        .map_err(|e| ParseError::InvalidFormat(format!("Failed to extract sections from {:?}: {}", simfile_path, e)))?;

    // Manually parse other header tags not covered by rssp's extract_sections' main tuple
    let stops_header_bytes_opt = find_tag_value(&content_bytes_vec, b"#STOPS:");
    let music_path_bytes_opt = find_tag_value(&content_bytes_vec, b"#MUSIC:");
    let banner_path_bytes_opt = find_tag_value(&content_bytes_vec, b"#BANNER:");
    let samplestart_bytes_opt = find_tag_value(&content_bytes_vec, b"#SAMPLESTART:");
    let samplelength_bytes_opt = find_tag_value(&content_bytes_vec, b"#SAMPLELENGTH:");


    let bytes_to_string_cleaned = |opt_bytes: Option<&[u8]>| {
        opt_bytes.and_then(|b| str::from_utf8(b).ok().map(parse::clean_tag)).unwrap_or_default()
    };
    let bytes_to_string_trimmed = |opt_bytes: Option<&[u8]>| {
        opt_bytes.and_then(|b| str::from_utf8(b).ok().map(|s| s.trim().to_string())).unwrap_or_default()
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
    let stops_header_str = bytes_to_string_trimmed(stops_header_bytes_opt); // From manual find
    let audio_filename = bytes_to_opt_string_trimmed(music_path_bytes_opt); // From manual find
    let banner_filename = bytes_to_opt_string_trimmed(banner_path_bytes_opt); // From manual find
    let sample_start_str = bytes_to_opt_string_trimmed(samplestart_bytes_opt); // From manual find
    let sample_length_str = bytes_to_opt_string_trimmed(samplelength_bytes_opt); // From manual find


    let offset = offset_str.parse::<f32>().unwrap_or(0.0); 
    let bpms_header = parse_bpms(&bpms_header_str)?;
    let stops_header = parse_stops(&stops_header_str)?;
    let sample_start = sample_start_str.as_deref().and_then(|s| s.parse::<f32>().ok());
    let sample_length = sample_length_str.as_deref().and_then(|s| s.parse::<f32>().ok());

    let mut charts: Vec<ChartInfo> = Vec::new();
    // raw_charts_list is Vec<(Vec<u8>, Option<Vec<u8>>)>
    // The Option<Vec<u8>> is for SSC chart-specific BPMs. SM files will have None here.
    for (chart_content_block_bytes, ssc_chart_bpms_bytes_opt) in raw_charts_list {
        let (metadata_fields_byte_slices, actual_notes_data_bytes) = 
            parse::split_notes_fields(&chart_content_block_bytes);

        if metadata_fields_byte_slices.len() < 5 {
            warn!("Chart in {:?} has incomplete metadata fields after split_notes_fields (found {}), skipping.", simfile_path, metadata_fields_byte_slices.len());
            continue;
        }

        let stepstype = String::from_utf8_lossy(metadata_fields_byte_slices[0]).trim().to_string();
        let description = String::from_utf8_lossy(metadata_fields_byte_slices[1]).trim().to_string();
        let difficulty = String::from_utf8_lossy(metadata_fields_byte_slices[2]).trim().to_string();
        let meter = String::from_utf8_lossy(metadata_fields_byte_slices[3]).trim().to_string();
        
        let credit_or_radar_sm = String::from_utf8_lossy(metadata_fields_byte_slices[4]).trim().to_string();
        
        let credit = if extension_str == "ssc" {
            credit_or_radar_sm // In rssp's SSC processing, this field becomes credit
        } else {
            String::new() // SM doesn't put credit here in this way
        };
        
        let notes_data_raw = String::from_utf8_lossy(actual_notes_data_bytes).to_string();
        
        let chart_bpms = ssc_chart_bpms_bytes_opt.map(|b| String::from_utf8_lossy(&b).trim().to_string());
        // Chart-specific stops for SSC would need to be parsed from within the ssc_block_data if present
        // rssp's `process_ssc_notedata` doesn't return it separately from extract_sections's notes_list.
        // It would be a subtag like #STOPS: inside the #NOTEDATA block.
        // For now, we'll assume chart_stops come from global or aren't parsed per-chart from SSC blocks yet.
        let chart_stops: Option<String> = None; // Placeholder, needs more robust SSC subtag parsing for this

        if stepstype.is_empty() || difficulty.is_empty() || meter.is_empty() {
             warn!("Skipping chart in {:?} (type: '{}', desc: '{}', diff: '{}', meter: '{}') due to missing essential metadata fields post-split.",
                simfile_path, stepstype, description, difficulty, meter);
            continue;
        }
        
        charts.push(ChartInfo {
            stepstype, description, difficulty, meter, credit,
            notes_data_raw,
            bpms_chart: chart_bpms,
            stops_chart: chart_stops, // Will be None for SM, potentially Some for SSC if parsed
            processed_data: None,
            calculated_length_sec: None,
        });
    }

    // Validation checks
    if title.is_empty() && !simfile_path.to_string_lossy().contains("EditCourses") {
        return Err(ParseError::MissingTag(format!("#TITLE in {:?}", simfile_path)));
    }
    if bpms_header.is_empty() && charts.iter().all(|c| c.bpms_chart.is_none()) {
         if !charts.is_empty() {
            return Err(ParseError::MissingTag(format!("#BPMS (neither header nor chart-specific, but charts exist) in {:?}", simfile_path)));
         }
    }
    if charts.is_empty() && !simfile_path.to_string_lossy().contains("EditCourses") {
        return Err(ParseError::NoCharts);
    }

    let folder_path = simfile_path.parent().ok_or_else(|| ParseError::Io(io::Error::new(io::ErrorKind::InvalidInput, "Invalid simfile path")))?.to_path_buf();
    let audio_path = audio_filename.map(|f| folder_path.join(f)).filter(|p| p.exists());
    let banner_path = banner_filename.map(|f| folder_path.join(f)).filter(|p| p.exists());

    let mut song_info = SongInfo {
        title, subtitle, artist, title_translit, subtitle_translit, artist_translit,
        offset, bpms_header, stops_header, charts,
        simfile_path: simfile_path.to_path_buf(), folder_path, audio_path, banner_path,
        sample_start, sample_length,
    };

    // Post-process charts (stats, duration)
    for chart_idx in 0..song_info.charts.len() {
        // Borrowing gymnastics to modify chart in place
        let (notes_data_raw_clone, stepstype_clone, difficulty_clone) = {
            let chart_ref = &song_info.charts[chart_idx];
            (chart_ref.notes_data_raw.clone(), chart_ref.stepstype.clone(), chart_ref.difficulty.clone())
        };
        
        if notes_data_raw_clone.trim().is_empty() {
            if let Some(chart_mut) = song_info.charts.get_mut(chart_idx) {
                warn!("Chart (type: '{}', difficulty: '{}') in {:?} has empty notes data string, assigning default processed data.",
                    chart_mut.stepstype, chart_mut.difficulty, simfile_path);
                chart_mut.processed_data = Some(ProcessedChartData::default());
                chart_mut.calculated_length_sec = Some(0.0); 
            }
            continue; 
        }

        let (minimized_bytes_for_measure_parsing, stats, measure_densities) =
            minimize_chart_and_count(notes_data_raw_clone.as_bytes()); 

        let measures = parse_minimized_bytes_to_measures(&minimized_bytes_for_measure_parsing);
        
        if measures.is_empty() && !notes_data_raw_clone.trim().is_empty() && (!measure_densities.is_empty() && measure_densities.iter().any(|&d| d > 0)) {
            warn!("Chart (type: '{}', difficulty: '{}') in {:?} resulted in zero measures after processing, though raw notes and densities were present. Raw notes length: {}. Minimized bytes length: {}. Densities (first 5): {:?}", 
                stepstype_clone, difficulty_clone, simfile_path, notes_data_raw_clone.len(), minimized_bytes_for_measure_parsing.len(), measure_densities.iter().take(5).collect::<Vec<_>>());
        }

        let stream_counts = compute_stream_counts(&measure_densities); 
        let breakdown_detailed = generate_breakdown(&measure_densities, BreakdownMode::Detailed); 
        let breakdown_simplified = generate_breakdown(&measure_densities, BreakdownMode::Simplified); 

        if let Some(chart_mut) = song_info.charts.get_mut(chart_idx) {
            chart_mut.processed_data = Some(ProcessedChartData {
                measures, stats, measure_densities, stream_counts,
                breakdown_detailed, breakdown_simplified,
            });
        
            let mut chart_specific_bpm_map_for_duration = song_info.bpms_header.clone();
            if let Some(chart_bpms_str) = &chart_mut.bpms_chart { 
                if !chart_bpms_str.trim().is_empty() {
                    match parse_bpms(chart_bpms_str) { 
                        Ok(parsed_chart_bpms) => {
                            if !parsed_chart_bpms.is_empty() { 
                                chart_specific_bpm_map_for_duration = parsed_chart_bpms;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse chart-specific BPMs for chart '{} {}' in {:?}: {}. Using header BPMs for duration.", chart_mut.stepstype, chart_mut.difficulty, simfile_path, e);
                        }
                    }
                }
            }
            chart_specific_bpm_map_for_duration.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            chart_specific_bpm_map_for_duration.dedup_by_key(|k| k.0);

            if let Some(pd) = &chart_mut.processed_data {
                if !chart_specific_bpm_map_for_duration.is_empty() {
                    chart_mut.calculated_length_sec = Some(calculate_chart_duration_seconds(pd, &chart_specific_bpm_map_for_duration));
                } else {
                    warn!("No BPMs available for chart '{} {}' in {:?}. Cannot calculate duration.", chart_mut.stepstype, chart_mut.difficulty, simfile_path);
                    chart_mut.calculated_length_sec = None;
                }
            }
            debug!("Processed chart (type: '{}', difficulty: '{}') for song '{}': {} measures, {} arrows.",
                chart_mut.stepstype, chart_mut.difficulty, song_info.title,
                chart_mut.processed_data.as_ref().map_or(0, |pd| pd.measures.len()),
                chart_mut.processed_data.as_ref().map_or(0, |pd| pd.stats.total_arrows)
            );
        }
    }
    
    song_info.charts.retain(|c| c.processed_data.as_ref().map_or(false, |pd| !pd.measures.is_empty() || pd.stats.total_arrows > 0));
    if song_info.charts.is_empty() && !song_info.simfile_path.to_string_lossy().contains("EditCourses") {
        return Err(ParseError::NoCharts);
    }

    Ok(song_info)
}


// get_bpm_at_beat, calculate_chart_duration_seconds, parse_song_folder, scan_pack, scan_packs
// From<io::Error> for ParseError, impl fmt::Display for ParseError
// remain the same as your last full simfile.rs
pub fn get_bpm_at_beat(bpm_map: &[(f32, f32)], beat: f32) -> f32 {
    let mut current_bpm = if !bpm_map.is_empty() { bpm_map[0].1 } else { 120.0 }; 
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
                                        debug!("Successfully parsed and processed song: {} from {:?}", song_info.title, song_info.simfile_path);
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
                    error!("Error reading entry in packs root {:?}: {}", packs_root_dir, e);
                }
            }
        }
        Err(e) => {
            error!("Failed to read root songs directory {:?}: {}", packs_root_dir, e);
        }
    }
    info!("Finished scanning. Found {} processable songs.", all_songs.len());
    all_songs.sort_by(|a, b| {
        let pack_a_osstr = a.folder_path.parent().and_then(|p| p.file_name()).unwrap_or_default();
        let pack_b_osstr = b.folder_path.parent().and_then(|p| p.file_name()).unwrap_or_default();
        let pack_a = pack_a_osstr.to_string_lossy();
        let pack_b = pack_b_osstr.to_string_lossy();
        
        pack_a.cmp(&pack_b).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
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
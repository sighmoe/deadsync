use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::fmt; // Import fmt
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str; // Import str for Utf8Error

#[derive(Debug, Clone)]
pub struct ChartInfo {
    pub stepstype: String,
    pub description: String,
    pub difficulty: String,
    pub meter: String,
    pub credit: String,
    pub notes_data: String,
    pub bpms_chart: Option<String>,
    pub stops_chart: Option<String>,
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

// Remove #[derive(thiserror::Error)]
// Keep #[derive(Debug)] for logging/debugging
#[derive(Debug)] // Don't derive Clone unless absolutely needed for errors
pub enum ParseError {
    // Remove #[error(...)] and #[from]
    Io(io::Error),
    NotFound(PathBuf),
    UnsupportedExtension(String),
    Utf8Error {
        tag: String,
        // Keep source for the Error trait impl
        source: str::Utf8Error,
    },
    InvalidFormat(String),
    MissingTag(String),
    NoCharts,
}

// --- Manual Trait Implementations for ParseError ---

// Implement Display for user-friendly messages
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

// Implement Error trait
impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Io(e) => Some(e),
            ParseError::Utf8Error { source, .. } => Some(source),
            _ => None, // Other variants don't wrap another error
        }
    }
}

// Implement From<io::Error> to allow use of `?` with IO results
impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> Self {
        ParseError::Io(err)
    }
}

// --- Parsing Functions ---

// clean_tag, parse_simple_tag, parse_bpms, parse_stops remain the same
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

fn parse_bpms(bpm_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
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

fn parse_stops(stop_string: &str) -> Result<Vec<(f32, f32)>, ParseError> {
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

// parse_simfile_content remains largely the same logic, but error handling for parse might change slightly
fn parse_simfile_content(content: &str, simfile_path: &Path) -> Result<SongInfo, ParseError> {
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
    let mut charts_raw = Vec::new();

    let mut current_chart: Option<HashMap<String, String>> = None;

    for line in content.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() || trimmed_line.starts_with("//") {
            continue;
        }

        if trimmed_line.starts_with('#') {
             if trimmed_line == ";" && current_chart.is_some() {
                 if let Some(mut chart_map) = current_chart.take() {
                      if let Some(notes) = chart_map.remove("NOTES") {
                          charts_raw.push((chart_map, notes, None, None));
                      } else {
                           warn!("Chart definition ended without #NOTES: tag in {:?}, skipping chart.", simfile_path);
                      }
                  }
                 continue;
             }

            let parts: Vec<&str> = trimmed_line.splitn(2, ':').collect();
            if parts.len() == 2 {
                let tag = parts[0].to_uppercase();
                let value = parts[1].trim_end_matches(';').trim();

                 if let Some(ref mut chart_map) = current_chart {
                    chart_map.insert(tag.clone(), value.to_string());
                     if tag == "#NOTES" && value.contains(';') {
                         let notes_value = value.trim_end_matches(';').trim();
                         chart_map.insert(tag.clone(), notes_value.to_string());
                         let bpms = chart_map.remove("#BPMS");
                         let stops = chart_map.remove("#STOPS");
                         if let Some(notes) = chart_map.remove("#NOTES") {
                             charts_raw.push((chart_map.clone(), notes, bpms, stops));
                          } else {
                             warn!("SSC chart ended without #NOTES tag in {:?}, skipping chart", simfile_path);
                          }
                         current_chart = None;
                     } else if tag == "#NOTES" {
                         chart_map.insert(tag.clone(), value.to_string());
                     }

                 } else {
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
                        "#NOTES" | "#NOTEDATA" => {
                            if current_chart.is_some() {
                                warn!("Unexpected start of new chart block before previous one finished in {:?}. Discarding previous.", simfile_path);
                            }
                            current_chart = Some(HashMap::new());
                             if tag=="#NOTES" && !value.is_empty() {
                                let note_parts : Vec<&str> = value.split(':').collect();
                                if note_parts.len() >= 6 {
                                    let chart_map = current_chart.as_mut().unwrap();
                                    chart_map.insert("#STEPSTYPE".to_string(), note_parts[0].trim().to_string());
                                    chart_map.insert("#DESCRIPTION".to_string(), note_parts[1].trim().to_string());
                                    chart_map.insert("#DIFFICULTY".to_string(), note_parts[2].trim().to_string());
                                    chart_map.insert("#METER".to_string(), note_parts[3].trim().to_string());
                                    let notes_content = note_parts[5..].join(":");
                                    chart_map.insert("#NOTES".to_string(), notes_content);
                                } else {
                                    warn!("Malformed legacy #NOTES tag line in {:?}: {}", simfile_path, value);
                                    current_chart = None;
                                }
                             }
                        }
                        _ => {}
                    }
                 }
            }
        } else if let Some(ref mut chart_map) = current_chart {
             if let Some(notes_val) = chart_map.get_mut("#NOTES") {
                 if trimmed_line.contains(';') {
                     notes_val.push_str("\n");
                     notes_val.push_str(trimmed_line.trim_end_matches(';').trim());
                     let bpms = chart_map.remove("#BPMS");
                     let stops = chart_map.remove("#STOPS");
                      if let Some(notes) = chart_map.remove("#NOTES") {
                          charts_raw.push((chart_map.clone(), notes, bpms, stops));
                      } else {
                          warn!("#NOTES tag disappeared unexpectedly during multi-line parsing in {:?}, skipping chart", simfile_path);
                      }
                     current_chart = None;
                 } else {
                     notes_val.push_str("\n");
                     notes_val.push_str(trimmed_line);
                 }
             }
        }
    }

      if let Some(mut chart_map) = current_chart.take() {
         warn!("Simfile {:?} ended mid-chart definition without a semicolon. Attempting to finalize.", simfile_path);
         let bpms = chart_map.remove("#BPMS");
         let stops = chart_map.remove("#STOPS");
         if let Some(notes) = chart_map.remove("#NOTES") {
             charts_raw.push((chart_map.clone(), notes, bpms, stops));
         } else {
             warn!("Chart definition ended without #NOTES tag at EOF in {:?}, skipping chart.", simfile_path);
         }
      }

    let offset = offset_str
        .parse::<f32>()
        // Use map_err to convert parse error to our InvalidFormat variant
        .map_err(|e| {
            error!("Failed to parse #OFFSET value '{}': {}", offset_str, e);
            ParseError::InvalidFormat(format!("#OFFSET ('{}')", offset_str))
        })?;

    let bpms_header = parse_bpms(&bpms_str)?;
    let stops_header = parse_stops(&stops_str)?;

    let mut charts = Vec::new();
    for (map, notes_data, chart_bpms_str, chart_stops_str) in charts_raw {
        let chart_info = ChartInfo {
            stepstype: map.get("#STEPSTYPE").cloned().unwrap_or_default(),
            description: map.get("#DESCRIPTION").cloned().unwrap_or_default(),
            difficulty: map.get("#DIFFICULTY").cloned().unwrap_or_default(),
            meter: map.get("#METER").cloned().unwrap_or_default(),
            credit: map.get("#CREDIT").cloned().unwrap_or_default(),
            notes_data,
             bpms_chart: chart_bpms_str,
             stops_chart: chart_stops_str,
        };
        if chart_info.stepstype.is_empty() || chart_info.difficulty.is_empty() || chart_info.meter.is_empty() || chart_info.notes_data.trim().is_empty() {
            warn!("Skipping chart in {:?} due to missing essential fields (type, difficulty, meter, or notes).", simfile_path);
            continue;
        }
        charts.push(chart_info);
    }

    if title.is_empty() {
        return Err(ParseError::MissingTag("#TITLE".to_string()));
    }
     if bpms_header.is_empty() && charts.iter().all(|c| c.bpms_chart.is_none()) {
         // Only error if NO bpms are defined anywhere
         if charts.is_empty() {
             // If no charts, it's definitely an error
            return Err(ParseError::MissingTag("#BPMS".to_string()));
         } else {
             // If charts exist but none have their own BPMs, it's still an error
             warn!("Simfile {:?} has no #BPMS defined in the header or any chart.", simfile_path);
             return Err(ParseError::MissingTag("#BPMS".to_string()));
         }
     }
    if charts.is_empty() {
        return Err(ParseError::NoCharts);
    }

    let folder_path = simfile_path
        .parent()
        // Convert Option<Path> error into our ParseError::Io
        .ok_or_else(|| ParseError::Io(io::Error::new(io::ErrorKind::InvalidInput, "Invalid simfile path")))?
        .to_path_buf();

    let audio_path = audio_filename
        .map(|f| folder_path.join(f))
        .filter(|p| p.exists());

    let banner_path = banner_filename
        .map(|f| folder_path.join(f))
        .filter(|p| p.exists());

    Ok(SongInfo {
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
    })
}

// parse_simfile remains the same logic, but now `?` works due to From<io::Error>
pub fn parse_simfile(simfile_path: &Path) -> Result<SongInfo, ParseError> {
    info!("Parsing simfile: {:?}", simfile_path);
    let content_bytes = fs::read(simfile_path)?; // `?` works now

    match str::from_utf8(&content_bytes) {
        Ok(content_utf8) => parse_simfile_content(content_utf8, simfile_path),
        Err(e) => {
            warn!("UTF-8 decoding failed for {:?}, trying latin1: {}", simfile_path, e);
            let content_latin1: String = content_bytes.iter().map(|&byte| byte as char).collect();
             match parse_simfile_content(&content_latin1, simfile_path) {
                 Ok(info) => {
                     warn!("Successfully parsed {:?} using latin1 fallback.", simfile_path);
                     Ok(info)
                 },
                 Err(parse_err) => {
                     // Log the parse error using Display trait we implemented
                     error!("Failed to parse {:?} even with latin1 fallback: {}", simfile_path, parse_err);
                      // Store the original UTF-8 error info
                      Err(ParseError::Utf8Error {
                          tag: "file content".to_string(),
                          source: e,
                      })
                 }
             }
        }
    }
}

// parse_song_folder remains the same logic, `?` works now
pub fn parse_song_folder(folder_path: &Path) -> Result<SongInfo, ParseError> {
    debug!("Scanning song folder: {:?}", folder_path);
    let mut ssc_path: Option<PathBuf> = None;
    let mut sm_path: Option<PathBuf> = None;

    for entry in fs::read_dir(folder_path)? { // `?` works now
        let entry = entry?; // `?` works now
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
        Err(ParseError::NotFound(folder_path.join("*.ssc/*.sm")))
    }
}

// scan_pack remains the same
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

// scan_packs remains the same, but error logging now uses Display
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
                                    debug!("Successfully parsed song: {}", song_info.title);
                                    all_songs.push(song_info);
                                }
                                Err(e) => {
                                    // Use Display impl for logging
                                    error!("Failed to parse song: {}", e);
                                }
                            }
                        }
                    }
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
    info!("Finished scanning. Found {} successfully parsed songs.", all_songs.len());
    all_songs
}
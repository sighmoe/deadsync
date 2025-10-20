use rssp::stats::ArrowStats;
use rssp::TechCounts;

#[derive(Clone, Debug, PartialEq)]
pub struct ChartData {
    pub chart_type: String,
    pub difficulty: String,
    pub meter: u32,
    pub step_artist: String,
    pub notes: Vec<u8>, // This is the minimized raw data we will parse
    pub short_hash: String,
    pub stats: ArrowStats,
    pub tech_counts: TechCounts,
    pub total_streams: u32,
    pub max_nps: f64,
    pub detailed_breakdown: String,
    pub partial_breakdown: String,
    pub simple_breakdown: String,
    pub total_measures: usize,
    pub measure_nps_vec: Vec<f64>,
}

// Define a public enum for the parsing result. This decouples the parser from the gameplay screen.
#[derive(Clone, Debug)]
pub enum NoteType {
    Tap,
    Hold,
    Roll,
}
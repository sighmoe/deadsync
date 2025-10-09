use crate::gameplay::chart::NoteType;
use log::info;

/// Parses the raw, minimized `#NOTES:` data block from `rssp`.
///
/// This function converts the byte representation of notes into a vector of
/// `(row_index, column, NoteType)`, leaving the conversion from row to beat
/// up to the `TimingData` module.
pub fn parse_chart_notes(raw_note_bytes: &[u8]) -> Vec<(usize, usize, NoteType)> {
    let mut notes = Vec::new();
    let mut row_index = 0;
    // Split by lines, also handling potential commas on their own lines
    for line in raw_note_bytes.split(|&b| b == b'\n') {
        let trimmed_line = line.strip_suffix(b"\r").unwrap_or(line);
        if trimmed_line.is_empty() || trimmed_line == b"," {
            continue;
        }

        if trimmed_line.len() >= 4 {
            for (col_index, &ch) in trimmed_line.iter().take(4).enumerate() {
                let note_type = match ch {
                    b'1' => Some(NoteType::Tap),
                    b'2' => Some(NoteType::Hold),
                    b'4' => Some(NoteType::Roll),
                    _ => None,
                };
                if let Some(nt) = note_type {
                    notes.push((row_index, col_index, nt));
                }
            }
        }
        row_index += 1;
    }

    info!("Pre-parsed {} notes from raw chart data.", notes.len());
    notes
}

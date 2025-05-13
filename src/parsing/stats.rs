// src/parsing/stats.rs

use std::collections::HashSet;
use log; // For potential logging within this module if needed

#[derive(Debug, Default, Clone, Copy)]
pub struct ArrowStats {
    pub total_arrows: u32,
    pub left: u32,
    pub down: u32,
    pub up: u32,
    pub right: u32,
    pub total_steps: u32, // Lines with at least one '1', '2', or '4'
    pub jumps: u32,       // Lines with 2+ '1'/'2'/'4's
    pub hands: u32,       // Lines with 3+ '1'/'2'/'4's or complex hold+tap situations
    pub mines: u32,
    pub holds: u32,       // Count of '2's
    pub rolls: u32,       // Count of '4's
    pub lifts: u32,
    pub fakes: u32,
    pub holding: i32, // Transient state: number of active holds/rolls during counting
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StreamCounts {
    pub run16_streams: u32,
    pub run20_streams: u32,
    pub run24_streams: u32,
    pub run32_streams: u32,
    pub total_breaks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunDensity {
    Run32,
    Run24,
    Run20,
    Run16,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakdownMode {
    Detailed,
    Partial,
    Simplified,
}

#[inline]
fn is_all_zero(line: &[u8; 4]) -> bool {
    // Check if all characters are ASCII '0' (0x30)
    u32::from_ne_bytes(*line) == 0x30303030
}

/// Minimizes measure lines if every other line is all-zero.
#[inline]
pub fn minimize_measure(measure: &mut Vec<[u8; 4]>) {
    while measure.len() >= 2 && measure.len() % 2 == 0 {
        if measure.iter().skip(1).step_by(2).any(|line| !is_all_zero(line)) {
            break;
        }
        let half_len = measure.len() / 2;
        for i in 0..half_len {
            measure[i] = measure[i * 2];
        }
        measure.truncate(half_len);
    }

    if !measure.is_empty() && measure.iter().all(is_all_zero) {
        measure.truncate(1);
        measure[0] = [b'0', b'0', b'0', b'0']; // Ensure it's actually zeros
    }
}

#[inline]
fn count_line(
    line: &[u8; 4],
    stats: &mut ArrowStats,
    holds_started_on_line: &mut u32,
    ends_seen_on_line: &mut u32,
) -> bool {
    let mut note_mask = 0u8;
    let mut hold_start_mask = 0u8;
    let mut end_mask = 0u8;
    let mut mine_count = 0u32;
    let mut lift_count = 0u32;
    let mut fake_count = 0u32;

    *holds_started_on_line = 0;
    *ends_seen_on_line = 0;

    for (i, &ch) in line.iter().enumerate() {
        match ch {
            b'1' | b'2' | b'4' => {
                note_mask |= 1 << i;
                if ch == b'2' || ch == b'4' {
                    hold_start_mask |= 1 << i;
                }
                match i {
                    0 => stats.left += 1,
                    1 => stats.down += 1,
                    2 => stats.up += 1,
                    3 => stats.right += 1,
                    _ => {}
                }
                stats.total_arrows += 1;
                if ch == b'2' {
                    stats.holds += 1;
                } else if ch == b'4' {
                    stats.rolls += 1;
                }
            }
            b'3' => end_mask |= 1 << i,
            b'M' => mine_count += 1,
            b'L' => lift_count += 1,
            b'F' => fake_count += 1,
            _ => {}
        }
    }

    stats.mines += mine_count;
    stats.lifts += lift_count;
    stats.fakes += fake_count;
    let notes_on_line_count = note_mask.count_ones();
    *holds_started_on_line = hold_start_mask.count_ones();
    *ends_seen_on_line = end_mask.count_ones();

    if notes_on_line_count == 0 {
        stats.holding = (stats.holding - *ends_seen_on_line as i32).max(0);
        return false;
    }

    stats.total_steps += 1;
    if notes_on_line_count >= 2 {
        stats.jumps += 1;
    }
    
    let prev_hands = stats.hands; // Store previous hands to avoid double counting below

    if notes_on_line_count >= 3 {
        stats.hands += 1;
    }

    let current_holding_val = stats.holding;
    if (current_holding_val == 1 && notes_on_line_count >= 2) || (current_holding_val >= 2 && notes_on_line_count >= 1) {
        // Only increment if it wasn't already counted as a 3-note hand
        if notes_on_line_count < 3 && stats.hands == prev_hands {
            stats.hands += 1;
        }
    }
    
    stats.holding = (stats.holding + *holds_started_on_line as i32 - *ends_seen_on_line as i32).max(0);
    true
}

pub fn minimize_chart_and_count(notes_data: &[u8]) -> (Vec<u8>, ArrowStats, Vec<usize>) {
    let mut output_bytes = Vec::with_capacity(notes_data.len());
    let mut current_measure_lines_for_minimization = Vec::<[u8; 4]>::with_capacity(192);

    let mut stats = ArrowStats::default();
    let mut measure_densities = Vec::new();
    let mut saw_semicolon = false;

    let mut all_lines_buffer_for_second_pass = Vec::new();
    let mut total_holds_started_overall = 0u32;
    let mut total_ends_seen_overall = 0u32;

    #[inline]
    fn finalize_measure_processing(
        measure_lines_for_min: &mut Vec<[u8; 4]>,
        output_buf: &mut Vec<u8>,
        current_stats: &mut ArrowStats,
        measure_density_list: &mut Vec<usize>,
        all_lines_buf_second_pass: &mut Vec<[u8; 4]>,
        total_holds_started_acc: &mut u32,
        total_ends_seen_acc: &mut u32,
    ) {
        if measure_lines_for_min.is_empty() {
            measure_density_list.push(0);
            return;
        }
        minimize_measure(measure_lines_for_min);
        output_buf.reserve(measure_lines_for_min.len() * 5);

        let mut current_measure_density = 0usize;
        let mut holds_on_line = 0u32;
        let mut ends_on_line = 0u32;

        for m_line in measure_lines_for_min.iter() {
            all_lines_buf_second_pass.push(*m_line);
            if count_line(m_line, current_stats, &mut holds_on_line, &mut ends_on_line) {
                current_measure_density += 1;
            }
            *total_holds_started_acc += holds_on_line;
            *total_ends_seen_acc += ends_on_line;
            output_buf.extend_from_slice(m_line);
            output_buf.push(b'\n');
        }
        measure_lines_for_min.clear();
        measure_density_list.push(current_measure_density);
    }

    for line_raw_bytes in notes_data.split(|&b| b == b'\n') {
        let first_char_idx = line_raw_bytes.iter().position(|&c| !c.is_ascii_whitespace());
        if let Some(start_idx) = first_char_idx {
            let line_content_bytes = &line_raw_bytes[start_idx..];
            if line_content_bytes.is_empty() { continue; }
            match line_content_bytes[0] {
                b',' => {
                    finalize_measure_processing(&mut current_measure_lines_for_minimization, &mut output_bytes, &mut stats, &mut measure_densities, &mut all_lines_buffer_for_second_pass, &mut total_holds_started_overall, &mut total_ends_seen_overall);
                    output_bytes.extend_from_slice(b",\n");
                }
                b';' => {
                    finalize_measure_processing(&mut current_measure_lines_for_minimization, &mut output_bytes, &mut stats, &mut measure_densities, &mut all_lines_buffer_for_second_pass, &mut total_holds_started_overall, &mut total_ends_seen_overall);
                    saw_semicolon = true;
                    break;
                }
                b'/' if line_content_bytes.len() > 1 && line_content_bytes[1] == b'/' => {} // Comment
                _ => {
                    if line_content_bytes.len() >= 4 {
                        let mut arr = [b'0'; 4];
                        let len_to_copy = line_content_bytes.len().min(4);
                        arr[..len_to_copy].copy_from_slice(&line_content_bytes[..len_to_copy]);
                        current_measure_lines_for_minimization.push(arr);
                    }
                }
            }
        }
    }

    if !saw_semicolon && !current_measure_lines_for_minimization.is_empty() {
        finalize_measure_processing(&mut current_measure_lines_for_minimization, &mut output_bytes, &mut stats, &mut measure_densities, &mut all_lines_buffer_for_second_pass, &mut total_holds_started_overall, &mut total_ends_seen_overall);
    }

    if output_bytes.ends_with(b",\n") { output_bytes.truncate(output_bytes.len() - 2); }
    if output_bytes.last() == Some(&b'\n') { output_bytes.pop(); }

    if total_holds_started_overall != total_ends_seen_overall {
        log::debug!("Chart has inconsistent hold/end counts ({} starts, {} ends). Running correction pass.", total_holds_started_overall, total_ends_seen_overall);
        let mut col_hold_start_indices: [Vec<usize>; 4] = Default::default();
        let mut phantom_hold_starts = HashSet::<(usize, usize)>::new();
        for (line_idx, line_chars) in all_lines_buffer_for_second_pass.iter().enumerate() {
            for (col_idx, &char_code) in line_chars.iter().enumerate() {
                match char_code {
                    b'2' | b'4' => col_hold_start_indices[col_idx].push(line_idx),
                    b'3' => { col_hold_start_indices[col_idx].pop(); },
                    _ => {}
                }
            }
        }
        for col_idx in 0..4 {
            for &line_idx in &col_hold_start_indices[col_idx] {
                phantom_hold_starts.insert((line_idx, col_idx));
            }
        }

        if !phantom_hold_starts.is_empty() {
            let mut corrected_lines_for_stats = all_lines_buffer_for_second_pass.clone();
            for (line_idx, col_idx) in phantom_hold_starts {
                if line_idx < corrected_lines_for_stats.len() && matches!(corrected_lines_for_stats[line_idx][col_idx], b'2' | b'4') {
                    corrected_lines_for_stats[line_idx][col_idx] = b'0';
                }
            }
            let mut corrected_stats = ArrowStats::default();
            // total_holds_started_overall and total_ends_seen_overall are not recounted here,
            // as the correction is about fixing the primary stats (arrows, holds, etc.)
            // based on invalid hold definitions. The original overall counts reflect the raw file.
            let mut temp_holds_on_line = 0u32;
            let mut temp_ends_on_line = 0u32;
            for line_chars in &corrected_lines_for_stats {
                count_line(line_chars, &mut corrected_stats, &mut temp_holds_on_line, &mut temp_ends_on_line);
            }
            stats = corrected_stats;
        }
    }
    (output_bytes, stats, measure_densities)
}

#[inline]
pub fn categorize_measure_density(density: usize) -> RunDensity {
    if density == 0 { RunDensity::Break }
    else if density >= 32 { RunDensity::Run32 }
    else if density >= 24 { RunDensity::Run24 }
    else if density >= 20 { RunDensity::Run20 }
    else if density >= 16 { RunDensity::Run16 }
    else { RunDensity::Break }
}

pub fn compute_stream_counts(measure_densities: &[usize]) -> StreamCounts {
    let mut stream_counts = StreamCounts::default();
    if measure_densities.is_empty() { return stream_counts; }
    let categories: Vec<RunDensity> = measure_densities.iter().map(|&d| categorize_measure_density(d)).collect();
    let first_run_idx = categories.iter().position(|&cat| cat != RunDensity::Break);
    let last_run_idx = categories.iter().rposition(|&cat| cat != RunDensity::Break);
    if let (Some(start_idx), Some(end_idx)) = (first_run_idx, last_run_idx) {
        if start_idx <= end_idx {
            for &category in &categories[start_idx..=end_idx] {
                match category {
                    RunDensity::Run16 => stream_counts.run16_streams += 1,
                    RunDensity::Run20 => stream_counts.run20_streams += 1,
                    RunDensity::Run24 => stream_counts.run24_streams += 1,
                    RunDensity::Run32 => stream_counts.run32_streams += 1,
                    RunDensity::Break => stream_counts.total_breaks += 1,
                }
            }
        }
    }
    stream_counts
}

#[derive(Debug, Clone, Copy)]
enum BreakdownToken {
    Run(RunDensity, usize),
    Break(usize),
}

pub fn generate_breakdown(measure_densities: &[usize], mode: BreakdownMode) -> String {
    if measure_densities.is_empty() { return String::new(); }
    let categories: Vec<RunDensity> = measure_densities.iter().map(|&d| categorize_measure_density(d)).collect();
    let first_active_idx = categories.iter().position(|&c| c != RunDensity::Break);
    let last_active_idx = categories.iter().rposition(|&c| c != RunDensity::Break);
    if first_active_idx.is_none() || last_active_idx.is_none() { return String::new(); }
    let active_categories = &categories[first_active_idx.unwrap()..=last_active_idx.unwrap()];
    let mut tokens: Vec<BreakdownToken> = Vec::new();
    if !active_categories.is_empty() {
        let mut current_cat = active_categories[0];
        let mut count = 0;
        for &cat in active_categories {
            if cat == current_cat { count += 1; }
            else {
                tokens.push(match current_cat { RunDensity::Break => BreakdownToken::Break(count), other => BreakdownToken::Run(other, count), });
                current_cat = cat; count = 1;
            }
        }
        tokens.push(match current_cat { RunDensity::Break => BreakdownToken::Break(count), other => BreakdownToken::Run(other, count), });
    }
    let break_merge_threshold = match mode { BreakdownMode::Detailed => 0, BreakdownMode::Partial => 1, BreakdownMode::Simplified => 4, };
    #[derive(Debug)] enum MergedToken { Run(RunDensity, usize, bool), Break(usize), }
    let mut merged_tokens: Vec<MergedToken> = Vec::new();
    let mut token_iter = tokens.into_iter().peekable();
    while let Some(token) = token_iter.next() {
        match token {
            BreakdownToken::Run(current_run_cat, mut current_run_len) => {
                let mut was_merged_with_break = false;
                // Peek for a break
                if let Some(BreakdownToken::Break(peeked_break_len)) = token_iter.peek().copied() { // .copied() to get usize
                    if peeked_break_len <= break_merge_threshold {
                        // Peek further for a run of the same type
                        let mut after_break_iter = token_iter.clone();
                        after_break_iter.next(); // Skip the break we just peeked

                        if let Some(BreakdownToken::Run(peeked_next_run_cat, peeked_next_run_len)) = after_break_iter.peek().copied() {
                            if peeked_next_run_cat == current_run_cat {
                                // We found a merge candidate! Consume from original iterator.
                                token_iter.next(); // Consume Break
                                token_iter.next(); // Consume Run
                                current_run_len += peeked_break_len + peeked_next_run_len;
                                was_merged_with_break = true;
                            } else if mode == BreakdownMode::Simplified && peeked_break_len > 1 && peeked_break_len <= break_merge_threshold {
                                // Simplified mode merge with just the break
                                token_iter.next(); // Consume Break
                                current_run_len += peeked_break_len;
                                was_merged_with_break = true;
                            }
                        } else if mode == BreakdownMode::Simplified && peeked_break_len > 1 && peeked_break_len <= break_merge_threshold {
                            // Simplified mode merge if break is followed by nothing or a different run type
                            token_iter.next(); // Consume Break
                            current_run_len += peeked_break_len;
                            was_merged_with_break = true;
                        }
                    }
                }
                merged_tokens.push(MergedToken::Run(current_run_cat, current_run_len, was_merged_with_break));
            }
            BreakdownToken::Break(break_len) => {
                // This break was not consumed by a preceding Run, so add it.
                merged_tokens.push(MergedToken::Break(break_len));
            }
        }
    }
    merged_tokens.into_iter().filter_map(|m_token| match m_token {
        MergedToken::Run(cat, len, star) => Some(format_run_symbol(cat, len, star)),
        MergedToken::Break(bk_len) => match mode {
            BreakdownMode::Detailed => { // Ensure this arm exists and handles bk_len
                if bk_len > 0 { Some(format!("({})", bk_len)) } else { None }
            }
            BreakdownMode::Partial => match bk_len { 
                1 => None, 
                2..=4 => Some("-".to_string()), 
                5..=32 => Some("/".to_string()), 
                _ => Some("|".to_string()), 
            },
            BreakdownMode::Simplified => match bk_len { 
                1..=4 => None, 
                5..=32 => Some("/".to_string()), 
                _ => Some("|".to_string()), 
            },
        },
    }).collect::<Vec<String>>().join(" ")
}

pub fn format_run_symbol(cat: RunDensity, length: usize, star: bool) -> String {
    let base_symbol = match cat {
        RunDensity::Run16 => format!("{}", length),
        RunDensity::Run20 => format!("~{}~", length),
        RunDensity::Run24 => format!(r"\{}/", length),
        RunDensity::Run32 => format!("={}=", length),
        RunDensity::Break => unreachable!(),
    };
    if star { format!("{}*", base_symbol) } else { base_symbol }
}
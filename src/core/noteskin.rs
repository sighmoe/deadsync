// FILE: src/core/noteskin.rs
use log::info;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::Path;

// The number of distinct quantizations a noteskin can define, from 4ths to 192nds.
pub const NUM_QUANTIZATIONS: usize = 9;

/// Represents the quantization of a note (e.g., 4th, 8th, 16th).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Quantization {
    Q4th = 0, Q8th, Q12th, Q16th, Q24th, Q32nd, Q48th, Q64th, Q192nd,
}

impl Quantization {
    pub fn from_row(row: u32) -> Option<Self> {
        match row {
            4 => Some(Self::Q4th), 8 => Some(Self::Q8th), 12 => Some(Self::Q12th),
            16 => Some(Self::Q16th), 24 => Some(Self::Q24th), 32 => Some(Self::Q32nd),
            48 => Some(Self::Q48th), 64 => Some(Self::Q64th), 192 => Some(Self::Q192nd),
            _ => None,
        }
    }
}

/// Defines the visual properties of a single sprite from the noteskin texture atlas.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpriteDefinition {
    pub src: [i32; 2],
    pub size: [i32; 2],
    pub rotation_deg: i32,
    pub mirror_h: bool,
    pub mirror_v: bool,
}

/// A simple representation of a game style, needed for parsing.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub num_cols: usize,
    pub num_players: usize,
}

/// Holds all parsed data for a noteskin for a specific style.
#[derive(Debug)]
pub struct Noteskin {
    pub tex_notes_path: String,
    pub tex_receptors_path: String,
    pub tex_glow_path: String,
    pub tex_notes_dims: (u32, u32),
    pub tex_receptors_dims: (u32, u32),
    pub tex_glow_dims: (u32, u32),
    pub notes: Vec<SpriteDefinition>,
    pub mines: Vec<SpriteDefinition>,
    pub hold_bodies: [Vec<SpriteDefinition>; 2],
    pub hold_tails: [Vec<SpriteDefinition>; 2],
    pub receptor_on: Vec<SpriteDefinition>,
    pub receptor_off: Vec<SpriteDefinition>,
    pub receptor_glow: Vec<SpriteDefinition>,
    pub column_xs: Vec<i32>,
    pub hold_y_offsets: [Vec<i32>; 2],
    pub field_left_x: i32,
    pub field_right_x: i32,
}

/// A helper to convert a sprite definition from pixel coordinates to a normalized UV rect.
pub fn get_uv_rect(def: &SpriteDefinition, tex_dims: (u32, u32)) -> [f32; 4] {
    if tex_dims.0 == 0 || tex_dims.1 == 0 { return [0.0, 0.0, 1.0, 1.0]; }
    let u0 = def.src[0] as f32 / tex_dims.0 as f32;
    let v0 = def.src[1] as f32 / tex_dims.1 as f32;
    let u1 = (def.src[0] + def.size[0]) as f32 / tex_dims.0 as f32;
    let v1 = (def.src[1] + def.size[1]) as f32 / tex_dims.1 as f32;
    [u0, v0, u1, v1]
}

/// Main function to load and parse a noteskin file for a given style.
pub fn load(path: &Path, style: &Style) -> Result<Noteskin, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = io::BufReader::new(file);
    let noteskin_dir = path.parent().unwrap_or(Path::new("")).strip_prefix("assets/").unwrap_or(Path::new("")).to_string_lossy();

    let mut defaults = HashMap::<String, SpriteDefinition>::new();
    let mut noteskin = Noteskin {
        tex_notes_path: "".to_string(),
        tex_receptors_path: "".to_string(),
        tex_glow_path: "".to_string(),
        tex_notes_dims: (0, 0),
        tex_receptors_dims: (0, 0),
        tex_glow_dims: (0, 0),
        notes: vec![Default::default(); style.num_players * style.num_cols * NUM_QUANTIZATIONS],
        mines: vec![Default::default(); style.num_players * style.num_cols],
        hold_bodies: [vec![Default::default(); style.num_cols], vec![Default::default(); style.num_cols]],
        hold_tails: [vec![Default::default(); style.num_cols], vec![Default::default(); style.num_cols]],
        receptor_on: vec![Default::default(); style.num_cols],
        receptor_off: vec![Default::default(); style.num_cols],
        receptor_glow: vec![Default::default(); style.num_cols],
        column_xs: (0..style.num_cols).map(|i| (i as i32 * 68) - ((style.num_cols - 1) as i32 * 34)).collect(),
        hold_y_offsets: [vec![0; style.num_cols], vec![0; style.num_cols]],
        field_left_x: 0, field_right_x: 0,
    };

    for line_result in reader.lines() {
        let owned_line = line_result.unwrap_or_default();
        let line = owned_line.trim();
        if line.is_empty() { continue; }

        if let Some((tag, rest)) = line.split_once('{') {
            let tag = tag.trim();
            if let Some(content) = rest.strip_suffix('}') {
                let props: HashMap<&str, &str> = content.split(';')
                    .filter_map(|p| p.trim().split_once('='))
                    .map(|(k, v)| (k.trim(), v.trim()))
                    .collect();

                parse_sprite_rule(tag, &props, style, &mut noteskin, &mut defaults);
            }
        } else if let Some((tag, val)) = line.split_once('=') {
             match tag.trim() {
                "Texture-notes" => noteskin.tex_notes_path = format!("{}/{}", noteskin_dir, val.trim().trim_matches('"')),
                "Texture-receptors" => noteskin.tex_receptors_path = format!("{}/{}", noteskin_dir, val.trim().trim_matches('"')),
                "Texture-glow" => noteskin.tex_glow_path = format!("{}/{}", noteskin_dir, val.trim().trim_matches('"')),
                _ => {}
            }
        }
    }
    
    let first_col_x = noteskin.column_xs.first().cloned().unwrap_or(0);
    let first_receptor_w = noteskin.receptor_off.first().map_or(0, |s| s.size[0]);
    noteskin.field_left_x = first_col_x - first_receptor_w / 2;
    
    let last_col_x = noteskin.column_xs.last().cloned().unwrap_or(0);
    let last_receptor_w = noteskin.receptor_off.last().map_or(0, |s| s.size[0]);
    noteskin.field_right_x = last_col_x + last_receptor_w / 2;

    info!("Loaded noteskin from: {:?}", path);
    Ok(noteskin)
}

fn parse_sprite_rule<'a>(tag: &'a str, props: &HashMap<&str, &str>, style: &Style, ns: &mut Noteskin, defaults: &mut HashMap<String, SpriteDefinition>) {
    let mut def = defaults.get(tag).cloned().unwrap_or_default();
    
    if let Some(src_str) = props.get("src") {
        if let Some((x_str, y_str)) = src_str.split_once(',') {
            def.src = [x_str.parse().unwrap_or(0), y_str.parse().unwrap_or(0)];
        }
    }
    if let Some(size_str) = props.get("size") {
        if let Some((w_str, h_str)) = size_str.split_once(',') {
            def.size = [w_str.parse().unwrap_or(0), h_str.parse().unwrap_or(0)];
        }
    }
    if let Some(rot_str) = props.get("rot") { def.rotation_deg = rot_str.parse().unwrap_or(0); }
    if let Some(mirror_str) = props.get("mirror") {
        def.mirror_h = mirror_str.contains('h');
        def.mirror_v = mirror_str.contains('v');
    }

    let has_range_spec = props.contains_key("row") || props.contains_key("col") || props.contains_key("player");

    if !has_range_spec {
        defaults.insert(tag.to_string(), def);
    }

    let apply_to = |target_vec: &mut Vec<SpriteDefinition>, index: usize| {
        if index < target_vec.len() {
            if !has_range_spec {
                for item in target_vec.iter_mut() { *item = def; }
            } else {
                target_vec[index] = def;
            }
        }
    };
    
    let rows = props.get("row").and_then(|s| s.parse().ok()).map(|r| vec![r]).unwrap_or_else(|| (0..=192).collect());
    let cols = props.get("col").and_then(|s| s.parse().ok()).map(|c| vec![c]).unwrap_or_else(|| (0..style.num_cols as u32).collect());
    let players = props.get("player").and_then(|s| s.parse().ok()).map(|p| vec![p]).unwrap_or_else(|| (0..style.num_players as u32).collect());

    for p in &players {
        for c in &cols {
            if *p >= style.num_players as u32 || *c >= style.num_cols as u32 { continue; }

            match tag {
                "Note" => {
                    for r in &rows {
                        if let Some(q) = Quantization::from_row(*r) {
                            let idx = (*p as usize * style.num_cols + *c as usize) * NUM_QUANTIZATIONS + q as usize;
                            apply_to(&mut ns.notes, idx);
                        }
                    }
                }
                "Mine" => apply_to(&mut ns.mines, *p as usize * style.num_cols + *c as usize),
                "Receptor-on" => apply_to(&mut ns.receptor_on, *c as usize),
                "Receptor-off" => apply_to(&mut ns.receptor_off, *c as usize),
                "Receptor-glow" => apply_to(&mut ns.receptor_glow, *c as usize),
                "Hold-body" => apply_to(&mut ns.hold_bodies[0], *c as usize),
                "Hold-tail" => apply_to(&mut ns.hold_tails[0], *c as usize),
                "Roll-body" => apply_to(&mut ns.hold_bodies[1], *c as usize),
                "Roll-tail" => apply_to(&mut ns.hold_tails[1], *c as usize),
                "Receptor" => if let Some(x_str) = props.get("x") { ns.column_xs[*c as usize] = x_str.parse().unwrap_or(0); },
                _ => {},
            }
        }
    }
}

use crate::assets;
use image::image_dimensions;
use log::{info, warn};
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const NUM_QUANTIZATIONS: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Quantization {
    Q4th = 0,
    Q8th,
    Q12th,
    Q16th,
    Q24th,
    Q32nd,
    Q48th,
    Q64th,
    Q192nd,
}

impl Quantization {
    pub fn from_row(row: u32) -> Option<Self> {
        match row {
            4 => Some(Self::Q4th),
            8 => Some(Self::Q8th),
            12 => Some(Self::Q12th),
            16 => Some(Self::Q16th),
            24 => Some(Self::Q24th),
            32 => Some(Self::Q32nd),
            48 => Some(Self::Q48th),
            64 => Some(Self::Q64th),
            192 => Some(Self::Q192nd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpriteDefinition {
    pub src: [i32; 2],
    pub size: [i32; 2],
    pub rotation_deg: i32,
    pub mirror_h: bool,
    pub mirror_v: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum AnimationRate {
    FramesPerSecond(f32),
    FramesPerBeat(f32),
}

#[derive(Debug)]
pub enum SpriteSource {
    Atlas {
        texture_key: String,
        tex_dims: (u32, u32),
    },
    Animated {
        texture_key: String,
        tex_dims: (u32, u32),
        frame_size: [i32; 2],
        grid: (usize, usize),
        frame_count: usize,
        rate: AnimationRate,
    },
}

impl SpriteSource {
    pub fn texture_key(&self) -> &str {
        match self {
            SpriteSource::Atlas { texture_key, .. } => texture_key,
            SpriteSource::Animated { texture_key, .. } => texture_key,
        }
    }

    pub fn tex_dims(&self) -> (u32, u32) {
        match self {
            SpriteSource::Atlas { tex_dims, .. } => *tex_dims,
            SpriteSource::Animated { tex_dims, .. } => *tex_dims,
        }
    }

    pub fn frame_count(&self) -> usize {
        match self {
            SpriteSource::Atlas { .. } => 1,
            SpriteSource::Animated { frame_count, .. } => (*frame_count).max(1),
        }
    }

    fn frame_size(&self) -> Option<[i32; 2]> {
        match self {
            SpriteSource::Atlas { .. } => None,
            SpriteSource::Animated { frame_size, .. } => Some(*frame_size),
        }
    }

    pub fn is_beat_based(&self) -> bool {
        matches!(
            self,
            SpriteSource::Animated {
                rate: AnimationRate::FramesPerBeat(_),
                ..
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct SpriteSlot {
    pub def: SpriteDefinition,
    pub source: Arc<SpriteSource>,
}

impl SpriteSlot {
    pub fn texture_key(&self) -> &str {
        self.source.texture_key()
    }

    pub fn size(&self) -> [i32; 2] {
        self.def.size
    }

    pub fn frame_index(&self, time: f32, beat: f32) -> usize {
        let frames = self.source.frame_count();
        if frames <= 1 {
            return 0;
        }
        match self.source.as_ref() {
            SpriteSource::Atlas { .. } => 0,
            SpriteSource::Animated { rate, .. } => {
                let frame = match rate {
                    AnimationRate::FramesPerSecond(fps) if *fps > 0.0 => {
                        (time * fps).floor() as isize
                    }
                    AnimationRate::FramesPerBeat(frames_per_beat) if *frames_per_beat > 0.0 => {
                        (beat * frames_per_beat).floor() as isize
                    }
                    _ => return 0,
                };
                ((frame % frames as isize) + frames as isize) as usize % frames
            }
        }
    }

    pub fn uv_for_frame(&self, frame_index: usize) -> [f32; 4] {
        match self.source.as_ref() {
            SpriteSource::Atlas { tex_dims, .. } => {
                let tw = tex_dims.0.max(1);
                let th = tex_dims.1.max(1);
                let src = self.def.src;
                let size = self.def.size;
                let u0 = src[0] as f32 / tw as f32;
                let v0 = src[1] as f32 / th as f32;
                let u1 = (src[0] + size[0]) as f32 / tw as f32;
                let v1 = (src[1] + size[1]) as f32 / th as f32;
                [u0, v0, u1, v1]
            }
            SpriteSource::Animated {
                tex_dims,
                frame_size,
                grid,
                frame_count,
                ..
            } => {
                let frames = (*frame_count).max(1);
                let idx = if frames > 0 { frame_index % frames } else { 0 };
                let cols = grid.0.max(1);
                let row = idx / cols;
                let col = idx % cols;
                let src_x = self.def.src[0] + (col as i32 * frame_size[0]);
                let src_y = self.def.src[1] + (row as i32 * frame_size[1]);
                let tw = tex_dims.0.max(1);
                let th = tex_dims.1.max(1);
                let u0 = src_x as f32 / tw as f32;
                let v0 = src_y as f32 / th as f32;
                let u1 = (src_x + frame_size[0]) as f32 / tw as f32;
                let v1 = (src_y + frame_size[1]) as f32 / th as f32;
                [u0, v0, u1, v1]
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub num_cols: usize,
    pub num_players: usize,
}

#[derive(Debug)]
pub struct Noteskin {
    pub notes: Vec<SpriteSlot>,
    pub receptor_off: Vec<SpriteSlot>,
    pub receptor_glow: Vec<Option<SpriteSlot>>,
    pub column_xs: Vec<i32>,
    pub field_left_x: i32,
    pub field_right_x: i32,
    pub tap_explosions: HashMap<String, SpriteSlot>,
    pub receptor_pulse: ReceptorPulse,
}

#[derive(Debug, Clone, Copy)]
pub struct ReceptorPulse {
    pub base_color: [f32; 4],
    pub beat_color: [f32; 4],
    pub hold_beats: f32,
    pub fade_beats: f32,
}

impl ReceptorPulse {
    pub fn color_for_beat(&self, beat: f32) -> [f32; 4] {
        let hold = self.hold_beats.max(0.0);
        let fade = self.fade_beats.max(0.0);

        let phase = beat.rem_euclid(1.0);
        if phase < f32::EPSILON {
            return self.beat_color;
        }
        if phase < hold {
            return self.beat_color;
        }
        if fade > 0.0 && phase < hold + fade {
            let t = ((phase - hold) / fade).clamp(0.0, 1.0);
            let mut color = [0.0; 4];
            for i in 0..4 {
                color[i] = self.beat_color[i] + (self.base_color[i] - self.beat_color[i]) * t;
            }
            return color;
        }

        self.base_color
    }
}

impl Default for ReceptorPulse {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            beat_color: [1.0, 1.0, 1.0, 1.0],
            hold_beats: 0.0,
            fade_beats: 0.0,
        }
    }
}

#[derive(Clone, Default)]
struct SlotBuilder {
    def: SpriteDefinition,
    source: Option<Arc<SpriteSource>>,
}

impl SlotBuilder {
    fn set_source(&mut self, source: Arc<SpriteSource>) {
        self.source = Some(source);
    }
}

struct NoteskinBuilder {
    notes: Vec<SlotBuilder>,
    receptor_off: Vec<SlotBuilder>,
    receptor_glow: Vec<SlotBuilder>,
    column_xs: Vec<i32>,
    defaults: HashMap<String, SpriteDefinition>,
    default_sources: HashMap<String, Arc<SpriteSource>>,
    tap_explosions: HashMap<String, SlotBuilder>,
    receptor_pulse: ReceptorPulse,
}

impl NoteskinBuilder {
    fn new(style: &Style) -> Self {
        let note_slots = style.num_players * style.num_cols * NUM_QUANTIZATIONS;
        Self {
            notes: vec![SlotBuilder::default(); note_slots],
            receptor_off: vec![SlotBuilder::default(); style.num_cols],
            receptor_glow: vec![SlotBuilder::default(); style.num_cols],
            column_xs: (0..style.num_cols)
                .map(|i| (i as i32 * 68) - ((style.num_cols - 1) as i32 * 34))
                .collect(),
            defaults: HashMap::new(),
            default_sources: HashMap::new(),
            tap_explosions: HashMap::new(),
            receptor_pulse: ReceptorPulse::default(),
        }
    }

    fn finalize(self) -> Result<Noteskin, String> {
        fn finalize_slots(
            slots: Vec<SlotBuilder>,
            default_source: Option<&Arc<SpriteSource>>,
            tag: &str,
        ) -> Result<Vec<SpriteSlot>, String> {
            let mut result = Vec::with_capacity(slots.len());
            for slot in slots {
                let source = match slot.source {
                    Some(src) => src,
                    None => match default_source {
                        Some(src) => src.clone(),
                        None => {
                            return Err(format!(
                                "Noteskin missing texture assignment for category '{}'.",
                                tag
                            ))
                        }
                    },
                };
                result.push(SpriteSlot {
                    def: slot.def,
                    source,
                });
            }
            Ok(result)
        }

        fn finalize_optional_slots(
            slots: Vec<SlotBuilder>,
            default_source: Option<&Arc<SpriteSource>>,
        ) -> Vec<Option<SpriteSlot>> {
            slots
                .into_iter()
                .map(|slot| {
                    slot.source
                        .or_else(|| default_source.cloned())
                        .map(|source| SpriteSlot {
                            def: slot.def,
                            source,
                        })
                })
                .collect()
        }

        let notes = finalize_slots(self.notes, self.default_sources.get("Note"), "Note")?;
        let receptor_off = finalize_slots(
            self.receptor_off,
            self.default_sources.get("Receptor-off"),
            "Receptor-off",
        )?;
        let receptor_glow = finalize_optional_slots(
            self.receptor_glow,
            self.default_sources.get("Receptor-glow"),
        );

        let tap_explosions = self
            .tap_explosions
            .into_iter()
            .filter_map(|(window, slot)| {
                slot.source.map(|source| {
                    (
                        window,
                        SpriteSlot {
                            def: slot.def,
                            source,
                        },
                    )
                })
            })
            .collect();

        let column_xs = self.column_xs;
        let field_left_x = column_xs.first().cloned().unwrap_or(0)
            - receptor_off
                .first()
                .map(|slot| slot.def.size[0] / 2)
                .unwrap_or(0);
        let field_right_x = column_xs.last().cloned().unwrap_or(0)
            + receptor_off
                .last()
                .map(|slot| slot.def.size[0] / 2)
                .unwrap_or(0);

        Ok(Noteskin {
            notes,
            receptor_off,
            receptor_glow,
            column_xs,
            field_left_x,
            field_right_x,
            tap_explosions,
            receptor_pulse: self.receptor_pulse,
        })
    }
}

pub fn load(path: &Path, style: &Style) -> Result<Noteskin, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = io::BufReader::new(file);
    let noteskin_dir = path
        .parent()
        .unwrap_or(Path::new(""))
        .strip_prefix("assets/")
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .to_string();

    let mut builder = NoteskinBuilder::new(style);

    for line_result in reader.lines() {
        let owned_line = line_result.unwrap_or_default();
        let line = owned_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((tag, rest)) = line.split_once('{') {
            if let Some(content) = rest.strip_suffix('}') {
                let tag = tag.trim();
                let props: HashMap<&str, &str> = content
                    .split(';')
                    .filter_map(|p| p.trim().split_once('='))
                    .map(|(k, v)| (k.trim(), v.trim()))
                    .collect();

                match tag {
                    "NoteSheet" => parse_note_sheet(&noteskin_dir, &mut builder, style, &props),
                    "ReceptorSheet" => {
                        parse_receptor_sheet(&noteskin_dir, &mut builder, style, &props)
                    }
                    "GlowSheet" => parse_glow_sheet(&noteskin_dir, &mut builder, style, &props),
                    "ExplosionSheet" => parse_explosion_sheet(&noteskin_dir, &mut builder, &props),
                    "ReceptorPulse" => parse_receptor_pulse(&mut builder, &props),
                    _ => parse_sprite_rule(tag, &props, style, &mut builder),
                }
            }
        } else if let Some((tag, val)) = line.split_once('=') {
            let value = val.trim().trim_matches('"');
            match tag.trim() {
                "Texture-notes" => {
                    if let Some(src) = build_atlas_source(&noteskin_dir, value) {
                        builder.default_sources.insert("Note".to_string(), src);
                    }
                }
                "Texture-receptors" => {
                    if let Some(src) = build_atlas_source(&noteskin_dir, value) {
                        builder
                            .default_sources
                            .insert("Receptor-off".to_string(), src.clone());
                        builder
                            .default_sources
                            .insert("Receptor-on".to_string(), src);
                    }
                }
                "Texture-glow" => {
                    if let Some(src) = build_atlas_source(&noteskin_dir, value) {
                        builder
                            .default_sources
                            .insert("Receptor-glow".to_string(), src);
                    }
                }
                _ => {}
            }
        }
    }

    let noteskin = builder.finalize()?;
    info!("Loaded noteskin from: {:?}", path);
    Ok(noteskin)
}

fn parse_note_sheet(
    noteskin_dir: &str,
    builder: &mut NoteskinBuilder,
    style: &Style,
    props: &HashMap<&str, &str>,
) {
    let Some(texture) = props.get("texture").map(|s| s.trim().trim_matches('"')) else {
        warn!("NoteSheet missing texture attribute");
        return;
    };

    let source = match build_sheet_source(noteskin_dir, texture, props, 30.0) {
        Some(src) => src,
        None => return,
    };

    let quants = parse_quant_list(props).into_iter().collect::<Vec<_>>();
    if quants.is_empty() {
        warn!("NoteSheet declared without quantization list");
        return;
    }

    let players = parse_index(props.get("player"), style.num_players as u32);
    let cols = parse_index(props.get("col"), style.num_cols as u32);

    let frame_size = source.frame_size().unwrap_or_else(|| {
        builder
            .defaults
            .get("Note")
            .map(|d| d.size)
            .unwrap_or([0, 0])
    });

    for p in &players {
        if (*p as usize) >= style.num_players {
            continue;
        }
        for c in &cols {
            if (*c as usize) >= style.num_cols {
                continue;
            }
            for q in &quants {
                let idx = ((*p as usize * style.num_cols) + *c as usize) * NUM_QUANTIZATIONS
                    + *q as usize;
                if let Some(slot) = builder.notes.get_mut(idx) {
                    slot.def.size = frame_size;
                    slot.def.src = parse_src_offset(props).unwrap_or([0, 0]);
                    slot.set_source(source.clone());
                }
            }
        }
    }
}

fn parse_receptor_sheet(
    noteskin_dir: &str,
    builder: &mut NoteskinBuilder,
    style: &Style,
    props: &HashMap<&str, &str>,
) {
    let Some(texture) = props.get("texture").map(|s| s.trim().trim_matches('"')) else {
        warn!("ReceptorSheet missing texture attribute");
        return;
    };

    let default_state = props
        .get("state")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "off".to_string());
    let source = match build_sheet_source(noteskin_dir, texture, props, 20.0) {
        Some(src) => src,
        None => return,
    };
    let frame_size = source.frame_size().unwrap_or_else(|| {
        builder
            .defaults
            .get("Receptor-off")
            .map(|d| d.size)
            .unwrap_or([0, 0])
    });

    let cols = parse_index(props.get("col"), style.num_cols as u32);
    let target = match default_state.as_str() {
        "off" => Some(&mut builder.receptor_off),
        "glow" => Some(&mut builder.receptor_glow),
        _ => {
            warn!("ReceptorSheet state '{}' is not supported", default_state);
            None
        }
    };

    if let Some(slots) = target {
        for c in cols {
            if (c as usize) >= slots.len() {
                continue;
            }
            if let Some(slot) = slots.get_mut(c as usize) {
                slot.def.size = frame_size;
                slot.def.src = parse_src_offset(props).unwrap_or([0, 0]);
                slot.set_source(source.clone());
            }
        }
    }
}

fn parse_glow_sheet(
    noteskin_dir: &str,
    builder: &mut NoteskinBuilder,
    style: &Style,
    props: &HashMap<&str, &str>,
) {
    let Some(texture) = props.get("texture").map(|s| s.trim().trim_matches('"')) else {
        warn!("GlowSheet missing texture attribute");
        return;
    };

    let source = match build_sheet_source(noteskin_dir, texture, props, 30.0) {
        Some(src) => src,
        None => return,
    };
    let frame_size = source.frame_size().unwrap_or_else(|| {
        builder
            .defaults
            .get("Receptor-glow")
            .map(|d| d.size)
            .unwrap_or([0, 0])
    });

    let cols = parse_index(props.get("col"), style.num_cols as u32);
    for c in cols {
        if (c as usize) >= builder.receptor_glow.len() {
            continue;
        }
        if let Some(slot) = builder.receptor_glow.get_mut(c as usize) {
            slot.def.size = frame_size;
            slot.def.src = parse_src_offset(props).unwrap_or([0, 0]);
            slot.set_source(source.clone());
        }
    }
}

fn parse_explosion_sheet(
    noteskin_dir: &str,
    builder: &mut NoteskinBuilder,
    props: &HashMap<&str, &str>,
) {
    let Some(texture) = props.get("texture").map(|s| s.trim().trim_matches('"')) else {
        warn!("ExplosionSheet missing texture attribute");
        return;
    };
    let Some(window) = props.get("window").map(|s| s.trim().to_ascii_uppercase()) else {
        warn!("ExplosionSheet missing window attribute");
        return;
    };

    let source = match build_sheet_source(noteskin_dir, texture, props, 30.0) {
        Some(src) => src,
        None => return,
    };

    let mut slot = SlotBuilder::default();
    slot.def.size = source.frame_size().unwrap_or([0, 0]);
    slot.def.src = parse_src_offset(props).unwrap_or([0, 0]);
    slot.set_source(source);
    builder.tap_explosions.insert(window, slot);
}

fn parse_sprite_rule(
    tag: &str,
    props: &HashMap<&str, &str>,
    style: &Style,
    builder: &mut NoteskinBuilder,
) {
    let apply_properties = |slot: &mut SlotBuilder| {
        if let Some(src_str) = props.get("src") {
            if let Some((x_str, y_str)) = src_str.split_once(',') {
                slot.def.src = [x_str.parse().unwrap_or(0), y_str.parse().unwrap_or(0)];
            }
        }
        if let Some(size_str) = props.get("size") {
            if let Some((w_str, h_str)) = size_str.split_once(',') {
                slot.def.size = [w_str.parse().unwrap_or(0), h_str.parse().unwrap_or(0)];
            }
        }
        if let Some(rot_str) = props.get("rot") {
            slot.def.rotation_deg = rot_str.parse().unwrap_or(0);
        }
        if let Some(mirror_str) = props.get("mirror") {
            slot.def.mirror_h = mirror_str.contains('h');
            slot.def.mirror_v = mirror_str.contains('v');
        }
    };

    let has_range_spec =
        props.contains_key("row") || props.contains_key("col") || props.contains_key("player");
    if !has_range_spec {
        let mut def = builder.defaults.get(tag).cloned().unwrap_or_default();
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
        if let Some(rot_str) = props.get("rot") {
            def.rotation_deg = rot_str.parse().unwrap_or(0);
        }
        if let Some(mirror_str) = props.get("mirror") {
            def.mirror_h = mirror_str.contains('h');
            def.mirror_v = mirror_str.contains('v');
        }
        builder.defaults.insert(tag.to_string(), def);
    }

    let rows = props
        .get("row")
        .and_then(|s| s.parse().ok())
        .map(|r| vec![r])
        .unwrap_or_else(|| (0..=192).collect());
    let cols = parse_index(props.get("col"), style.num_cols as u32);
    let players = parse_index(props.get("player"), style.num_players as u32);

    for p in &players {
        for c in &cols {
            if (*p as usize) >= style.num_players || (*c as usize) >= style.num_cols {
                continue;
            }
            match tag {
                "Note" => {
                    for r in &rows {
                        if let Some(q) = Quantization::from_row(*r) {
                            let idx = ((*p as usize * style.num_cols) + *c as usize)
                                * NUM_QUANTIZATIONS
                                + q as usize;
                            if let Some(slot) = builder.notes.get_mut(idx) {
                                apply_properties(slot);
                            }
                        }
                    }
                }
                "Receptor-off" => {
                    if let Some(slot) = builder.receptor_off.get_mut(*c as usize) {
                        apply_properties(slot);
                    }
                }
                "Receptor-glow" => {
                    if let Some(slot) = builder.receptor_glow.get_mut(*c as usize) {
                        apply_properties(slot);
                    }
                }
                "Receptor" => {
                    if let Some(x_str) = props.get("x") {
                        builder.column_xs[*c as usize] = x_str.parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }
    }
}

fn parse_quant_list(props: &HashMap<&str, &str>) -> Vec<Quantization> {
    if let Some(list) = props.get("quants").or_else(|| props.get("quant")) {
        list.split(|c| c == ',' || c == ' ' || c == ';')
            .filter_map(|part| part.trim().parse::<u32>().ok())
            .filter_map(Quantization::from_row)
            .collect()
    } else if let Some(row) = props.get("row").and_then(|s| s.parse::<u32>().ok()) {
        Quantization::from_row(row).into_iter().collect()
    } else {
        Vec::new()
    }
}

fn parse_index(spec: Option<&&str>, max: u32) -> Vec<u32> {
    match spec {
        Some(value) => value
            .split(',')
            .filter_map(|v| v.trim().parse::<u32>().ok())
            .collect::<Vec<_>>(),
        None => (0..max).collect(),
    }
}

fn parse_src_offset(props: &HashMap<&str, &str>) -> Option<[i32; 2]> {
    props.get("offset").and_then(|s| {
        s.split_once(',')
            .map(|(x, y)| [x.trim().parse().unwrap_or(0), y.trim().parse().unwrap_or(0)])
    })
}

fn build_atlas_source(noteskin_dir: &str, texture: &str) -> Option<Arc<SpriteSource>> {
    let key = resolve_texture_key(noteskin_dir, texture);
    let dims = texture_dimensions(&key)?;
    Some(Arc::new(SpriteSource::Atlas {
        texture_key: key,
        tex_dims: dims,
    }))
}

fn build_sheet_source(
    noteskin_dir: &str,
    texture: &str,
    props: &HashMap<&str, &str>,
    default_fps: f32,
) -> Option<Arc<SpriteSource>> {
    let key = resolve_texture_key(noteskin_dir, texture);
    let dims = texture_dimensions(&key)?;
    let fps = props
        .get("fps")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(default_fps);
    let grid = props
        .get("grid")
        .and_then(|g| parse_pair_usize(g))
        .unwrap_or_else(|| infer_grid(&key));
    let frame_size = props
        .get("frame_size")
        .and_then(|s| parse_pair_i32(s))
        .unwrap_or_else(|| infer_frame_size(dims, grid));
    let frames = props
        .get("frames")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| grid.0 * grid.1);
    let beats_per_loop = props
        .get("beats_per_loop")
        .or_else(|| props.get("loop_beats"))
        .or_else(|| props.get("beat_length"))
        .or_else(|| props.get("animation_length"))
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0);
    let beat_based_flag = props
        .get("AnimationIsBeatBased")
        .or_else(|| props.get("animationisbeatbased"))
        .or_else(|| props.get("animation_is_beat_based"))
        .map(|v| matches!(v.trim(), "1" | "true" | "True" | "TRUE" | "yes" | "Yes"))
        .unwrap_or(false);
    let beat_based = beat_based_flag || beats_per_loop.is_some();
    let frame_count = frames.max(1);
    let rate = if beat_based {
        let frames_per_beat = beats_per_loop
            .map(|beats| {
                if beats != 0.0 {
                    frame_count as f32 / beats
                } else {
                    0.0
                }
            })
            .unwrap_or(fps)
            .max(0.0);
        AnimationRate::FramesPerBeat(frames_per_beat)
    } else {
        AnimationRate::FramesPerSecond(fps.max(0.0))
    };

    Some(Arc::new(SpriteSource::Animated {
        texture_key: key,
        tex_dims: dims,
        frame_size,
        grid,
        frame_count,
        rate,
    }))
}

fn resolve_texture_key(base: &str, texture: &str) -> String {
    let mut key = if texture.contains('/') {
        texture.to_string()
    } else if base.is_empty() {
        texture.to_string()
    } else {
        format!("{}/{}", base, texture)
    };
    key = key.replace('\\', "/");
    key
}

fn texture_dimensions(key: &str) -> Option<(u32, u32)> {
    if let Some(meta) = assets::texture_dims(key) {
        return Some((meta.w, meta.h));
    }
    let path = PathBuf::from("assets").join(key);
    image_dimensions(&path).ok()
}

fn parse_pair_usize(input: &str) -> Option<(usize, usize)> {
    input
        .split_once(',')
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
}

fn parse_pair_i32(input: &str) -> Option<[i32; 2]> {
    input
        .split_once(',')
        .and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?]))
}

fn infer_grid(texture_key: &str) -> (usize, usize) {
    let (w, h) = assets::parse_sprite_sheet_dims(texture_key);
    (w.max(1) as usize, h.max(1) as usize)
}

fn infer_frame_size(dims: (u32, u32), grid: (usize, usize)) -> [i32; 2] {
    let width = if grid.0 > 0 {
        (dims.0 / grid.0 as u32) as i32
    } else {
        dims.0 as i32
    };
    let height = if grid.1 > 0 {
        (dims.1 / grid.1 as u32) as i32
    } else {
        dims.1 as i32
    };
    [width.max(1), height.max(1)]
}

fn parse_color_rgba(input: &str) -> Option<[f32; 4]> {
    let mut components = input
        .split(',')
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>();

    if components.is_empty() {
        return None;
    }

    if components.len() == 3 {
        components.push("1.0");
    }

    if components.len() != 4 {
        return None;
    }

    let mut color = [0.0; 4];
    for (i, component) in components.iter().enumerate().take(4) {
        color[i] = component.parse::<f32>().ok()?;
    }
    Some(color)
}

fn parse_receptor_pulse(builder: &mut NoteskinBuilder, props: &HashMap<&str, &str>) {
    let mut pulse = builder.receptor_pulse;

    if let Some(base) = props
        .get("base_color")
        .or_else(|| props.get("base"))
        .and_then(|v| parse_color_rgba(v))
    {
        pulse.base_color = base;
    }

    if let Some(beat) = props
        .get("beat_color")
        .or_else(|| props.get("bright_color"))
        .and_then(|v| parse_color_rgba(v))
    {
        pulse.beat_color = beat;
    }

    if let Some(hold) = props
        .get("hold_beats")
        .or_else(|| props.get("hold"))
        .or_else(|| props.get("duration"))
        .and_then(|v| v.parse::<f32>().ok())
    {
        pulse.hold_beats = hold.max(0.0);
    }

    if let Some(fade) = props
        .get("fade_beats")
        .or_else(|| props.get("fade"))
        .or_else(|| props.get("fade_duration"))
        .and_then(|v| v.parse::<f32>().ok())
    {
        pulse.fade_beats = fade.max(0.0);
    }

    builder.receptor_pulse = pulse;
}

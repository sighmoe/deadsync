use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use std::sync::{Arc, LazyLock};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// --- Optional palette accessors (hex → rgba) if/when you need them later ---
#[allow(dead_code)] fn col_music_wheel_box() -> [f32; 4] { color::rgba_hex("#0a141b") } // #0a141b
#[allow(dead_code)] fn col_pack_header_box() -> [f32; 4] { color::rgba_hex("#4c565d") } // #4c565d
#[allow(dead_code)] fn col_selected_song_box() -> [f32; 4] { color::rgba_hex("#272f35") } // #272f35
#[allow(dead_code)] fn col_selected_pack_header_box() -> [f32; 4] { color::rgba_hex("#5f686e") } // #5f686e
#[allow(dead_code)] fn col_pink_box() -> [f32; 4] { color::rgba_hex("#ff47b3") } // #ff47b3

// --- New UI Box Color ---
static UI_BOX_BG_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#1E282F"));

// --- Typography (targets in SM TL units) ---
const MUSIC_WHEEL_TEXT_TARGET_PX: f32 = 15.0;
const DETAIL_HEADER_TEXT_TARGET_PX: f32 = 22.0;
const DETAIL_VALUE_TEXT_TARGET_PX: f32 = 15.0;

// --- Music Wheel Layout ---
const NUM_WHEEL_ITEMS: usize = 13;
const CENTER_WHEEL_SLOT_INDEX: usize = NUM_WHEEL_ITEMS / 2;
const SELECTION_ANIMATION_CYCLE_DURATION: f32 = 1.0;

const SONG_TEXT_LEFT_PADDING: f32 = 66.0;
const PACK_COUNT_RIGHT_PADDING: f32 = 11.0;
const PACK_COUNT_TEXT_TARGET_PX: f32 = 14.0;

// --- Difficulty Meter ---
const DIFFICULTY_DISPLAY_INNER_BOX_COLOR: [f32; 4] = [0.059, 0.059, 0.059, 1.0]; // #0f0f0f
const DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
// Colors are now sourced from the theme palette
static DIFFICULTY_COLORS: LazyLock<[[f32; 4]; 5]> = LazyLock::new(|| [
    color::simply_love_rgba(10), // Beginner ~ #FFBE00
    color::simply_love_rgba(11), // Easy ~ #FF7D00
    color::simply_love_rgba(0),  // Medium ~ #FF5D47
    color::simply_love_rgba(1),  // Hard ~ #FF577E
    color::simply_love_rgba(2),  // Challenge ~ #FF47B3
]);

/* ==================================================================
 *                            STATE & DATA
 * ================================================================== */
#[derive(Clone, Debug)]
pub struct SongData {
    pub title: String,
    pub artist: String,
    pub banner_path: Option<&'static str>,
    pub charts: Vec<ChartData>,
}

#[derive(Clone, Debug)]
pub struct ChartData {
    pub difficulty: String,
    pub meter: u32,
    pub step_artist: String,
}

#[derive(Clone, Debug)]
pub enum MusicWheelEntry {
    PackHeader { name: String, color: [f32; 4] },
    Song(Arc<SongData>),
}

pub struct State {
    pub all_entries: Vec<MusicWheelEntry>, // The master list of all songs and packs
    pub entries: Vec<MusicWheelEntry>,     // The currently visible entries in the wheel
    pub selected_index: usize,
    pub selected_difficulty_index: usize,
    pub active_color_index: i32,
    pub selection_animation_timer: f32,
    pub expanded_pack_name: Option<String>, // Tracks which pack is open

    bg: heart_bg::State,
}

/// Rebuilds the visible `entries` list based on which pack is expanded.
fn rebuild_displayed_entries(state: &mut State) {
    let mut new_entries = Vec::new();
    let mut current_pack_name_for_filtering: Option<String> = None;

    for entry in &state.all_entries {
        match entry {
            MusicWheelEntry::PackHeader { name, .. } => {
                current_pack_name_for_filtering = Some(name.clone());
                new_entries.push(entry.clone()); // Always show pack headers
            }
            MusicWheelEntry::Song(_) => {
                // Only show songs if their pack is the currently expanded one
                if state.expanded_pack_name.is_some() && current_pack_name_for_filtering == state.expanded_pack_name {
                    new_entries.push(entry.clone());
                }
            }
        }
    }
    state.entries = new_entries;
}


pub fn init() -> State {
    let mut all_entries = vec![];

    // --- Pack 1 ---
    all_entries.push(MusicWheelEntry::PackHeader {
        name: "Community Pack".to_string(),
        color: color::simply_love_rgba(2),
    });
    for i in 1..=5 {
        all_entries.push(MusicWheelEntry::Song(Arc::new(SongData {
            title: format!("Community Song {}", i),
            artist: "Various Artists".to_string(),
            banner_path: Some("fallback_banner.png"),
            charts: vec![
                ChartData { difficulty: "Easy".to_string(), meter: 3 + i as u32, step_artist: "Community".to_string() },
                ChartData { difficulty: "Hard".to_string(), meter: 7 + i as u32, step_artist: "Community".to_string() },
            ],
        })));
    }

    // --- Pack 2 ---
    all_entries.push(MusicWheelEntry::PackHeader {
        name: "Extra Pack".to_string(),
        color: color::simply_love_rgba(5),
    });
    for i in 1..=8 {
        all_entries.push(MusicWheelEntry::Song(Arc::new(SongData {
            title: format!("Extra Song {}", i),
            artist: "An Artist".to_string(),
            banner_path: Some("fallback_banner.png"),
            charts: vec![
                ChartData { difficulty: "Medium".to_string(), meter: 6, step_artist: "Val".to_string() },
                ChartData { difficulty: "Challenge".to_string(), meter: 11, step_artist: "Val".to_string() },
            ],
        })));
    }

    // --- Pack 3 ---
    all_entries.push(MusicWheelEntry::PackHeader {
        name: "Solo Pack".to_string(),
        color: color::simply_love_rgba(8),
    });
    all_entries.push(MusicWheelEntry::Song(Arc::new(SongData {
        title: "The Only Song".to_string(),
        artist: "Solo Artist".to_string(),
        banner_path: Some("fallback_banner.png"),
        charts: vec![
            ChartData { difficulty: "Hard".to_string(), meter: 9, step_artist: "Solo".to_string() },
        ],
    })));

    // --- Pack 4 ---
    all_entries.push(MusicWheelEntry::PackHeader {
        name: "Final Pack".to_string(),
        color: color::simply_love_rgba(0),
    });
    for i in 1..=3 {
        all_entries.push(MusicWheelEntry::Song(Arc::new(SongData {
            title: format!("Final Song {}", i),
            artist: "The Finisher".to_string(),
            banner_path: Some("fallback_banner.png"),
            charts: vec![
                ChartData { difficulty: "Challenge".to_string(), meter: 10 + i as u32, step_artist: "Final".to_string() },
            ],
        })));
    }

    let mut state = State {
        all_entries,
        entries: Vec::new(), // Will be populated by rebuild
        selected_index: 0,
        selected_difficulty_index: 2,
        active_color_index: color::DEFAULT_COLOR_INDEX,
        selection_animation_timer: 0.0,
        expanded_pack_name: None, // All packs start closed
        bg: heart_bg::State::new(),
    };

    rebuild_displayed_entries(&mut state); // Populate the initial visible list

    state
}

/* ==================================================================
 *                         INPUT & UPDATE
 * ================================================================== */

/// Simple color interpolation.
fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed { return ScreenAction::None; }
    let num_entries = state.entries.len();
    if num_entries == 0 { return ScreenAction::None; }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
            state.selected_index = (state.selected_index + 1) % num_entries;
            state.selection_animation_timer = 0.0;
        }
        PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
            state.selected_index = (state.selected_index + num_entries - 1) % num_entries;
            state.selection_animation_timer = 0.0;
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected_difficulty_index = (state.selected_difficulty_index + 1).min(DIFFICULTY_NAMES.len() - 1);
        }
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            state.selected_difficulty_index = state.selected_difficulty_index.saturating_sub(1);
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            if let Some(entry) = state.entries.get(state.selected_index).cloned() {
                match entry {
                    MusicWheelEntry::Song(_) => {
                        return ScreenAction::Navigate(Screen::Gameplay);
                    }
                    MusicWheelEntry::PackHeader { name, .. } => {
                        let pack_name_to_focus = name.clone();

                        if state.expanded_pack_name.as_ref() == Some(&pack_name_to_focus) {
                            state.expanded_pack_name = None; // Collapse current pack
                        } else {
                            state.expanded_pack_name = Some(pack_name_to_focus.clone()); // Expand or switch
                        }

                        rebuild_displayed_entries(state);

                        // Find the pack we just interacted with in the new list to keep it selected
                        let new_selection = state.entries.iter().position(|e| {
                            if let MusicWheelEntry::PackHeader{ name: n, .. } = e {
                                n == &pack_name_to_focus
                            } else {
                                false
                            }
                        }).unwrap_or(0);

                        state.selected_index = new_selection;
                    }
                }
            }
        }
        PhysicalKey::Code(KeyCode::Escape) => return ScreenAction::Navigate(Screen::Menu),
        _ => {}
    }
    ScreenAction::None
}

pub fn update(state: &mut State, dt: f32) {
    state.selection_animation_timer += dt;
    // Wrap the animation timer to create a looping effect for the selection pulse.
    if state.selection_animation_timer > SELECTION_ANIMATION_CYCLE_DURATION {
        state.selection_animation_timer -= SELECTION_ANIMATION_CYCLE_DURATION;
    }
}

/* ==================================================================
 *                           DRAWING
 * ================================================================== */

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(256);

    // --- 0) Animated heart background using the selected color index ---
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    // --- 1) Header & Footer bars ---
    actors.push(screen_bar::build(ScreenBarParams {
        title: "SELECT MUSIC",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None,
        center_text: None,
        right_text: None,
    }));
    actors.push(screen_bar::build(ScreenBarParams {
        title: "EVENT MODE",
        title_placement: ScreenBarTitlePlacement::Center,
        position: ScreenBarPosition::Bottom,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: Some("PerfectTaste"),
        center_text: None,
        right_text: None,
    }));

    // --- 2) Primary accent box (left, flush to top of footer bar) ---
    // NOTE: keep this in sync with ScreenBar height (32.0 in screen_bar.rs)
    const BAR_H: f32 = 32.0;

    let accent = color::simply_love_rgba(state.active_color_index);
    let box_w = 373.8;
    let box_h = 60.0;

    let box_bottom = screen_height() - BAR_H;     // top edge of the bottom bar
    let box_top    = box_bottom - box_h;

    // Rectangle: bottom-left anchored at the bar’s top edge
    actors.push(act!(quad:
        align(0.0, 1.0):
        xy(0.0, box_bottom):
        zoomto(box_w, box_h):
        diffuse(accent[0], accent[1], accent[2], 1.0):
        z(50)
    ));

    // --- 3) Step Info & Rating boxes (above the accent box) ---
    const RATING_BOX_W: f32 = 31.8;
    const RATING_BOX_H: f32 = 151.8;
    const STEP_INFO_BOX_W: f32 = 286.2;
    const STEP_INFO_BOX_H: f32 = 63.6;

    const GAP: f32 = 1.8;
    const MARGIN_ABOVE_STATS_BOX: f32 = 5.4;

    let z_layer = 51; // Just above the accent box (50)

    // The bottom of these new boxes aligns to a Y position above the accent box.
    let boxes_bottom_y = box_top - MARGIN_ABOVE_STATS_BOX;

    // The rating box is right-aligned with the accent box below it.
    let rating_box_right_x = box_w;
    actors.push(act!(quad:
        align(1.0, 1.0): // bottom-right pivot
        xy(rating_box_right_x, boxes_bottom_y):
        zoomto(RATING_BOX_W, RATING_BOX_H):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0):
        z(z_layer)
    ));

    // The step info box is to the left of the rating box, with a gap.
    let step_info_box_right_x = rating_box_right_x - RATING_BOX_W - GAP;
    actors.push(act!(quad:
        align(1.0, 1.0): // bottom-right pivot
        xy(step_info_box_right_x, boxes_bottom_y):
        zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0):
        z(z_layer)
    ));

    // --- 4) Density Graph Box ---
    // Aligned to the top of the rating box, with the same dimensions as the step info box.
    let rating_box_top_y = boxes_bottom_y - RATING_BOX_H;
    actors.push(act!(quad:
        align(1.0, 0.0): // top-right pivot
        xy(step_info_box_right_x, rating_box_top_y):
        zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0):
        z(z_layer)
    ));

    // --- 5) Step Artist Box ---
    const STEP_ARTIST_BOX_W: f32 = 175.2;
    const STEP_ARTIST_BOX_H: f32 = 16.8;
    const GAP_ABOVE_DENSITY: f32 = 1.0;

    let step_artist_box_color = color::simply_love_rgba(state.active_color_index);
    let density_graph_left_x = step_info_box_right_x - STEP_INFO_BOX_W;
    let step_artist_box_bottom_y = rating_box_top_y - GAP_ABOVE_DENSITY;

    actors.push(act!(quad:
        align(0.0, 1.0): // bottom-right pivot
        xy(density_graph_left_x, step_artist_box_bottom_y):
        zoomto(STEP_ARTIST_BOX_W, STEP_ARTIST_BOX_H):
        diffuse(step_artist_box_color[0], step_artist_box_color[1], step_artist_box_color[2], 1.0):
        z(z_layer)
    ));

    // --- 6) Banner Box ---
    const BANNER_BOX_W: f32 = 319.8;
    const BANNER_BOX_H: f32 = 126.0;
    const GAP_BELOW_TOP_BAR: f32 = 1.0;

    let banner_box_top_y = BAR_H + GAP_BELOW_TOP_BAR;
    let banner_box_left_x = density_graph_left_x; // Align with boxes below

    actors.push(act!(sprite("fallback_banner.png"):
        align(0.0, 0.0): // top-left pivot
        xy(banner_box_left_x, banner_box_top_y):
        zoomto(BANNER_BOX_W, BANNER_BOX_H):
        z(z_layer)
    ));

    // --- 7) BPM/Length Box ---
    const BPM_BOX_H: f32 = 49.8;
    const GAP_BELOW_BANNER: f32 = 1.0;

    let bpm_box_top_y = banner_box_top_y + BANNER_BOX_H + GAP_BELOW_BANNER;

    actors.push(act!(quad:
        align(0.0, 0.0): // top-left pivot
        xy(banner_box_left_x, bpm_box_top_y):
        zoomto(BANNER_BOX_W, BPM_BOX_H):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0):
        z(z_layer)
    ));

    // --- 8) Music Wheel ---
    const WHEEL_W: f32 = 352.8;
    const WHEEL_ITEM_GAP: f32 = 2.0;

    let wheel_right_x = screen_width();
    let wheel_left_x = wheel_right_x - WHEEL_W;

    // Calculate vertical dimensions dynamically
    let content_area_y_start = BAR_H;
    let content_area_y_end = screen_height() - BAR_H;
    let total_available_h = content_area_y_end - content_area_y_start;

    // There are (N+1) gaps for N items (one top, one bottom, N-1 in between)
    let total_gap_h = (NUM_WHEEL_ITEMS + 1) as f32 * WHEEL_ITEM_GAP;
    let total_items_h = total_available_h - total_gap_h;
    let item_h = total_items_h / NUM_WHEEL_ITEMS as f32;

    // --- Animation timer for selection pulse ---
    let anim_t_unscaled = (state.selection_animation_timer / SELECTION_ANIMATION_CYCLE_DURATION) * std::f32::consts::PI * 2.0;
    // Sine wave for a smooth pulse (0 -> 1 -> 0)
    let anim_t = (anim_t_unscaled.sin() + 1.0) / 2.0;

    // Only draw the wheel if the calculated height is positive
    if item_h > 0.0 {
        let num_entries = state.entries.len();

        for i_slot in 0..NUM_WHEEL_ITEMS {
            let item_top_y = content_area_y_start
                + WHEEL_ITEM_GAP
                + (i_slot as f32 * (item_h + WHEEL_ITEM_GAP));

            let is_selected_slot = i_slot == CENTER_WHEEL_SLOT_INDEX;

            // --- Determine which entry to display in this slot ---
            let (display_text, box_color, text_color, text_x_pos, song_count) = if num_entries > 0 {
                let list_index = (state.selected_index as isize + i_slot as isize - CENTER_WHEEL_SLOT_INDEX as isize + num_entries as isize) as usize % num_entries;

                if let Some(entry) = state.entries.get(list_index) {
                    match entry {
                        MusicWheelEntry::Song(song_info) => {
                            let base_color = col_music_wheel_box();
                            let selected_color = col_selected_song_box();
                            let final_box_color = if is_selected_slot {
                                lerp_color(base_color, selected_color, anim_t)
                            } else {
                                base_color
                            };
                            (
                                song_info.title.clone(),
                                final_box_color,
                                [1.0, 1.0, 1.0, 1.0], // All song titles are white
                                wheel_left_x + SONG_TEXT_LEFT_PADDING,
                                None,
                            )
                        }
                        MusicWheelEntry::PackHeader { name, color: pack_color } => {
                             // Count songs in this pack for display
                            let count = state.all_entries.iter().filter(|e| {
                                if let MusicWheelEntry::Song(s) = e {
                                    // This logic is a simplification; a real implementation would need
                                    // to associate songs with packs more robustly.
                                    s.title.contains("Community") && name.contains("Community") ||
                                    s.title.contains("Extra") && name.contains("Extra") ||
                                    s.title.contains("Solo") && name.contains("Solo") ||
                                    s.title.contains("Final") && name.contains("Final")
                                } else {
                                    false
                                }
                            }).count();

                            let base_color = col_pack_header_box();
                            let selected_color = col_selected_pack_header_box();
                            let final_box_color = if is_selected_slot {
                                lerp_color(base_color, selected_color, anim_t)
                            } else {
                                base_color
                            };
                            // Center pack text
                            let text_x = wheel_left_x + 0.5 * WHEEL_W;
                            (name.clone(), final_box_color, *pack_color, text_x, Some(count))
                        }
                    }
                } else {
                    ("".to_string(), col_music_wheel_box(), [1.0; 4], wheel_left_x, None)
                }
            } else {
                ("".to_string(), col_music_wheel_box(), [1.0; 4], wheel_left_x, None)
            };

            // --- Draw Wheel Item Background ---
            actors.push(act!(quad:
                align(0.0, 0.0): // top-left pivot
                xy(wheel_left_x, item_top_y):
                zoomto(WHEEL_W, item_h):
                diffuse(box_color[0], box_color[1], box_color[2], 1.0):
                z(z_layer)
            ));

            // --- Draw Wheel Item Text ---
            let is_pack = song_count.is_some();
            if is_pack {
                actors.push(act!(text:
                    align(0.5, 0.5):
                    xy(text_x_pos, item_top_y + 0.5 * item_h):
                    zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX):
                    font("miso"): settext(display_text):
                    horizalign(center):
                    diffuse(text_color[0], text_color[1], text_color[2], 1.0):
                    z(z_layer + 1)
                ));
            } else {
                actors.push(act!(text:
                    align(0.0, 0.5):
                    xy(text_x_pos, item_top_y + 0.5 * item_h):
                    zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX):
                    font("miso"): settext(display_text):
                    horizalign(left):
                    diffuse(text_color[0], text_color[1], text_color[2], 1.0):
                    z(z_layer + 1)
                ));
            }


            // --- Draw Pack Song Count (if applicable) ---
            if let Some(count) = song_count {
                if count > 0 {
                    actors.push(act!(text:
                        align(1.0, 0.5):
                        xy(wheel_right_x - PACK_COUNT_RIGHT_PADDING, item_top_y + 0.5 * item_h):
                        zoomtoheight(PACK_COUNT_TEXT_TARGET_PX):
                        font("miso"): settext(format!("{}", count)): horizalign(right):
                        diffuse(1.0, 1.0, 1.0, 0.8):
                        z(z_layer + 1)
                    ));
                }
            }
        }
    }


    // Label inside the box (top-left, small)
    let pad_x = 10.0;
    let pad_y = 8.0;
    actors.push(act!(text:
        align(0.0, 0.0):
        xy(pad_x, box_top + pad_y):
        zoomtoheight(15.0):
        font("miso"): settext("STEP STATS"): horizalign(left):
        diffuse(1.0, 1.0, 1.0, 1.0):
        z(51)
    ));

    actors
}

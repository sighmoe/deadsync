use crate::act;
use crate::core::audio;
use crate::core::space::*;
use crate::gameplay::song::SongData;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

/* -------------------------- hold-to-scroll timing ------------------------- */
const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(300);
const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(50);

/* --------------------------------- Layout --------------------------------- */
const BAR_H: f32 = 32.0;
const LEFT_MARGIN_PX: f32 = 13.0;
const RIGHT_MARGIN_PX: f32 = 30.0;
const FIRST_ROW_TOP_MARGIN_PX: f32 = 10.0;
const VISIBLE_ROWS: usize = 12;
const ROW_H: f32 = 32.0;
const ROW_GAP: f32 = 1.0;
const LABEL_COL_W: f32 = 240.0;
const HELP_BOX_H: f32 = 40.0;
const HELP_BOX_Y_FROM_BOTTOM: f32 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavDirection {
    Up,
    Down,
}

pub struct Row {
    pub name: String,
    pub choices: Vec<String>,
    pub selected_choice_index: usize,
    pub help: Vec<String>,
}

pub struct State {
    pub song: Arc<SongData>,
    pub chart_difficulty_index: usize,
    pub rows: Vec<Row>,
    pub selected_row: usize,
    pub prev_selected_row: usize,
    pub active_color_index: i32,
    bg: heart_bg::State,
    nav_key_held_direction: Option<NavDirection>,
    nav_key_held_since: Option<Instant>,
    nav_key_last_scrolled_at: Option<Instant>,
}

fn build_rows() -> Vec<Row> {
    vec![
        Row {
            name: "Speed Mod".to_string(),
            choices: vec!["x1.00".to_string(), "x1.25".to_string(), "x1.50".to_string(), "x1.75".to_string(), "x2.00".to_string(), "C300".to_string(), "M500".to_string()],
            selected_choice_index: 0,
            help: vec!["Adjust the scroll speed of the arrows.".to_string(), "x: Multiplier | C: Constant BPM | M: Max BPM".to_string()],
        },
        Row {
            name: "Perspective".to_string(),
            choices: vec!["Overhead".to_string(), "Hallway".to_string(), "Distant".to_string(), "Incoming".to_string(), "Space".to_string()],
            selected_choice_index: 0,
            help: vec!["Changes the camera perspective.".to_string()],
        },
        Row { name: "Note Skin".to_string(), choices: vec!["cel".to_string(), "metal".to_string(), "note".to_string()], selected_choice_index: 0, help: vec!["Change the appearance of the arrows.".to_string()] },
        Row { name: "Judgment Graphic".to_string(), choices: vec!["ITG".to_string(), "Simply Love".to_string()], selected_choice_index: 1, help: vec!["Change the appearance of judgment text.".to_string()] },
        Row { name: "Combo Font".to_string(), choices: vec!["ITG".to_string(), "Simply Love".to_string()], selected_choice_index: 1, help: vec!["Change the appearance of the combo text.".to_string()] },
        Row {
            name: "Background Filter".to_string(),
            choices: vec!["Off".to_string(), "Dark".to_string(), "Darker".to_string(), "Darkest".to_string()],
            selected_choice_index: 3,
            help: vec!["Dims the background video or artwork.".to_string()],
        },
        Row { name: "Visual Delay".to_string(), choices: vec!["0ms".to_string()], selected_choice_index: 0, help: vec!["Adjust audio-visual synchronization.".to_string()] },
        Row { name: "Music Rate".to_string(), choices: vec!["1.00x".to_string()], selected_choice_index: 0, help: vec!["Change the playback speed of the song.".to_string()] },
        Row { name: "Stepchart".to_string(), choices: vec!["(Current)".to_string()], selected_choice_index: 0, help: vec!["Change to a different chart for this song.".to_string()] },
        Row {
            name: "Exit".to_string(),
            choices: vec!["Start Game".to_string(), "Go Back".to_string()],
            selected_choice_index: 0,
            help: vec!["Begin the song or return to the music wheel.".to_string()],
        },
    ]
}

pub fn init(song: Arc<SongData>, chart_difficulty_index: usize, active_color_index: i32) -> State {
    State {
        song,
        chart_difficulty_index,
        rows: build_rows(),
        selected_row: 0,
        prev_selected_row: 0,
        active_color_index,
        bg: heart_bg::State::new(),
        nav_key_held_direction: None,
        nav_key_held_since: None,
        nav_key_last_scrolled_at: None,
    }
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1100):
        linear(TRANSITION_IN_DURATION): alpha(0.0):
        linear(0.0): visible(false)
    );
    (vec![actor], TRANSITION_IN_DURATION)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

pub fn handle_key_press(state: &mut State, e: &KeyEvent) -> ScreenAction {
    let num_rows = state.rows.len();
    let key_code = if let PhysicalKey::Code(code) = e.physical_key { code } else { return ScreenAction::None };

    if e.state == ElementState::Pressed {
        if e.repeat { return ScreenAction::None; }

        match key_code {
            KeyCode::Escape => return ScreenAction::Navigate(Screen::SelectMusic),
            KeyCode::ArrowUp | KeyCode::KeyW => {
                if num_rows > 0 { state.selected_row = (state.selected_row + num_rows - 1) % num_rows; }
                state.nav_key_held_direction = Some(NavDirection::Up);
                state.nav_key_held_since = Some(Instant::now());
                state.nav_key_last_scrolled_at = Some(Instant::now());
            }
            KeyCode::ArrowDown | KeyCode::KeyS => {
                if num_rows > 0 { state.selected_row = (state.selected_row + 1) % num_rows; }
                state.nav_key_held_direction = Some(NavDirection::Down);
                state.nav_key_held_since = Some(Instant::now());
                state.nav_key_last_scrolled_at = Some(Instant::now());
            }
            KeyCode::ArrowLeft | KeyCode::KeyA => {
                if num_rows > 0 {
                    let row = &mut state.rows[state.selected_row];
                    let num_choices = row.choices.len();
                    if num_choices > 0 {
                        row.selected_choice_index = (row.selected_choice_index + num_choices - 1) % num_choices;
                        audio::play_sfx("assets/sounds/change.ogg");
                    }
                }
            }
            KeyCode::ArrowRight | KeyCode::KeyD => {
                if num_rows > 0 {
                    let row = &mut state.rows[state.selected_row];
                    let num_choices = row.choices.len();
                    if num_choices > 0 {
                        row.selected_choice_index = (row.selected_choice_index + 1) % num_choices;
                        audio::play_sfx("assets/sounds/change.ogg");
                    }
                }
            }
            KeyCode::Enter => {
                if num_rows > 0 && state.rows[state.selected_row].name == "Exit" {
                    return if state.rows[state.selected_row].selected_choice_index == 0 {
                        ScreenAction::Navigate(Screen::Gameplay)
                    } else {
                        ScreenAction::Navigate(Screen::SelectMusic)
                    };
                }
            }
            _ => {}
        }
    } else if e.state == ElementState::Released {
        if let Some(dir) = state.nav_key_held_direction {
             match (dir, key_code) {
                (NavDirection::Up, KeyCode::ArrowUp | KeyCode::KeyW) | (NavDirection::Down, KeyCode::ArrowDown | KeyCode::KeyS) => {
                    state.nav_key_held_direction = None;
                    state.nav_key_held_since = None;
                    state.nav_key_last_scrolled_at = None;
                },
                _ => {}
            }
        }
    }
    ScreenAction::None
}

pub fn update(state: &mut State, _dt: f32) {
    if let (Some(direction), Some(held_since), Some(last_scrolled_at)) =
        (state.nav_key_held_direction, state.nav_key_held_since, state.nav_key_last_scrolled_at)
    {
        let now = Instant::now();
        if now.duration_since(held_since) > NAV_INITIAL_HOLD_DELAY {
            if now.duration_since(last_scrolled_at) >= NAV_REPEAT_SCROLL_INTERVAL {
                let total = state.rows.len();
                if total > 0 {
                    match direction {
                        NavDirection::Up => state.selected_row = (state.selected_row + total - 1) % total,
                        NavDirection::Down => state.selected_row = (state.selected_row + 1) % total,
                    }
                    state.nav_key_last_scrolled_at = Some(now);
                }
            }
        }
    }

    if state.selected_row != state.prev_selected_row {
        audio::play_sfx("assets/sounds/change.ogg");
        state.prev_selected_row = state.selected_row;
    }
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(64);

    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    actors.push(screen_bar::build(ScreenBarParams {
        title: "SELECT MODIFIERS",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None, center_text: None, right_text: None, left_avatar: None,
    }));

    let content_w = screen_width() - LEFT_MARGIN_PX - RIGHT_MARGIN_PX;
    let value_col_w = content_w - LABEL_COL_W;
    let list_x = LEFT_MARGIN_PX;
    let list_y = BAR_H + FIRST_ROW_TOP_MARGIN_PX;

    let total_items = state.rows.len();
    let anchor_row: usize = 5;
    let max_offset = total_items.saturating_sub(VISIBLE_ROWS);
    let offset_rows = if total_items <= VISIBLE_ROWS { 0 } else { state.selected_row.saturating_sub(anchor_row).min(max_offset) };

    for i_vis in 0..VISIBLE_ROWS {
        let item_idx = offset_rows + i_vis;
        if item_idx >= total_items { break; }

        let row_y = list_y + (i_vis as f32) * (ROW_H + ROW_GAP);
        let is_active = item_idx == state.selected_row;
        let row = &state.rows[item_idx];

        let active_bg = color::rgba_hex("#333333");
        let inactive_bg_base = color::rgba_hex("#071016");
        let label_bg_base = color::rgba_hex("#000000");
        let active_text_color = color::simply_love_rgba(state.active_color_index);
        let exit_bg = color::simply_love_rgba(state.active_color_index);

        let (val_bg_col, text_col, val_bg_alpha) = if is_active {
            (if row.name == "Exit" { exit_bg } else { active_bg }, if row.name == "Exit" { [0.0, 0.0, 0.0, 1.0] } else { active_text_color }, 1.0)
        } else {
            (inactive_bg_base, [1.0, 1.0, 1.0, 1.0], 0.8)
        };

        actors.push(act!(quad: align(0.0, 0.0): xy(list_x, row_y): zoomto(LABEL_COL_W, ROW_H): diffuse(label_bg_base[0], label_bg_base[1], label_bg_base[2], 0.4) ));
        actors.push(act!(quad: align(0.0, 0.0): xy(list_x + LABEL_COL_W, row_y): zoomto(value_col_w, ROW_H): diffuse(val_bg_col[0], val_bg_col[1], val_bg_col[2], val_bg_alpha) ));

        let text_h = 20.0;
        let row_mid_y = row_y + 0.5 * ROW_H;

        actors.push(act!(text: font("miso"): settext(row.name.clone()): align(0.0, 0.5): xy(list_x + 15.0, row_mid_y): zoomtoheight(text_h): diffuse(1.0, 1.0, 1.0, 1.0) ));
        if let Some(choice_text) = row.choices.get(row.selected_choice_index) {
             actors.push(act!(text: font("miso"): settext(choice_text.clone()): align(0.5, 0.5): xy(list_x + LABEL_COL_W + 0.5 * value_col_w, row_mid_y): zoomtoheight(text_h): maxwidth(value_col_w - 20.0): horizalign(center): diffuse(text_col[0], text_col[1], text_col[2], text_col[3]) ));
        }
    }

    let help_text_y = screen_height() - HELP_BOX_Y_FROM_BOTTOM - HELP_BOX_H;
    let help_bg_base_color = color::rgba_hex("#000000");
    actors.push(act!(quad: align(0.0, 0.0): xy(LEFT_MARGIN_PX, help_text_y): zoomto(content_w, HELP_BOX_H): diffuse(help_bg_base_color[0], help_bg_base_color[1], help_bg_base_color[2], 0.8) ));

    if let Some(row) = state.rows.get(state.selected_row) {
        let help_text = row.help.join(" | ");
        actors.push(act!(text: font("miso"): settext(help_text): align(0.0, 0.5): xy(LEFT_MARGIN_PX + 15.0, help_text_y + 0.5 * HELP_BOX_H): zoom(0.75): diffuse(1.0, 1.0, 1.0, 1.0): maxwidth(content_w - 30.0)));
    }

    actors
}
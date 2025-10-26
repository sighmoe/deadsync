use crate::act;
use crate::core::audio;
use crate::core::space::*;
use crate::gameplay::song::SongData;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{
    self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement,
};
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

pub struct SpeedMod {
    pub mod_type: String, // "X", "C", "M"
    pub value: f32,
}

pub struct State {
    pub song: Arc<SongData>,
    pub chart_difficulty_index: usize,
    pub rows: Vec<Row>,
    pub selected_row: usize,
    pub prev_selected_row: usize,
    pub active_color_index: i32,
    pub speed_mod: SpeedMod,
    bg: heart_bg::State,
    nav_key_held_direction: Option<NavDirection>,
    nav_key_held_since: Option<Instant>,
    nav_key_last_scrolled_at: Option<Instant>,
}

fn build_rows(speed_mod: &SpeedMod) -> Vec<Row> {
    let speed_mod_value_str = match speed_mod.mod_type.as_str() {
        "X" => format!("{:.2}x", speed_mod.value),
        "C" => format!("C{}", speed_mod.value as i32),
        "M" => format!("M{}", speed_mod.value as i32),
        _ => "".to_string(),
    };

    vec![
        Row {
            name: "Speed Mod Type".to_string(),
            choices: vec![
                "X (multiplier)".to_string(),
                "C (constant)".to_string(),
                "M (maximum)".to_string(),
            ],
            selected_choice_index: match speed_mod.mod_type.as_str() {
                "X" => 0,
                "C" => 1,
                "M" => 2,
                _ => 1, // Default to C
            },
            help: vec![
                "Adjust the scroll speed of the arrows.".to_string(),
                "x: Multiplier | C: Constant BPM | M: Max BPM".to_string(),
            ],
        },
        Row {
            name: "Speed Mod".to_string(),
            choices: vec![speed_mod_value_str], // Display only the current value
            selected_choice_index: 0,
            help: vec!["Use Left/Right to adjust the speed value.".to_string()],
        },
        Row {
            name: "Perspective".to_string(),
            choices: vec![
                "Overhead".to_string(),
                "Hallway".to_string(),
                "Distant".to_string(),
                "Incoming".to_string(),
                "Space".to_string(),
            ],
            selected_choice_index: 0,
            help: vec!["Changes the camera perspective.".to_string()],
        },
        Row {
            name: "Mini".to_string(),
            choices: vec![
                "0%".to_string(),
            ],
            selected_choice_index: 0,
            help: vec!["Changes the camera perspective.".to_string()],
        },
        Row {
            name: "Note Skin".to_string(),
            choices: vec!["cel".to_string(), "metal".to_string(), "note".to_string()],
            selected_choice_index: 0,
            help: vec!["Change the appearance of the arrows.".to_string()],
        },
        Row {
            name: "Background Filter".to_string(),
            choices: vec![
                "Off".to_string(),
                "Dark".to_string(),
                "Darker".to_string(),
                "Darkest".to_string(),
            ],
            selected_choice_index: 3,
            help: vec!["Dims the background video or artwork.".to_string()],
        },
        Row {
            name: "Visual Delay".to_string(),
            choices: vec!["0ms".to_string()],
            selected_choice_index: 0,
            help: vec!["Adjust audio-visual synchronization.".to_string()],
        },
        Row {
            name: "Music Rate".to_string(),
            choices: vec!["1.00x".to_string()],
            selected_choice_index: 0,
            help: vec!["Change the playback speed of the song.".to_string()],
        },
        Row {
            name: "Stepchart".to_string(),
            choices: vec!["(Current)".to_string()],
            selected_choice_index: 0,
            help: vec!["Change to a different chart for this song.".to_string()],
        },
        Row {
            name: "Exit".to_string(),
            choices: vec!["Start Game".to_string(), "Go Back".to_string()],
            selected_choice_index: 0,
            help: vec!["Begin the song or return to the music wheel.".to_string()],
        },
    ]
}

pub fn init(song: Arc<SongData>, chart_difficulty_index: usize, active_color_index: i32) -> State {
    let profile = crate::gameplay::profile::get();
    let speed_mod = match profile.scroll_speed {
        crate::gameplay::profile::ScrollSpeedSetting::CMod(bpm) => SpeedMod {
            mod_type: "C".to_string(),
            value: bpm,
        },
        crate::gameplay::profile::ScrollSpeedSetting::XMod(mult) => SpeedMod {
            mod_type: "X".to_string(),
            value: mult,
        },
        crate::gameplay::profile::ScrollSpeedSetting::MMod(bpm) => SpeedMod {
            mod_type: "M".to_string(),
            value: bpm,
        },
    };

    State {
        song,
        chart_difficulty_index,
        rows: build_rows(&speed_mod),
        selected_row: 0,
        prev_selected_row: 0,
        active_color_index,
        speed_mod,
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
    let key_code = if let PhysicalKey::Code(code) = e.physical_key {
        code
    } else {
        return ScreenAction::None;
    };

    if e.state == ElementState::Pressed {
        if e.repeat {
            return ScreenAction::None;
        }

        match key_code {
            KeyCode::Escape => return ScreenAction::Navigate(Screen::SelectMusic),
            KeyCode::ArrowUp | KeyCode::KeyW => {
                if num_rows > 0 {
                    state.selected_row = (state.selected_row + num_rows - 1) % num_rows;
                }
                state.nav_key_held_direction = Some(NavDirection::Up);
                state.nav_key_held_since = Some(Instant::now());
                state.nav_key_last_scrolled_at = Some(Instant::now());
            }
            KeyCode::ArrowDown | KeyCode::KeyS => {
                if num_rows > 0 {
                    state.selected_row = (state.selected_row + 1) % num_rows;
                }
                state.nav_key_held_direction = Some(NavDirection::Down);
                state.nav_key_held_since = Some(Instant::now());
                state.nav_key_last_scrolled_at = Some(Instant::now());
            }
            KeyCode::ArrowLeft | KeyCode::KeyA | KeyCode::ArrowRight | KeyCode::KeyD => {
                let row = &mut state.rows[state.selected_row];
                if row.name == "Speed Mod" {
                    let delta_dir = if key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA {
                        -1.0
                    } else {
                        1.0
                    };
                    let speed_mod = &mut state.speed_mod;
                    let (upper, increment) = match speed_mod.mod_type.as_str() {
                        "X" => (20.0, 0.05),
                        "C" | "M" => (2000.0, 5.0),
                        _ => (1.0, 0.1),
                    };
                    speed_mod.value += delta_dir * increment;
                    speed_mod.value = (speed_mod.value / increment).round() * increment;
                    speed_mod.value = speed_mod.value.clamp(increment, upper);

                    let speed_mod_value_str = match speed_mod.mod_type.as_str() {
                        "X" => format!("{:.2}x", speed_mod.value),
                        "C" => format!("C{}", speed_mod.value as i32),
                        "M" => format!("M{}", speed_mod.value as i32),
                        _ => "".to_string(),
                    };
                    row.choices[0] = speed_mod_value_str;
                    audio::play_sfx("assets/sounds/change.ogg");
                } else {
                    let num_choices = row.choices.len();
                    if num_choices > 0 {
                        row.selected_choice_index =
                            if key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA {
                                (row.selected_choice_index + num_choices - 1) % num_choices
                            } else {
                                (row.selected_choice_index + 1) % num_choices
                            };

                        if row.name == "Speed Mod Type" {
                            let new_type = match row.selected_choice_index {
                                0 => "X",
                                1 => "C",
                                2 => "M",
                                _ => "C",
                            };
                            state.speed_mod.mod_type = new_type.to_string();

                            // Reset value to a default for the new type
                            let new_value = match new_type {
                                "X" => 1.0,
                                "C" => 600.0,
                                "M" => 600.0,
                                _ => 600.0,
                            };
                            state.speed_mod.value = new_value;

                            // Format the new value string
                            let speed_mod_value_str = match new_type {
                                "X" => format!("{:.2}x", new_value),
                                "C" => format!("C{}", new_value as i32),
                                "M" => format!("M{}", new_value as i32),
                                _ => "".to_string(),
                            };

                            // Update the choices vec for the "Speed Mod" row.
                            if let Some(speed_mod_row) = state.rows.get_mut(1) {
                                if speed_mod_row.name == "Speed Mod" {
                                    speed_mod_row.choices[0] = speed_mod_value_str;
                                }
                            }
                        }
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
                (NavDirection::Up, KeyCode::ArrowUp | KeyCode::KeyW)
                | (NavDirection::Down, KeyCode::ArrowDown | KeyCode::KeyS) => {
                    state.nav_key_held_direction = None;
                    state.nav_key_held_since = None;
                    state.nav_key_last_scrolled_at = None;
                }
                _ => {}
            }
        }
    }
    ScreenAction::None
}

pub fn update(state: &mut State, _dt: f32) {
    if let (Some(direction), Some(held_since), Some(last_scrolled_at)) = (
        state.nav_key_held_direction,
        state.nav_key_held_since,
        state.nav_key_last_scrolled_at,
    ) {
        let now = Instant::now();
        if now.duration_since(held_since) > NAV_INITIAL_HOLD_DELAY {
            if now.duration_since(last_scrolled_at) >= NAV_REPEAT_SCROLL_INTERVAL {
                let total = state.rows.len();
                if total > 0 {
                    match direction {
                        NavDirection::Up => {
                            state.selected_row = (state.selected_row + total - 1) % total
                        }
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
        left_text: None,
        center_text: None,
        right_text: None,
        left_avatar: None,
    }));

    // Speed Mod Helper Display (from overlay.lua)
    let speed_mod_y = 48.0;
    let speed_mod_x = screen_center_x() + widescale(-77.0, -100.0);
    let speed_color = color::simply_love_rgba(state.active_color_index);
    let speed_text = if state.speed_mod.mod_type == "X" {
        format!("{:.2}x", state.speed_mod.value)
    } else {
        format!("{}{}", state.speed_mod.mod_type, state.speed_mod.value as i32)
    };
    actors.push(act!(text: font("wendy"): settext(speed_text):
        align(0.0, 0.5): xy(speed_mod_x, speed_mod_y): zoom(0.5):
        diffuse(speed_color[0], speed_color[1], speed_color[2], 1.0):
        z(121)
    ));

    /* ---------- SHARED GEOMETRY (rows aligned to help box) ---------- */
    // Help Text Box (from underlay.lua) — define this first so rows can match its width/left.
    let help_box_h = 40.0;
    let help_box_w = widescale(614.0, 792.0);
    let help_box_x = widescale(13.0, 30.666);
    let help_box_bottom_y = screen_height() - 36.0;

    // Row layout constants
    const ROW_START_OFFSET: f32 = -164.0;
    const ROW_HEIGHT: f32 = 33.0;
    const ROW_GAP: f32 = 1.0;
    const TITLE_BG_WIDTH: f32 = 115.0;

    let frame_h = ROW_HEIGHT;
    let row_gap = ROW_GAP;

    // Make row frame LEFT and WIDTH exactly match the help box.
    let row_left = help_box_x;
    let row_width = help_box_w;
    let row_center_x = row_left + (row_width * 0.5);
    let title_bg_center_x = row_left + (TITLE_BG_WIDTH * 0.5);

    // Title text x: a bit of padding inside the title strip
    let title_x = row_left + widescale(13.0, 24.0);

    // Start first row exactly at the requested offset
    let mut current_row_y = screen_center_y() + ROW_START_OFFSET;

    for i in 0..state.rows.len() {
        let is_active = i == state.selected_row;
        let row = &state.rows[i];

        let active_bg = color::rgba_hex("#333333");
        let inactive_bg_base = color::rgba_hex("#071016");
        let exit_bg = color::simply_love_rgba(state.active_color_index);
        let bg_color = if is_active {
            if row.name == "Exit" { exit_bg } else { active_bg }
        } else {
            [inactive_bg_base[0], inactive_bg_base[1], inactive_bg_base[2], 0.8]
        };

        // Row background — matches help box width & left
        actors.push(act!(quad:
            align(0.5, 0.5): xy(row_center_x, current_row_y):
            zoomto(row_width, frame_h):
            diffuse(bg_color[0], bg_color[1], bg_color[2], bg_color[3]):
            z(100)
        ));

        if row.name != "Exit" {
            actors.push(act!(quad:
                align(0.5, 0.5): xy(title_bg_center_x, current_row_y):
                zoomto(TITLE_BG_WIDTH, frame_h):
                diffuse(0.0, 0.0, 0.0, 0.25):
                z(101)
            ));
        }

        let title_color = if is_active && row.name != "Exit" {
            color::simply_love_rgba(state.active_color_index)
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        actors.push(act!(text: font("miso"): settext(row.name.clone()):
            align(0.0, 0.5): xy(title_x, current_row_y): zoom(0.8):
            diffuse(title_color[0], title_color[1], title_color[2], title_color[3]):
            horizalign(left): maxwidth(widescale(128.0, 120.0)):
            z(101)
        ));

        // Choice text: keep it inside the row frame; position it with some inner padding
        // (Using widescale so it breathes similarly across ARs.)
        let choice_inner_left = row_left + widescale(150.0, 180.0);
        let choice_text = &row.choices[row.selected_choice_index];
        let choice_color = if is_active && row.name == "Exit" {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        actors.push(act!(text: font("miso"): settext(choice_text.clone()):
            align(0.0, 0.5): xy(choice_inner_left, current_row_y): zoom(0.75):
            diffuse(choice_color[0], choice_color[1], choice_color[2], choice_color[3]):
            z(101)
        ));

        current_row_y += frame_h + row_gap;
    }

    // Help Text Box (render) — uses the same geometry the rows used
    actors.push(act!(quad:
        align(0.0, 1.0): xy(help_box_x, help_box_bottom_y):
        zoomto(help_box_w, help_box_h):
        diffuse(0.0, 0.0, 0.0, 0.8)
    ));

    if let Some(row) = state.rows.get(state.selected_row) {
        let help_text = row.help.join(" | ");
        let help_text_color = color::simply_love_rgba(state.active_color_index);
        let wrap_width = help_box_w - 30.0; // padding

        actors.push(act!(text:
            font("miso"): settext(help_text):
            align(0.0, 0.5):
            xy(help_box_x + 15.0, help_box_bottom_y - (help_box_h / 2.0)):
            zoom(widescale(0.7, 0.75)):
            diffuse(help_text_color[0], help_text_color[1], help_text_color[2], 1.0):
            maxwidth(wrap_width): horizalign(left)
        ));
    }

    actors
}

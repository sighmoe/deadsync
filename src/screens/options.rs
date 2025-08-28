use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::{heart_bg, screen_bar};
use crate::ui::components::screen_bar::{ScreenBarPosition, ScreenBarTitlePlacement};

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Bars in `screen_bar.rs` use 32.0 px height.
const BAR_H: f32 = 32.0;

/// Unscaled spec constants (we’ll uniformly scale to the available content rect).
const ROW_COUNT: usize = 10;
const ROW_H: f32 = 55.0;
const ROW_GAP: f32 = 3.0;
const LIST_W: f32 = 721.0;

const SEP_W: f32 = 3.0;     // gap/stripe between rows and description
const DESC_W: f32 = 484.0;  // description panel width
const DESC_H: f32 = 584.0;  // total block height

/// Text sizing (unscaled). Picked to sit nicely inside 55px rows.
const TEXT_PX: f32 = 26.0;
const TEXT_LEFT_PAD: f32 = 16.0; // padding inside a row before the heart
const HEART_TEXT_GAP: f32 = 10.0;

/// Heart native aspect (for aspect-correct scaling).
const HEART_NATIVE_W: f32 = 668.0;
const HEART_NATIVE_H: f32 = 566.0;
const HEART_ASPECT: f32 = HEART_NATIVE_W / HEART_NATIVE_H;

/// A simple item model with help text for the description box.
struct Item<'a> {
    name: &'a str,
    help: &'a [&'a str],
}

const ITEMS: &[Item] = &[
    Item { name: "System Options",                  help: &["Game", "Theme", "Language", "Announcer", "Default NoteSkin", "Editor Noteskin"] },
    Item { name: "Configure Keyboard/Pad Mappings", help: &["Bind keys/buttons for each player."] },
    Item { name: "Test Input",                      help: &["View live input state for debugging."] },
    Item { name: "Input Options",                   help: &["Debounce, menu buttons, coin mode…"] },
    Item { name: "Graphics/Sound Options",          help: &["Resolution, VSync, sound device…"] },
    Item { name: "Visual Options",                  help: &["Judgment, combo, lifebar, etc."] },
    Item { name: "Arcade Options",                  help: &["Coin mode, premium, attract mode…"] },
    Item { name: "View Bookkeeping Data",           help: &["Audit play counts, coins, uptime."] },
    Item { name: "Advanced Options",                help: &["Low-level engine toggles."] },
    Item { name: "MenuTimer Options",               help: &["Per-screen time limits."] },
];

pub struct State {
    pub selected: usize,
    bg: heart_bg::State,
}

pub fn init() -> State {
    State { selected: 0, bg: heart_bg::State::new() }
}

/* --------------------------------- input --------------------------------- */

pub fn handle_key_press(state: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state != ElementState::Pressed || e.repeat {
        return ScreenAction::None;
    }
    match e.physical_key {
        PhysicalKey::Code(KeyCode::Escape) => ScreenAction::Navigate(Screen::Menu),
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            if state.selected == 0 { state.selected = ROW_COUNT - 1; } else { state.selected -= 1; }
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected = (state.selected + 1) % ROW_COUNT;
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            // No navigation yet; stub for future actions.
            ScreenAction::None
        }
        _ => ScreenAction::None,
    }
}

/* --------------------------------- layout -------------------------------- */

/// content rect = full screen minus top & bottom bars.
/// Returns (scale, origin_x, origin_y) for the block inside that content rect.
fn scaled_block_origin() -> (f32, f32, f32) {
    let total_w = LIST_W + SEP_W + DESC_W;
    let total_h = DESC_H;

    let sw = screen_width();
    let sh = screen_height();

    // available height excluding bars
    let avail_h = (sh - 2.0 * BAR_H).max(0.0);
    let s = (sw / total_w).min(avail_h / total_h);
    let ox = 0.5 * (sw - total_w * s);
    let oy = BAR_H + 0.5 * (avail_h - total_h * s);
    (s, ox, oy)
}

fn list_total_height() -> f32 {
    (ROW_COUNT as f32) * ROW_H + (ROW_COUNT.saturating_sub(1) as f32) * ROW_GAP
}

/* -------------------------------- drawing -------------------------------- */

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut v: Vec<Actor> = Vec::with_capacity(320);

    /* -------------------------- HEART BACKGROUND -------------------------- */
    v.extend(state.bg.build(heart_bg::Params {
        active_color_index: color::DEFAULT_COLOR_INDEX,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    /* ------------------------------ TOP BAR ------------------------------- */
    const FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    v.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "OPTIONS",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        fg_color: FG,
    }));

    /* ----------------------------- BOTTOM BAR ----------------------------- */
    v.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "EVENT MODE",
        title_placement: ScreenBarTitlePlacement::Center,
        position: ScreenBarPosition::Bottom,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        fg_color: FG,
    }));

    /* --------------------------- MAIN CONTENT UI -------------------------- */

    // --- global colors ---
    let col_active_bg   = color::rgba_hex("#333333");
    let col_inactive_bg = [0.0, 0.0, 0.0, 0.5];
    let col_white       = [1.0, 1.0, 1.0, 1.0];

    // Active text color uses the Simply Love palette at the selected index.
    let col_active_text = color::simply_love_rgba(state.selected as i32);

    // --- scale & origin (within content area between the bars) ---
    let (s, ox, oy) = scaled_block_origin();

    // Geometry (scaled)
    let list_w = LIST_W * s;
    let sep_w  = SEP_W * s;
    let desc_w = DESC_W * s;
    let desc_h = DESC_H * s;

    let block_x = ox;
    let block_y = oy;

    // Rows area (LEFT)
    let list_x = block_x;
    let list_h_unscaled = list_total_height();
    let list_h = list_h_unscaled * s;
    let list_y = block_y + 0.5 * (desc_h - list_h); // vertically centered inside desc block

    // Separator immediately to the RIGHT of the rows
    v.push(act!(quad:
        align(0.0, 0.0):
        xy(list_x + list_w, block_y):
        zoomto(sep_w, desc_h):
        diffuse(col_active_bg[0], col_active_bg[1], col_active_bg[2], col_active_bg[3]) // #333333
    ));

    // Description box (RIGHT of separator)
    let desc_x = list_x + list_w + sep_w;
    v.push(act!(quad:
        align(0.0, 0.0):
        xy(desc_x, block_y):
        zoomto(desc_w, desc_h):
        diffuse(col_active_bg[0], col_active_bg[1], col_active_bg[2], col_active_bg[3]) // #333333
    ));

    // Row loop (backgrounds + content)
    for i in 0..ROW_COUNT {
        let row_y = list_y + (i as f32) * (ROW_H + ROW_GAP) * s;

        // Row background
        let is_active = i == state.selected;
        let bg = if is_active { col_active_bg } else { col_inactive_bg };

        v.push(act!(quad:
            align(0.0, 0.0):
            xy(list_x, row_y):
            zoomto(list_w, ROW_H * s):
            diffuse(bg[0], bg[1], bg[2], bg[3])
        ));

        // Content placement inside row
        let row_mid_y = row_y + 0.5 * ROW_H * s;
        let text_h    = TEXT_PX * s;

        // Heart same height as text
        let heart_h = text_h;
        let heart_w = heart_h * HEART_ASPECT;

        let content_left = list_x + TEXT_LEFT_PAD * s;

        // Heart sprite (left of text)
        v.push(act!(sprite("heart.png"):
            align(0.0, 0.5):
            xy(content_left, row_mid_y):
            zoomto(heart_w, heart_h):
            diffuse(1.0, 1.0, 1.0, 1.0)
        ));

        // Text (Miso)
        let text_x = content_left + heart_w + HEART_TEXT_GAP * s;
        let label  = ITEMS[i].name;
        let color_t = if is_active { col_active_text } else { col_white };

        v.push(act!(text:
            align(0.0, 0.0):
            xy(text_x, row_mid_y - 0.5 * text_h):
            zoomtoheight(text_h):
            diffuse(color_t[0], color_t[1], color_t[2], color_t[3]):
            font("miso"):
            settext(label):
            horizalign(left)
        ));
    }

    // ------------------- Description content (selected) -------------------
    let sel = state.selected.min(ITEMS.len() - 1);
    let title_px = 28.0 * s;
    let body_px  = 18.0 * s;

    let desc_pad_x = 18.0 * s;
    let mut cursor_y = block_y + 18.0 * s;

    // Title (selected item name)
    v.push(act!(text:
        align(0.0, 0.0):
        xy(desc_x + desc_pad_x, cursor_y):
        zoomtoheight(title_px):
        diffuse(1.0, 1.0, 1.0, 1.0):
        font("miso"): settext(ITEMS[sel].name):
        horizalign(left)
    ));
    cursor_y += title_px + 12.0 * s;

    // Help bullets
    for &line in ITEMS[sel].help {
        // bullet
        v.push(act!(text:
            align(0.0, 0.0):
            xy(desc_x + desc_pad_x, cursor_y):
            zoomtoheight(body_px):
            diffuse(1.0, 1.0, 1.0, 1.0):
            font("miso"): settext("•"):
            horizalign(left)
        ));
        // text
        v.push(act!(text:
            align(0.0, 0.0):
            xy(desc_x + desc_pad_x + 12.0 * s, cursor_y):
            zoomtoheight(body_px):
            diffuse(1.0, 1.0, 1.0, 1.0):
            font("miso"): settext(line):
            horizalign(left)
        ));
        cursor_y += body_px + 8.0 * s;
    }

    v
}

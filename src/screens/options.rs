use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::{heart_bg, screen_bar};
use crate::ui::components::screen_bar::{ScreenBarPosition, ScreenBarTitlePlacement};

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* ========================================================================== */
/*                                STYLE TOKENS                                */
/* ========================================================================== */

/// Simply Love-ish palette (tuned for your Wendy/Miso pair).
mod style {
    // Left list panel
    pub const PANEL_BG:     [f32; 4] = [0.09411765, 0.10588235, 0.1254902, 1.0];   // #181B20
    pub const PANEL_EDGE:   [f32; 4] = [0.15686275, 0.17254902, 0.2,       1.0];   // #282C33
    pub const SEPARATOR:    [f32; 4] = [0.11764706, 0.14117648, 0.1882353, 1.0];   // #1E2430

    pub const TEXT_NORMAL:  [f32; 4] = [0.90588236, 0.93333334, 0.9647059, 1.0];   // #E7EEF6
    pub const TEXT_DIM:     [f32; 4] = [0.7176471,  0.7607843,  0.8,       1.0];   // #B7C2CC
    pub const TEXT_HINT:    [f32; 4] = [0.78039217, 0.8117647,  0.8392157, 1.0];   // #C7CFD6

    pub const SELECT_FILL:  [f32; 4] = [0.3529412,  0.654902,   1.0,       1.0];   // #5AA7FF
    pub const SELECT_EDGE:  [f32; 4] = [0.56078434, 0.7647059,  1.0,       1.0];   // #8FC3FF
    pub const TEXT_SELECT:  [f32; 4] = [0.043137256,0.1254902,  0.22745098,1.0];   // #0B203A

    // Right help panel
    pub const HELP_BG:      [f32; 4] = [0.17254902, 0.19607843, 0.23137255,1.0];   // #2C323B
    pub const HELP_EDGE:    [f32; 4] = [0.26666668, 0.29803923, 0.34117648,1.0];   // #444C57
    pub const HELP_TEXT:    [f32; 4] = TEXT_NORMAL;
    pub const HELP_BULLET:  [f32; 4] = TEXT_HINT;

    // Tiny scrollbar stub color
    pub const SCROLL_STUB:  [f32; 4] = [0.42352942, 0.46666667, 0.5254902, 1.0];   // #6C7786
}

/* ========================================================================== */
/*                              LAYOUT CONSTANTS                              */
/* ========================================================================== */

const TOP_BAR_H: f32 = 32.0;
const BOT_BAR_H: f32 = 32.0;

const SAFE_MARGIN_X: f32 = 24.0;
const SAFE_MARGIN_Y: f32 = 10.0;

/// height available for the panels
fn content_rect() -> (f32, f32, f32, f32) {
    let w = screen_width();
    let h = screen_height();
    let x = SAFE_MARGIN_X;
    let y = TOP_BAR_H + SAFE_MARGIN_Y;
    let hh = h - TOP_BAR_H - BOT_BAR_H - SAFE_MARGIN_Y * 2.0;
    let ww = w - SAFE_MARGIN_X * 2.0;
    (x, y, ww, hh)
}

// Left panel sizing tuned for 854x480 design space.
const GAP_BETWEEN_PANELS: f32 = 16.0;
const LEFT_PANEL_W: f32 = 540.0;

// List rows
const ROW_H: f32 = 44.0;
const ROW_TEXT_WENDY: f32 = 26.0;
const ROW_VALUE_MISO: f32 = 20.0;
const ROW_SIDE_PAD: f32 = 18.0;
const HEART_BULLET_W: f32 = 20.0;
const HEART_BULLET_H: f32 = 18.0;
const HEART_GAP_AFTER: f32 = 10.0;

/// bottom-left hint text
const HINT_PX: f32 = 14.0;

/* ========================================================================== */
/*                               DATA & STATE                                 */
/* ========================================================================== */

struct Item<'a> {
    name: &'a str,
    value: &'a str,
    help: &'a [&'a str], // bullet list on right panel
}

const ITEMS: &[Item] = &[
    Item { name: "System Options",                 value: "", help: &["Game", "Theme", "Language", "Announcer", "Default NoteSkin", "Editor Noteskin"] },
    Item { name: "Configure Keyboard/Pad Mappings",value: "", help: &["Bind keys/buttons for each player."] },
    Item { name: "Test Input",                     value: "", help: &["View live input state for debugging."] },
    Item { name: "Input Options",                  value: "", help: &["Debounce, menu buttons, coin mode…"] },
    Item { name: "Graphics/Sound Options",         value: "", help: &["Resolution, VSync, sound device…"] },
    Item { name: "Visual Options",                 value: "", help: &["Judgment, combo, lifebar, etc."] },
    Item { name: "Arcade Options",                 value: "", help: &["Coin mode, premium, attract mode…"] },
    Item { name: "View Bookkeeping Data",          value: "", help: &["Audit play counts, coins, uptime."] },
    Item { name: "Advanced Options",               value: "", help: &["Low-level engine toggles."] },
    Item { name: "MenuTimer Options",              value: "", help: &["Per-screen time limits."] },
];

pub struct State {
    pub selected: usize,
    bg: heart_bg::State,
}

pub fn init() -> State {
    State { selected: 0, bg: heart_bg::State::new() }
}

/* ========================================================================== */
/*                                   INPUT                                    */
/* ========================================================================== */

pub fn handle_key_press(state: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state != ElementState::Pressed || e.repeat {
        return ScreenAction::None;
    }
    match e.physical_key {
        PhysicalKey::Code(KeyCode::Escape) => ScreenAction::Navigate(Screen::Menu),
        PhysicalKey::Code(KeyCode::ArrowUp)   | PhysicalKey::Code(KeyCode::KeyW) => {
            if state.selected == 0 { state.selected = ITEMS.len() - 1; } else { state.selected -= 1; }
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected = (state.selected + 1) % ITEMS.len();
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            // Shell only (no navigation yet)
            ScreenAction::None
        }
        _ => ScreenAction::None,
    }
}

/* ========================================================================== */
/*                                   DRAW                                     */
/* ========================================================================== */

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(256);

    /* -------- background hearts ---------- */
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: color::DEFAULT_COLOR_INDEX,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    /* ---------------- bars ---------------- */
    const FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "OPTIONS",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        fg_color: FG,
    }));
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "EVENT MODE",
        title_placement: ScreenBarTitlePlacement::Center,
        position: ScreenBarPosition::Bottom,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        fg_color: FG,
    }));

    /* --------------- content rect --------------- */
    let (cx, cy, cw, ch) = content_rect();

    // Left & right panels
    let left_w  = LEFT_PANEL_W.min(cw - GAP_BETWEEN_PANELS - 160.0);
    let right_w = cw - left_w - GAP_BETWEEN_PANELS;

    /* ---------------- left list panel ---------------- */
    actors.extend(build_panel_with_border(
        cx, cy, left_w, ch, style::PANEL_BG, style::PANEL_EDGE, 1.0, -10,
    ));

    // Inner list rows
    let rows_top = cy + 1.0;
    let visible_rows = (ch / ROW_H).floor() as usize; // we won’t scroll yet
    let count = ITEMS.len().min(visible_rows);

    // Column x for content
    let x_l = cx + ROW_SIDE_PAD; // where the heart bullet starts
    let x_name = x_l + HEART_BULLET_W + HEART_GAP_AFTER;
    let x_value_right = cx + left_w - ROW_SIDE_PAD;

    // separators
    for i in 0..=count {
        let y = rows_top + (i as f32) * ROW_H;
        actors.push(act!(quad:
            align(0.0, 0.0): xy(cx, y - 0.5):
            zoomto(left_w, 1.0):
            diffuse(style::SEPARATOR[0], style::SEPARATOR[1], style::SEPARATOR[2], 1.0):
            z(5)
        ));
    }

    // selection highlight
    let sel_i = state.selected.min(count.saturating_sub(1));
    let sel_y = rows_top + (sel_i as f32) * ROW_H;

    // fill
    actors.push(act!(quad:
        align(0.0, 0.0): xy(cx + 2.0, sel_y + 2.0):
        zoomto(left_w - 4.0, ROW_H - 4.0):
        diffuse(style::SELECT_FILL[0], style::SELECT_FILL[1], style::SELECT_FILL[2], 1.0):
        z(9)
    ));
    // outline
    actors.push(act!(quad:
        align(0.0, 0.0): xy(cx + 1.0, sel_y + 1.0):
        zoomto(left_w - 2.0, ROW_H - 2.0):
        diffuse(style::SELECT_EDGE[0], style::SELECT_EDGE[1], style::SELECT_EDGE[2], 0.9):
        cropleft(0.0): cropright(0.0): croptop(0.0): cropbottom(0.0):
        z(10)
    ));

    // rows content
    for i in 0..count {
        let row = &ITEMS[i];
        let y_mid = rows_top + (i as f32) * ROW_H + 0.5 * ROW_H;

        // bullet heart
        actors.push(act!(sprite("heart.png"):
            align(0.0, 0.5):
            xy(x_l, y_mid):
            zoomto(HEART_BULLET_W, HEART_BULLET_H):
            diffuse(1.0, 1.0, 1.0, if i == sel_i { 1.0 } else { 0.85 }):
            z(12)
        ));

        // label
        let label_col = if i == sel_i { style::TEXT_SELECT } else { style::TEXT_NORMAL };
        actors.push(act!(text:
            align(0.0, 0.5):
            xy(x_name, y_mid - 0.5 * ROW_TEXT_WENDY): // text actor expects top-left
            zoomtoheight(ROW_TEXT_WENDY):
            font("wendy"): settext(row.name):
            diffuse(label_col[0], label_col[1], label_col[2], 1.0):
            horizalign(left):
            z(13)
        ));

        // value (right aligned)
        if !row.value.is_empty() {
            let vcol = if i == sel_i { style::TEXT_SELECT } else { style::TEXT_DIM };
            actors.push(act!(text:
                align(1.0, 0.5):
                xy(x_value_right, y_mid - 0.5 * ROW_VALUE_MISO):
                zoomtoheight(ROW_VALUE_MISO):
                font("miso"): settext(row.value):
                diffuse(vcol[0], vcol[1], vcol[2], 1.0):
                horizalign(right):
                z(13)
            ));
        }
    }

    /* ---------------- right help panel ---------------- */
    let rx = cx + left_w + GAP_BETWEEN_PANELS;
    actors.extend(build_panel_with_border(
        rx, cy, right_w, ch, style::HELP_BG, style::HELP_EDGE, 1.0, -10,
    ));

    // Title (selected item)
    let title = ITEMS[sel_i].name;
    actors.push(act!(text:
        align(0.0, 0.0):
        xy(rx + 18.0, cy + 14.0):
        zoomtoheight(28.0):
        font("wendy"): settext(title):
        diffuse(style::HELP_TEXT[0], style::HELP_TEXT[1], style::HELP_TEXT[2], 1.0):
        horizalign(left):
        z(15)
    ));

    // Bulleted help lines
    let mut y_top = cy + 60.0;
    let line_px = 20.0;
    let bullet_gap = 10.0;
    for &line in ITEMS[sel_i].help {
        // bullet (middot)
        actors.push(act!(text:
            align(0.0, 0.0):
            xy(rx + 28.0, y_top):
            zoomtoheight(line_px):
            font("miso"): settext("•"):
            diffuse(style::HELP_BULLET[0], style::HELP_BULLET[1], style::HELP_BULLET[2], 1.0):
            horizalign(left):
            z(15)
        ));
        // text
        actors.push(act!(text:
            align(0.0, 0.0):
            xy(rx + 28.0 + bullet_gap, y_top):
            zoomtoheight(line_px):
            font("miso"): settext(line):
            diffuse(style::HELP_TEXT[0], style::HELP_TEXT[1], style::HELP_TEXT[2], 1.0):
            horizalign(left):
            z(15)
        ));
        y_top += line_px + 8.0;
    }

    // tiny scrollbar stub at far right to sell the look
    actors.push(act!(quad:
        align(1.0, 0.5):
        xy(rx + right_w - 10.0, cy + 0.5 * ch):
        zoomto(16.0, ch * 0.75):
        diffuse(style::SCROLL_STUB[0], style::SCROLL_STUB[1], style::SCROLL_STUB[2], 0.9):
        z(12)
    ));

    /* ---------------- footer hint ---------------- */
    actors.push(act!(text:
        align(0.0, 1.0):
        xy(SAFE_MARGIN_X, screen_height() - BOT_BAR_H - 8.0):
        zoomtoheight(HINT_PX):
        font("miso"):
        settext("Up/Down: Move    Left/Right: Change    Enter: OK    Esc: Back"):
        diffuse(style::TEXT_HINT[0], style::TEXT_HINT[1], style::TEXT_HINT[2], 0.95):
        horizalign(left):
        z(100)
    ));

    actors
}

/* ========================================================================== */
/*                               PANEL HELPERS                                */
/* ========================================================================== */

fn build_panel_with_border(
    x: f32, y: f32, w: f32, h: f32,
    bg: [f32; 4],
    edge: [f32; 4],
    edge_px: f32,
    z: i16,
) -> Vec<Actor> {
    let mut v = Vec::with_capacity(2);
    // border
    v.push(act!(quad:
        align(0.0, 0.0): xy(x, y):
        zoomto(w, h):
        diffuse(edge[0], edge[1], edge[2], 0.95):
        z(z)
    ));
    // fill (inset)
    v.push(act!(quad:
        align(0.0, 0.0): xy(x + edge_px, y + edge_px):
        zoomto(w - 2.0 * edge_px, h - 2.0 * edge_px):
        diffuse(bg[0], bg[1], bg[2], 0.96):
        z(z + 1)
    ));
    v
}

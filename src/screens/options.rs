use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::{heart_bg, screen_bar};
use crate::ui::components::screen_bar::{ScreenBarPosition, ScreenBarTitlePlacement};

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* =============================================================================
   Options — rows LEFT, description RIGHT
   + hearts background + top/bottom bars

   Margins (screen pixels, not scaled):
   • LEFT  = 25 px from the left edge to the start of the rows
   • TOP   = 17 px from the content area’s top (just below the top bar) to row #1
   • RIGHT = 17 px from the screen’s right edge to the *right edge* of the description box

   Layout block (unscaled spec; uniformly scaled by `s` to fit between gutters):
   • Rows area width: 721 px, 10 visible rows, each 55 px tall, 3 px vertical gap
   • Separator: 3 px
   • Description: 484 px wide, **577 px tall** (matches 10×55 + 9×3)
============================================================================= */

/// Bars in `screen_bar.rs` use 32.0 px height.
const BAR_H: f32 = 32.0;

/// Screen-space margins (pixels, not scaled)
const LEFT_MARGIN_PX: f32 = 25.0;
const RIGHT_MARGIN_PX: f32 = 17.0;
const FIRST_ROW_TOP_MARGIN_PX: f32 = 17.0;

/// Unscaled spec constants (we’ll uniformly scale).
const VISIBLE_ROWS: usize = 10; // how many rows are shown at once
const ROW_H: f32 = 55.0;
const ROW_GAP: f32 = 3.0;
const LIST_W: f32 = 721.0;

const SEP_W: f32 = 3.0;     // gap/stripe between rows and description
const DESC_W: f32 = 484.0;  // description panel width
// derive description height from visible rows so it never includes a trailing gap
const DESC_H: f32 = (VISIBLE_ROWS as f32) * ROW_H + ((VISIBLE_ROWS - 1) as f32) * ROW_GAP;

/// Text sizing (unscaled). Picked to sit nicely inside 55px rows.
const TEXT_PX: f32 = 26.0;
const TEXT_LEFT_PAD: f32 = 19.0; // padding inside a row before the heart
const HEART_TEXT_GAP: f32 = 17.0;

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
    Item { name: "Network Options",                 help: &["Online features, matchmaking, latency…"] },
    Item { name: "Profiles",                        help: &["Create, select, and edit player profiles."] },
    Item { name: "Theme Options",                   help: &["UI skin, colorway, layout, accessibility."] },
    Item { name: "Data Management",                 help: &["Save data, screenshots, logs, cache."] },
    Item { name: "Service Options",                 help: &["Cabinet/service settings for operators."] },
    Item { name: "Credits",                         help: &["Project contributors and licenses."] },
    Item { name: "Exit",                            help: &["Return to the main menu."] },
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
    let total = ITEMS.len();
    match e.physical_key {
        PhysicalKey::Code(KeyCode::Escape) => ScreenAction::Navigate(Screen::Menu),
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            if state.selected == 0 { state.selected = total.saturating_sub(1); } else { state.selected -= 1; }
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected = if total == 0 { 0 } else { (state.selected + 1) % total };
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            // If the last item ("Exit") is selected, go back to main menu.
            if total > 0 && state.selected == total - 1 {
                ScreenAction::Navigate(Screen::Menu)
            } else {
                // Stub for other items (open sub-screens later).
                ScreenAction::None
            }
        }
        _ => ScreenAction::None,
    }
}

/* --------------------------------- layout -------------------------------- */

/// content rect = full screen minus top & bottom bars.
/// We fit the (rows + separator + description) block inside that content rect,
/// honoring LEFT, RIGHT and TOP margins in *screen pixels*.
/// Returns (scale, origin_x, origin_y).
fn scaled_block_origin_with_margins() -> (f32, f32, f32) {
    let total_w = LIST_W + SEP_W + DESC_W;
    let total_h = DESC_H;

    let sw = screen_width();
    let sh = screen_height();

    // content area (between bars)
    let content_top = BAR_H;
    let content_bottom = sh - BAR_H;
    let content_h = (content_bottom - content_top).max(0.0);

    // available width between fixed left/right gutters
    let avail_w = (sw - LEFT_MARGIN_PX - RIGHT_MARGIN_PX).max(0.0);
    // available height after the fixed top margin (inside content area)
    let avail_h = (content_h - FIRST_ROW_TOP_MARGIN_PX).max(0.0);

    // candidate scales
    let s_w = if total_w > 0.0 { avail_w / total_w } else { 1.0 };
    let s_h = if total_h > 0.0 { avail_h / total_h } else { 1.0 };
    let s = s_w.min(s_h).max(0.0);

    // X origin:
    // Right-align inside [LEFT..(sw-RIGHT)] so the description box ends exactly
    // RIGHT_MARGIN_PX from the screen edge.
    let ox = LEFT_MARGIN_PX + (avail_w - total_w * s).max(0.0);

    // Y origin is fixed under the top bar by the requested margin.
    let oy = content_top + FIRST_ROW_TOP_MARGIN_PX;

    (s, ox, oy)
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

    /* --------------------------- MAIN CONTENT UI -------------------------- */

    // --- global colors ---
    let col_active_bg  = color::rgba_hex("#333333"); // active bg for normal rows

    // inactive bg = #071016 @ 0.8 alpha
    let base_inactive  = color::rgba_hex("#071016");
    let col_inactive_bg: [f32; 4] = [base_inactive[0], base_inactive[1], base_inactive[2], 0.8];

    let col_white      = [1.0, 1.0, 1.0, 1.0];
    let col_black      = [0.0, 0.0, 0.0, 1.0];

    // Simply Love brand color (same index used by the heart bg).
    let col_brand_bg   = color::simply_love_rgba(color::DEFAULT_COLOR_INDEX);

    // Active text color (for normal rows) – keep using a palette color keyed by selection.
    let col_active_text = color::simply_love_rgba(state.selected as i32);

    // --- scale & origin honoring fixed screen-space margins ---
    let (s, list_x, list_y) = scaled_block_origin_with_margins();

    // Geometry (scaled)
    let list_w = LIST_W * s;
    let sep_w  = SEP_W * s;
    let desc_w = DESC_W * s;
    let desc_h = DESC_H * s;

    // Separator immediately to the RIGHT of the rows, aligned to the FIRST row top
    v.push(act!(quad:
        align(0.0, 0.0):
        xy(list_x + list_w, list_y):
        zoomto(sep_w, desc_h):
        diffuse(col_active_bg[0], col_active_bg[1], col_active_bg[2], col_active_bg[3]) // #333333
    ));

    // Description box (RIGHT of separator), aligned to the first row top
    let desc_x = list_x + list_w + sep_w;
    v.push(act!(quad:
        align(0.0, 0.0):
        xy(desc_x, list_y):
        zoomto(desc_w, desc_h):
        diffuse(col_active_bg[0], col_active_bg[1], col_active_bg[2], col_active_bg[3]) // #333333
    ));

    // ---------------------------- Scrolling math ---------------------------
    let total_items = ITEMS.len();
    let anchor_row: usize = 4; // keep cursor near middle (5th visible row)
    let max_offset = total_items.saturating_sub(VISIBLE_ROWS);
    let offset_rows = if total_items <= VISIBLE_ROWS {
        0
    } else {
        state.selected.saturating_sub(anchor_row).min(max_offset)
    };

    // Row loop (backgrounds + content). We render the visible window.
    for i_vis in 0..VISIBLE_ROWS {
        let item_idx = offset_rows + i_vis;
        if item_idx >= total_items { break; }

        let row_y = list_y + (i_vis as f32) * (ROW_H + ROW_GAP) * s;

        let is_active = item_idx == state.selected;
        let is_exit   = item_idx == total_items - 1;

        // Row background width:
        // - Exit: always keep the 3px gap (even when active)
        // - Normal items: inactive keeps gap; active touches the separator
        let row_w = if is_exit {
            list_w - sep_w
        } else if is_active {
            list_w
        } else {
            list_w - sep_w
        };

        // Choose bg color with special case for active Exit row
        let bg = if is_active {
            if is_exit { col_brand_bg } else { col_active_bg }
        } else {
            col_inactive_bg
        };

        v.push(act!(quad:
            align(0.0, 0.0):
            xy(list_x, row_y):
            zoomto(row_w, ROW_H * s):
            diffuse(bg[0], bg[1], bg[2], bg[3])
        ));

        // Content placement inside row
        let row_mid_y = row_y + 0.5 * ROW_H * s;
        let text_h    = TEXT_PX * s;

        // Heart/icon sizing
        let heart_h = text_h;
        let heart_w = heart_h * HEART_ASPECT;

        // Left padding INSIDE the row
        let content_left = list_x + TEXT_LEFT_PAD * s;

        // Heart sprite (skip for Exit)
        if !is_exit {
            let heart_tint = if is_active { col_active_text } else { col_white };
            v.push(act!(sprite("heart.png"):
                align(0.0, 0.5):
                xy(content_left, row_mid_y):
                zoomto(heart_w, heart_h):
                diffuse(heart_tint[0], heart_tint[1], heart_tint[2], heart_tint[3])
            ));
        }

        // Text (Miso)
        let text_x = if is_exit {
            // no heart => start at left pad
            content_left
        } else {
            // heart + gap
            content_left + heart_w + HEART_TEXT_GAP * s
        };

        let label = ITEMS[item_idx].name;

        // Exit text: white when inactive; black when active.
        let color_t = if is_exit {
            if is_active { col_black } else { col_white }
        } else if is_active {
            col_active_text
        } else {
            col_white
        };

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
    let body_px  = 28.0 * s;

    let desc_pad_x = 18.0 * s;
    let mut cursor_y = list_y + 18.0 * s;

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

// src/screens/select_color.rs
use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::{heart_bg, screen_bar};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Decorative palette (safe defaults). Feel free to swap/add colors.
pub const DECORATIVE_HEX: [&str; 12] = [
		"#FF3C23",
		"#FF003C",
		"#C1006F",
		"#8200A1",
		"#413AD0",
		"#0073FF",
		"#00ADC0",
		"#5CE087",
		"#AEFA44",
		"#FFFF00",
		"#FFBE00",
		"#FF7D00"
];

// Native art size of heart.png (for aspect-correct sizing)
const HEART_NATIVE_W: f32 = 668.0;
const HEART_NATIVE_H: f32 = 566.0;
const HEART_ASPECT: f32 = HEART_NATIVE_W / HEART_NATIVE_H;

// Wheel tuning
const SCROLL_SPEED_SLOTS_PER_SEC: f32 = 10.0; // how fast the wheel slides
const ROT_PER_SLOT_DEG: f32 = 10.0;           // inward tilt amount (± per slot)
const ZOOM_CENTER: f32 = 1.25;                // center heart size
const EDGE_MIN_RATIO: f32 = 0.20;        // edge zoom = ZOOM_CENTER * EDGE_MIN_RATIO
const WHEEL_Z_BASE: i16 = 105;                // above BG, below bars

#[inline(always)]
fn is_wide() -> bool {
    screen_width() / screen_height() >= 1.6 // ~16:10/16:9 and wider
}

#[inline(always)]
pub fn palette_rgba(idx: i32) -> [f32; 4] {
    let n = DECORATIVE_HEX.len() as i32;
    color::rgba_hex(DECORATIVE_HEX[idx.rem_euclid(n) as usize])
}

pub struct State {
    /// Which color in DECORATIVE_HEX is focused (and previewed in the bg)
    pub active_color_index: i32,
    /// Smooth wheel offset (in “slots”); eased toward active_color_index
    pub scroll: f32,
    bg: heart_bg::State,
}

pub fn init() -> State {
    State {
        active_color_index: 0,
        scroll: 0.0, // keep in sync with active_color_index
        bg: heart_bg::State::new(),
    }
}

pub fn handle_key_press(state: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state != ElementState::Pressed {
        return ScreenAction::None;
    }

    match e.physical_key {
        PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
            state.active_color_index += 1;
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
            state.active_color_index -= 1;
            ScreenAction::None
        }
        PhysicalKey::Code(KeyCode::Enter) => ScreenAction::Navigate(Screen::Gameplay),
        PhysicalKey::Code(KeyCode::Escape) => ScreenAction::Navigate(Screen::Menu),
        _ => ScreenAction::None,
    }
}

pub fn get_actors(state: &State, _: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(64);

    // 1) Animated heart background, tinted by the currently focused color
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
    }));

    // 2) Bars (top + bottom)
    const FG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "SELECT COLOR",
        position: screen_bar::ScreenBarPosition::Top,
        transparent: false,
        left_text: None,
        right_text: None,
        fg_color: FG,
    }));
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "EVENT MODE",
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: false,
        left_text: None,
        right_text: Some("NOT PRESENT"),
        fg_color: FG,
    }));

    // 3) The bow of hearts (wheel) — smooth + inward tilt, no refade
    let wide = is_wide();
    let num_slots: i32 = if wide { 11 } else { 7 };
    let center_slot: i32 = num_slots / 2;
    let w = screen_width();

    // symmetric spacing so the visual center is true center
    let x_spacing = w / (num_slots as f32 - 1.0);

    // split scroll into integer + fractional parts (stable left/right motion)
    let base_i = state.scroll.floor() as i32;
    let frac = state.scroll - base_i as f32; // [0, 1)

    for slot in 0..num_slots {
        let offset_i = slot - center_slot; // integer slot offset

        // fractional offset used for position/zoom/rotation (smooth slide)
        let o = offset_i as f32 - frac;

        // palette color for this slot (stick to integer to avoid “color lerp” look)
        let tint = palette_rgba(base_i + offset_i);

        // X centered, Y forms a gentle bow
        let x = screen_center_x() + o * x_spacing;
        let y = 12.0 * o * o - 20.0;

        // inward tilt: left leans right, right leans left
        let rot_deg = -o * ROT_PER_SLOT_DEG;

        // Geometric falloff against the farthest **visible** slot
        // (we hide slot 0 and last). That makes the outermost *visible*
        // hearts hit exactly ZOOM_CENTER * EDGE_MIN_RATIO.
        let max_off_all = 0.5 * (num_slots as f32 - 1.0); // theoretical edges
        let max_off_visible = (max_off_all - 1.0).max(1.0); // one in from edges
        let r = EDGE_MIN_RATIO.powf(1.0 / max_off_visible); // per-slot factor
        let zoom = ZOOM_CENTER * r.powf(o.abs().min(max_off_visible));

        // depth so near-center draws on top
        let z_layer = WHEEL_Z_BASE - (o.abs().round() as i16);

        // correct aspect (don’t stretch tall)
        let base_h = 168.0; // overall heart height (tweak)
        let base_w = base_h * HEART_ASPECT;

        // hide the very first/last as subtle padding (no animated fade)
        let alpha = if slot == 0 || slot == num_slots - 1 { 0.0 } else { 1.0 };

        actors.push(act!(sprite("heart.png"):
            align(0.5, 0.5):
            xy(x, screen_center_y() + y):
            rotationz(rot_deg):
            z(z_layer):
            zoomto(base_w, base_h):
            zoom(zoom):
            diffuse(tint[0], tint[1], tint[2], alpha)
        ));
    }

    actors
}

pub fn update(state: &mut State, dt: f32) {
    // glide scroll toward the selected slot
    let target = state.active_color_index as f32;
    let delta  = target - state.scroll;

    let max_step = SCROLL_SPEED_SLOTS_PER_SEC * dt;
    if delta.abs() <= max_step {
        state.scroll = target;                 // snap when close
    } else {
        state.scroll += delta.signum() * max_step;
    }
}
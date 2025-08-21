use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::act;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use rand::prelude::*;

// new: import the SCREEN_*() getters
use crate::core::space::globals::*;

// new: time + TAU for clean sine waves
use std::time::Instant;
use std::f32::consts::TAU;

const HEART_COLORS: [&str; 12] = [
    "#FF5D47", "#FF577E", "#FF47B3", "#DD57FF", "#8885ff", "#3D94FF",
    "#00B8CC", "#5CE087", "#AEFA44", "#FFFF00", "#FFBE00", "#FF7D00",
];
const NUM_HEARTS: usize = 75;
const HEART_SIZE: f32 = 48.0;

// how much hearts bob (in pixels) and how fast corners rotate
const BOB_AMPLITUDE: f32 = 8.0;
const CORNER_ROT_SPEED: f32 = 0.9;
const CORNER_ROT_AMPL: f32 = 30.0;

struct Heart {
    pos: [f32; 2],
    color: [f32; 4],
    cell: (u32, u32),
    // new: per-heart phase & speed for subtle, non-uniform bobbing
    phase: f32,
    speed: f32,
}

pub struct State {
    hearts: Vec<Heart>,
    // new: start time for animation
    t0: Instant,
}

pub fn init() -> State {
    let mut rng = rand::rng();

    // Parse the palette once; reuse sampled entries.
    let palette: Vec<[f32; 4]> = HEART_COLORS
        .iter()
        .map(|&hex| color::rgba_hex(hex))
        .collect();

    let hearts: Vec<Heart> = (0..NUM_HEARTS)
        .map(|_| {
            let tint = {
                let idx = rng.random_range(0..palette.len());
                palette[idx]
            };
            Heart {
                pos: [
                    rng.random_range(-400.0..400.0),
                    rng.random_range(-200.0..200.0),
                ],
                color: tint,
                cell: (rng.random_range(0..4), rng.random_range(0..4)),
                phase: rng.random_range(0.0..TAU),
                speed: rng.random_range(0.5..1.2),
            }
        })
        .collect();

    State {
        hearts,
        t0: Instant::now(), // new
    }
}

pub fn handle_key_press(_: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) = e.physical_key {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

// keep the Metrics arg in the signature (unused), so call sites don't need to change yet
pub fn get_actors(state: &State, _: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(NUM_HEARTS + 6);

    let w  = screen_width();      // ← was sm::width(m)
    let h  = screen_height();     // ← was sm::height(m)
    let cx = screen_center_x();   // ← was sm::center(m).0
    let cy = screen_center_y();   // ← was sm::center(m).1

    // new: seconds since init (used for animation)
    let t = state.t0.elapsed().as_secs_f32();

    // Hearts: stored as offsets around center; convert to SM xy in parent TL space.
    // new: add a tiny vertical sine bob per heart.
    actors.extend(state.hearts.iter().map(|h| {
        let y_bob = (t * h.speed + h.phase).sin() * BOB_AMPLITUDE;
        act!(sprite("hearts_4x4.png"):
            align(0.5, 0.5):
            xy(cx + h.pos[0], cy + h.pos[1] + y_bob):
            zoomto(HEART_SIZE, HEART_SIZE):
            cell(h.cell.0, h.cell.1):
            diffuse(h.color[0], h.color[1], h.color[2], h.color[3])
        )
    }));

    actors.push(crate::ui::components::top_bar::build("OPTIONS"));

    // Corners: compute positions in TL space using fractions (0, .5, 1) of the screen
    let corners = [
        ((0.0_f32, 0.0_f32), [ 12.0,  12.0], [1.0, 0.9, 0.2, 1.0]), // TopLeft
        ((1.0_f32, 0.0_f32), [-12.0,  12.0], [0.2, 1.0, 0.6, 1.0]), // TopRight
        ((0.0_f32, 1.0_f32), [ 12.0, -12.0], [0.6, 0.6, 1.0, 1.0]), // BottomLeft
        ((1.0_f32, 1.0_f32), [-12.0, -12.0], [1.0, 0.6, 0.2, 1.0]), // BottomRight
    ];

    // new: give each corner a slow, phase-shifted wobble so you can confirm animation is live
    for (i, ((hx, vy), off, col)) in corners.into_iter().enumerate() {
        let (x, y) = (hx * w + off[0], vy * h + off[1]);
        let rot = CORNER_ROT_AMPL * ((t * CORNER_ROT_SPEED) + i as f32 * 0.7).sin();
        actors.push(act!(quad:
            align(hx, vy):      // pivot at the corner
            xy(x, y):           // absolute SM xy in parent TL space
            zoomto(10.0, 10.0):
            diffuse(col[0], col[1], col[2], col[3]):
            rotationz(rot)
        ));
    }

    actors.push(act!(text:
        align(0.5, 1.0):                    // pivot bottom-center
        xy(0.5 * w, h - 100.0):             // SM xy in TL space
        px(60.0):
        font("miso"):
        diffuse(0.8, 0.9, 0.7, 1.0):
        text("This is miso font!"):
        talign(center)
    ));

    actors
}

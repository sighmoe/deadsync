// src/screens/options.rs
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, Anchor, TextAlign};
use crate::ui::{color};
use crate::act;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use rand::prelude::*;

const HEART_COLORS: [&str; 12] = [
    "#FF5D47", "#FF577E", "#FF47B3", "#DD57FF", "#8885ff", "#3D94FF",
    "#00B8CC", "#5CE087", "#AEFA44", "#FFFF00", "#FFBE00", "#FF7D00",
];
const NUM_HEARTS: usize = 75;
const HEART_SIZE: f32 = 48.0;

struct Heart {
    pos: [f32; 2],
    color: [f32; 4],
    cell: (u32, u32),
}

pub struct State {
    hearts: Vec<Heart>,
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
            }
        })
        .collect();

    State { hearts }
}

pub fn handle_key_press(_: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) = e.physical_key {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(NUM_HEARTS + 6);

    actors.extend(state.hearts.iter().map(|h| {
        act!(sprite("hearts_4x4.png"):
            align(0.5, 0.5):
            xy(h.pos[0], h.pos[1]):
            zoomto(HEART_SIZE, HEART_SIZE):
            cell(h.cell.0, h.cell.1):
            diffuse(h.color[0], h.color[1], h.color[2], h.color[3])
        )
    }));

    actors.push(crate::ui::components::top_bar::build("OPTIONS"));

    let corners = [
        ((0.0_f32, 0.0_f32), [ 12.0,  12.0], [1.0, 0.9, 0.2, 1.0]), // TopLeft
        ((1.0_f32, 0.0_f32), [-12.0,  12.0], [0.2, 1.0, 0.6, 1.0]), // TopRight
        ((0.0_f32, 1.0_f32), [ 12.0, -12.0], [0.6, 0.6, 1.0, 1.0]), // BottomLeft
        ((1.0_f32, 1.0_f32), [-12.0, -12.0], [1.0, 0.6, 0.2, 1.0]), // BottomRight
    ];
    actors.extend(corners.into_iter().map(|((hx, vy), off, col)| {
        act!(quad:
            align(hx, vy):
            xy(off[0], off[1]):
            zoomto(10.0, 10.0):
            diffuse(col[0], col[1], col[2], col[3])
        )
    }));

    actors.push(Actor::Text {
        anchor:  Anchor::BottomCenter,
        offset:  [0.0, -100.0],
        align:   TextAlign::Center,
        px:      60.0,
        font:    "miso",
        color:   [0.8, 0.9, 0.7, 1.0],
        content: "This is miso font!".to_string(),
    });

    actors
}

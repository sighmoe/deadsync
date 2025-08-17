// src/screens/options.rs
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, Anchor, SizeSpec, TextAlign};
use crate::ui::{color, components};
use crate::sprite;
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
    let hearts = (0..NUM_HEARTS).map(|_| {
        let color_hex = HEART_COLORS[rng.random_range(0..HEART_COLORS.len())];
        Heart {
            pos: [rng.random_range(-400.0..400.0), rng.random_range(-200.0..200.0)],
            color: color::rgba_hex(color_hex),
            cell: (rng.random_range(0..4), rng.random_range(0..4)),
        }
    }).collect();

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
    let mut actors = Vec::new();

    // Hearts (grid inferred from filename; only cell+tint specified)
    for heart in &state.hearts {
        actors.push(sprite! {
            anchor: Anchor::Center,
            offset: heart.pos,
            size:   [SizeSpec::Px(HEART_SIZE), SizeSpec::Px(HEART_SIZE)],
            texture:"hearts_4x4.png",
            tint:   heart.color,
            cell:   heart.cell,
            // grid: Some((4,4)),       // optional explicit override
            // uv_rect: Some([..]),     // optional explicit UV override
        });
    }

    // Reusable top bar
    actors.push(components::top_bar::build("OPTIONS"));

    // Corner markers (unchanged)
    actors.push(Actor::Quad { anchor: Anchor::TopLeft,     offset: [ 12.0,  12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [1.0,0.9,0.2,1.0]});
    actors.push(Actor::Quad { anchor: Anchor::TopRight,    offset: [-12.0,  12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [0.2,1.0,0.6,1.0]});
    actors.push(Actor::Quad { anchor: Anchor::BottomLeft,  offset: [ 12.0, -12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [0.6,0.6,1.0,1.0]});
    actors.push(Actor::Quad { anchor: Anchor::BottomRight, offset: [-12.0, -12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [1.0,0.6,0.2,1.0]});

    // Text sample
    actors.push(Actor::Text {
        anchor: Anchor::BottomCenter,
        offset: [0.0, -100.0],
        align: TextAlign::Center,
        px: 60.0,
        font: "miso",
        color: [0.8, 0.9, 0.7, 1.0],
        content: "This is miso font!".to_string(),
    });

    actors
}

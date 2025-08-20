// src/ui/components/top_bar.rs
use crate::ui::actors::{Actor, Anchor, SizeSpec, Background, TextAlign};
use crate::act;

/// A full-width bar anchored to the top of the screen, with a centered title.
pub fn build(title: &'static str) -> Actor {
    const BAR_H: f32 = 50.0;
    const TITLE_PX: f32 = 40.0;
    const BG_COLOR: [f32; 4] = [0.15, 0.15, 0.18, 1.0];
    const FG_COLOR: [f32; 4] = [0.90, 0.90, 1.00, 1.0];

    Actor::Frame {
        anchor: Anchor::TopLeft,
        offset: [0.0, 0.0],
        size:   [SizeSpec::Fill, SizeSpec::Px(BAR_H)],
        children: vec![
            act!(text:
                align(0.5, 0.5): xy(0.0, 0.0): px(TITLE_PX):
                diffuse(FG_COLOR[0], FG_COLOR[1], FG_COLOR[2], FG_COLOR[3]):
                font("wendy"): text(title): talign(center)
            )
        ],
        background: Some(Background::Color(BG_COLOR)),
        z: 0i16,
    }
}

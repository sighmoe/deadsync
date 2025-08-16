// src/ui/components/top_bar.rs
use crate::ui::actors::{Actor, Anchor, SizeSpec, Background, TextAlign};

/// A full-width bar anchored to the top of the screen, with a centered title.
pub fn build(title: &'static str) -> Actor {
    // --- Style constants ---
    const BAR_H: f32 = 50.0;
    const TITLE_PX: f32 = 40.0;
    const BG_COLOR: [f32; 4] = [0.15, 0.15, 0.18, 1.0];
    const FG_COLOR: [f32; 4] = [0.90, 0.90, 1.00, 1.0];

    Actor::Frame {
        anchor: Anchor::TopLeft,
        offset: [0.0, 0.0],
        size:   [SizeSpec::Fill, SizeSpec::Px(BAR_H)],
        children: vec![
            Actor::Text {
                anchor:  Anchor::Center,
                offset:  [0.0, 0.0],
                px:      TITLE_PX,
                color:   FG_COLOR,
                font:    "wendy",
                content: title.to_string(),
                align:   TextAlign::Center,
            }
        ],
        background: Some(Background::Color(BG_COLOR)),
    }

}
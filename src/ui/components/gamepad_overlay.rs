use crate::act;
use crate::core::space::*;
use crate::ui::actors::Actor;
use crate::ui::color;

// --- Constants to match StepMania's SystemMessage display ---
const BAR_H: f32 = 30.0;
const FADE_IN_DURATION: f32 = 0.0; // SM appears instantly
const HOLD_DURATION: f32 = 3.33;
const FADE_OUT_DURATION: f32 = 0.25;

const TEXT_MARGIN_X: f32 = 10.0;
const TEXT_MARGIN_Y: f32 = 10.0; // from top of bar
const TEXT_ZOOM: f32 = 0.8;

pub struct Params<'a> {
    pub message: &'a str,
}

/// Builds the actors for a temporary system message overlay at the top of the screen.
/// The actors manage their own lifecycle (fade-in, hold, fade-out) via tweens.
pub fn build(params: Params) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(2);

    let bg_color = color::rgba_hex("#000000");

    let bg = act!(quad:
        align(0.5, 0.0):
        xy(screen_center_x(), 0.0):
        zoomto(screen_width(), BAR_H):
        diffuse(bg_color[0], bg_color[1], bg_color[2], 0.0):
        z(1000): // High Z-order to be on top of other UI

        // Animation sequence
        linear(FADE_IN_DURATION): alpha(0.85):
        sleep(HOLD_DURATION):
        linear(FADE_OUT_DURATION): alpha(0.0)
    );

    let text = act!(text:
        font("miso"):
        settext(params.message):
        align(0.0, 0.0): // top-left
        xy(TEXT_MARGIN_X, TEXT_MARGIN_Y):
        zoom(TEXT_ZOOM):
        diffusealpha(0.0):
        z(1001): // Above the background quad

        // Animation sequence, synced with the background
        linear(FADE_IN_DURATION): alpha(1.0):
        sleep(HOLD_DURATION):
        linear(FADE_OUT_DURATION): alpha(0.0)
    );

    actors.push(bg);
    actors.push(text);
    actors
}

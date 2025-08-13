use crate::ui::components::options_swatches;
use crate::ui::primitives::{Sprite, UIElement, Quad};
use crate::screens::{Screen, ScreenAction};
use crate::core::space::{design_width_16_9, Metrics};
use crate::ui::build::{
    from_right, from_top, sm_rect_to_center_size,
    from_left, from_bottom, sm_point_to_world,
    screen_left, screen_right, screen_bottom, screen_top,
    wide_scale,
};
use cgmath::Vector2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};


pub struct State;

pub fn init() -> State {
    State
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
        if event.state == ElementState::Pressed {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_ui_elements(_state: &State, m: &Metrics) -> Vec<UIElement> {
    let mut elements = Vec::new();

    /* --- 1) Banner in the top-right (from_right/from_top/sm_rect_to_center_size) --- */
    let bw = 256.0;
    let bh = 64.0;
    let x_tl = from_right(bw + 20.0, m);
    let y_tl = from_top(20.0, m);
    let (center, size) = sm_rect_to_center_size(x_tl, y_tl, bw, bh, m);

    elements.push(UIElement::Sprite(Sprite {
        center: Vector2::new(center[0], center[1]),
        size:   Vector2::new(size[0],   size[1]),
        texture_id: "fallback_banner.png",
    }));

    /* --- 2) Bottom status bar spanning full width (screen_left/right, from_bottom, wide_scale) --- */
    let margin = 12.0;
    let bar_h  = wide_scale(10.0, 24.0, m); // lerp 4:3 -> 16:9
    let bar_w  = screen_right(m) - screen_left(m);
    let bar_x  = from_left(0.0, m);
    let bar_y  = from_bottom(margin, m) - bar_h; // SM top-left y of bottom-anchored rect
    let (bar_c, bar_s) = sm_rect_to_center_size(bar_x, bar_y, bar_w, bar_h, m);

    elements.push(UIElement::Quad(Quad {
        center: Vector2::new(bar_c[0], bar_c[1]),
        size:   Vector2::new(bar_s[0], bar_s[1]),
        color:  [0.15, 0.15, 0.18, 1.0],
    }));

    /* --- 3) Thin edge guides (use screen_top/screen_bottom) --- */
    let top_h = 4.0;
    let (top_c, top_s) = sm_rect_to_center_size(0.0, screen_top(m), screen_right(m), top_h, m);
    elements.push(UIElement::Quad(Quad {
        center: Vector2::new(top_c[0], top_c[1]),
        size:   Vector2::new(top_s[0], top_s[1]),
        color:  [0.35, 0.35, 0.38, 1.0],
    }));

    let bot_h = 4.0;
    let bot_y = screen_bottom(m) - bot_h;
    let (bot_c, bot_s) = sm_rect_to_center_size(0.0, bot_y, screen_right(m), bot_h, m);
    elements.push(UIElement::Quad(Quad {
        center: Vector2::new(bot_c[0], bot_c[1]),
        size:   Vector2::new(bot_s[0], bot_s[1]),
        color:  [0.35, 0.35, 0.38, 1.0],
    }));

    /* --- 4) Tiny marker at top-left in SM coords (sm_point_to_world, from_left/from_top) --- */
    let tl_sm_x = from_left(12.0, m);
    let tl_sm_y = from_top(12.0, m);
    let [tl_wx, tl_wy] = sm_point_to_world(tl_sm_x, tl_sm_y, m);
    elements.push(UIElement::Quad(Quad {
        center: Vector2::new(tl_wx, tl_wy),
        size:   Vector2::new(10.0, 10.0),
        color:  [1.0, 0.9, 0.2, 1.0],
    }));

    /* --- 5) Tiny marker at bottom-left (from_left/from_bottom via rect helper) --- */
    let (bl_c, bl_s) = sm_rect_to_center_size(from_left(12.0, m), from_bottom(12.0, m) - 10.0, 10.0, 10.0, m);
    elements.push(UIElement::Quad(Quad {
        center: Vector2::new(bl_c[0], bl_c[1]),
        size:   Vector2::new(bl_s[0], bl_s[1]),
        color:  [0.2, 1.0, 0.6, 1.0],
    }));

    /* --- 6) Center marker computed from world bounds (no cx/cy in Metrics anymore) --- */
    let cx = 0.5 * (m.left + m.right);
    let cy = 0.5 * (m.top  + m.bottom);
    let alpha = {
        let a = screen_right(m) / design_width_16_9(); // 0..~1
        if a < 0.2 { 0.2 } else if a > 1.0 { 1.0 } else { a }
    };
    let center_size = 14.0 + 0.01 * screen_bottom(m);

    elements.push(UIElement::Quad(Quad {
        center: Vector2::new(cx, cy),
        size:   Vector2::new(center_size, center_size),
        color:  [0.2, 0.8, 1.0, alpha],
    }));

    /* --- 7) The three colored swatches --- */
    elements.extend(options_swatches());

    elements
}

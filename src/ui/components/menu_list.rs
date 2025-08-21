// src/ui/components/menu_list.rs
use crate::ui::actors::Actor;
use crate::act;

#[derive(Clone, Copy)]
pub struct MenuParams<'a> {
    pub options: &'a [&'a str],
    pub selected_index: usize,

    // In SM TL space:
    pub start_center_y: f32,
    pub row_spacing: f32,

    // Typography + colors
    pub selected_px: f32,
    pub normal_px: f32,
    pub selected_color: [f32; 4],
    pub normal_color: [f32; 4],
    pub font: &'static str,

    // NEW: needed for SM-style xy (parent TL space)
    pub screen_width: f32,
}

/// Build a vertical, center-aligned menu whose visual center stays fixed.
pub fn build_vertical_menu(p: MenuParams) -> Vec<Actor> {
    let mut out = Vec::with_capacity(p.options.len());
    let center_x = 0.5 * p.screen_width;

    for (i, label) in p.options.iter().enumerate() {
        let selected = i == p.selected_index;
        let px      = if selected { p.selected_px } else { p.normal_px };
        let color   = if selected { p.selected_color } else { p.normal_color };

        let center_y = p.start_center_y + (i as f32) * p.row_spacing;
        let y_top    = center_y - 0.5 * px;

        out.push(act!(text:
            align(0.5, 0.0):    // pivot top-center (within actor)
            xy(center_x, y_top): // SM xy: absolute in parent TL space
            px(px):
            diffuse(color[0], color[1], color[2], color[3]):
            font(p.font):
            text(*label):
            talign(center)
        ));
    }
    out
}

// src/ui/components/menu_list.rs
use crate::ui::actors::{Actor, Anchor, TextAlign};

#[derive(Clone, Copy)]
pub struct MenuParams<'a> {
    pub options: &'a [&'a str],
    pub selected_index: usize,

    // Layout (top-left "SM px" space):
    // Use center-based layout so scaling doesn't shift position.
    pub start_center_y: f32,
    pub row_spacing: f32,

    // Typography + colors
    pub selected_px: f32,
    pub normal_px: f32,
    pub selected_color: [f32; 4],
    pub normal_color: [f32; 4],
    pub font: &'static str,
}

/// Build a vertical, center-aligned menu where the *visual center* of each row
/// stays fixed as size changes (selected row “zooms” from the middle).
pub fn build_vertical_menu(p: MenuParams) -> Vec<Actor> {
    let mut out = Vec::with_capacity(p.options.len());
    for (i, label) in p.options.iter().enumerate() {
        let selected = i == p.selected_index;
        let px      = if selected { p.selected_px } else { p.normal_px };
        let color   = if selected { p.selected_color } else { p.normal_color };

        // Keep row center fixed; convert to TopCenter offset for Text:
        let center_y = p.start_center_y + (i as f32) * p.row_spacing;
        let y_top    = center_y - 0.5 * px;

        out.push(Actor::Text {
            anchor:  Anchor::TopCenter,   // x centered, y is top
            offset:  [0.0, y_top],        // compute top from desired center
            px,
            color,
            font:    p.font,
            content: (*label).to_string(),
            align:   TextAlign::Center,
        });
    }
    out
}

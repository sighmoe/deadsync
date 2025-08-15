// src/ui/actors.rs
use crate::core::space::Metrics;
use crate::ui::msdf;
use crate::ui::primitives::{Quad as UiQuad, Sprite as UiSprite, Text as UiText, UIElement};
use cgmath::Vector2;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Background {
    Color([f32; 4]),
    Texture(&'static str),
}

#[derive(Clone, Copy, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
}

#[derive(Clone, Copy, Debug)]
pub enum SizeSpec {
    Px { w: f32, h: f32 },
    Fill, // take parent rect w/h
}

#[derive(Clone, Debug)]
pub enum Actor {
    Quad { anchor: Anchor, offset: [f32; 2], size: SizeSpec, color: [f32; 4] },
    Sprite { anchor: Anchor, offset: [f32; 2], size: SizeSpec, texture: &'static str },
    Text {
        anchor: Anchor,
        offset: [f32; 2],
        size: SizeSpec,
        px: f32,
        color: [f32; 4],
        font: &'static str,
        content: String,
        align: TextAlign,
    },
    Frame {
        anchor: Anchor,
        offset: [f32; 2],
        size: SizeSpec,
        children: Vec<Actor>,
        background: Option<Background>,
    },
}

/* -------------------- BUILD (actors -> UI elements) -------------------- */

#[derive(Clone, Copy)]
struct SmRect { x: f32, y: f32, w: f32, h: f32 } // top-left "SM px" space

#[inline(always)]
fn screen_w(m: &Metrics) -> f32 { m.right - m.left }
#[inline(always)]
fn screen_h(m: &Metrics) -> f32 { m.top - m.bottom }

#[inline(always)]
fn root_rect(m: &Metrics) -> SmRect {
    SmRect { x: 0.0, y: 0.0, w: screen_w(m), h: screen_h(m) }
}

#[inline(always)]
fn resolve_size(spec: SizeSpec, parent: SmRect) -> (f32, f32) {
    match spec {
        SizeSpec::Px { w, h } => (w, h),
        SizeSpec::Fill => (parent.w, parent.h),
    }
}

#[inline(always)]
fn anchor_ref(parent: SmRect, anchor: Anchor) -> (f32, f32) {
    let (fx, fy) = anchor_factors(anchor);
    (parent.x + fx * parent.w, parent.y + fy * parent.h)
}

/// Single canonical mapping from `Anchor` to alignment factors.
/// (0.0 = start, 0.5 = center, 1.0 = end), for both axes.
#[inline(always)]
const fn anchor_factors(anchor: Anchor) -> (f32, f32) {
    match anchor {
        Anchor::TopLeft      => (0.0, 0.0),
        Anchor::TopCenter    => (0.5, 0.0),
        Anchor::TopRight     => (1.0, 0.0),
        Anchor::CenterLeft   => (0.0, 0.5),
        Anchor::Center       => (0.5, 0.5),
        Anchor::CenterRight  => (1.0, 0.5),
        Anchor::BottomLeft   => (0.0, 1.0),
        Anchor::BottomCenter => (0.5, 1.0),
        Anchor::BottomRight  => (1.0, 1.0),
    }
}

#[inline(always)]
fn place_rect(parent: SmRect, anchor: Anchor, offset: [f32; 2], size: SizeSpec) -> SmRect {
    let (w, h) = resolve_size(size, parent);
    let (rx, ry) = anchor_ref(parent, anchor);
    let (ax, ay) = anchor_factors(anchor);

    SmRect {
        x: rx + offset[0] - ax * w,
        y: ry + offset[1] - ay * h,
        w,
        h,
    }
}

// Convert SM rect to world center/size.
#[inline(always)]
fn sm_rect_to_world(rect: SmRect, m: &Metrics) -> (Vector2<f32>, Vector2<f32>) {
    let cx = m.left + rect.x + 0.5 * rect.w;
    let cy = m.top - (rect.y + 0.5 * rect.h);
    (Vector2::new(cx, cy), Vector2::new(rect.w, rect.h))
}

#[inline(always)]
fn place_text_baseline(
    parent: SmRect,
    anchor: Anchor,
    offset: [f32; 2],
    align: TextAlign,
    measured_width: f32,
    font: &msdf::Font,
    content: &str,
    pixel_height: f32,
    m: &Metrics,
) -> Vector2<f32> {
    let (rx, ry) = anchor_ref(parent, anchor);

    // Horizontal alignment: Left / Center / Right
    let align_offset = match align {
        TextAlign::Left   => 0.0,
        TextAlign::Center => -0.5 * measured_width,
        TextAlign::Right  => -measured_width,
    };

    // Exact ascender/descender for THIS line (no heuristics)
    let (asc, desc) = line_extents_px(font, content, pixel_height);
    let line_h_px = asc + desc;

    // Convert anchor edge/center back to a baseline y in SM top-left space.
    let baseline_sm_y = match anchor {
        // Top edge + ascender moves us down to baseline
        Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => ry + offset[1] + asc,

        // Center line + (asc - half line height) moves to baseline
        Anchor::CenterLeft | Anchor::Center | Anchor::CenterRight => ry + offset[1] + asc - 0.5 * line_h_px,

        // Bottom edge - descender moves up to baseline
        Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => ry + offset[1] - desc,
    };

    let left_sm_x = rx + offset[0] + align_offset;

    let world_x = m.left + left_sm_x;
    let world_y = m.top  - baseline_sm_y;
    Vector2::new(world_x, world_y)
}

#[inline(always)]
fn line_extents_px(font: &msdf::Font, text: &str, pixel_height: f32) -> (f32, f32) {
    if pixel_height <= 0.0 || font.line_h == 0.0 || text.is_empty() {
        return (0.0, 0.0);
    }
    let s = pixel_height / font.line_h;

    // Track minimum "top" (can be negative = above baseline) and maximum "bottom" (positive = below).
    let mut any = false;
    let mut min_top = 0.0_f32;
    let mut max_bottom = 0.0_f32;

    for ch in text.chars() {
        if ch == '\n' { continue; }
        if let Some(g) = font.glyphs.get(&ch) {
            // In your loader: yoff = -pb.top (down-positive). plane_h >= 0.
            let top_rel_down     = g.yoff * s;                 // distance from baseline to glyph top (down-positive)
            let bottom_rel_down  = g.yoff * s + g.plane_h * s; // baseline to bottom (down-positive)
            if !any {
                min_top = top_rel_down;
                max_bottom = bottom_rel_down;
                any = true;
            } else {
                if top_rel_down < min_top { min_top = top_rel_down; }
                if bottom_rel_down > max_bottom { max_bottom = bottom_rel_down; }
            }
        }
    }

    if !any {
        return (0.0, 0.0);
    }

    // Ascender: distance ABOVE baseline (positive), so negate the most-negative top.
    // Descender: distance BELOW baseline (positive).
    let asc_px  = (-min_top).max(0.0);
    let desc_px =  max_bottom.max(0.0);
    (asc_px, desc_px)
}

pub fn build_actors(
    actors: &[Actor],
    m: &Metrics,
    fonts: &HashMap<&'static str, msdf::Font>,
) -> Vec<UIElement> {
    let root = root_rect(m);
    actors
        .iter()
        .flat_map(|actor| build_actor_recursive(actor, root, m, fonts))
        .collect()
}

fn build_actor_recursive(
    actor: &Actor,
    parent: SmRect,
    m: &Metrics,
    fonts: &HashMap<&'static str, msdf::Font>,
) -> Vec<UIElement> {
    match actor {
        Actor::Quad { anchor, offset, size, color } => {
            let rect = place_rect(parent, *anchor, *offset, *size);
            let (center, size) = sm_rect_to_world(rect, m);
            vec![UIElement::Quad(UiQuad { center, size, color: *color })]
        }
        Actor::Sprite { anchor, offset, size, texture } => {
            let rect = place_rect(parent, *anchor, *offset, *size);
            let (center, size) = sm_rect_to_world(rect, m);
            vec![UIElement::Sprite(UiSprite { center, size, texture_id: *texture })]
        }
        Actor::Text { anchor, offset, size: _, px, color, font, content, align } => {
            if let Some(font_metrics) = fonts.get(font) {
                let measured_width = font_metrics.measure_line_width(content, *px);
                let origin = place_text_baseline(
                    parent,
                    *anchor,
                    *offset,
                    *align,
                    measured_width,
                    font_metrics,
                    content,
                    *px,
                    m,
                );

                vec![UIElement::Text(UiText {
                    origin,
                    pixel_height: *px,
                    color: *color,
                    font_id: *font,
                    content: content.clone(),
                })]
            } else {
                vec![]
            }
        }
        Actor::Frame { anchor, offset, size, children, background } => {
            let rect = place_rect(parent, *anchor, *offset, *size);
            let mut elements = Vec::new();

            if let Some(bg) = background {
                let (center, size) = sm_rect_to_world(rect, m);
                match bg {
                    Background::Color(color) => {
                        elements.push(UIElement::Quad(UiQuad { center, size, color: *color }));
                    }
                    Background::Texture(texture_id) => {
                        elements.push(UIElement::Sprite(UiSprite { center, size, texture_id }));
                    }
                }
            }

            elements.extend(
                children
                    .iter()
                    .flat_map(|child| build_actor_recursive(child, rect, m, fonts)),
            );
            elements
        }
    }
}

/* -------------------- DSL MACROS -------------------- */

#[macro_export]
macro_rules! quad {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor = $crate::ui::actors::Anchor::TopLeft;
        let mut offset = [0.0_f32, 0.0_f32];
        let mut size   = $crate::ui::actors::SizeSpec::Px { w: 0.0, h: 0.0 };
        let mut color  = [1.0_f32, 1.0, 1.0, 1.0];

        $(
            $crate::__assign_quad_kv!([anchor, offset, size, color] $k : $v);
        )*

        $crate::ui::actors::Actor::Quad { anchor, offset, size, color }
    }};
}

#[macro_export]
macro_rules! sprite {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor  = $crate::ui::actors::Anchor::TopLeft;
        let mut offset  = [0.0_f32, 0.0_f32];
        let mut size    = $crate::ui::actors::SizeSpec::Px { w: 0.0, h: 0.0 };
        let mut texture = "";

        $(
            $crate::__assign_sprite_kv!([anchor, offset, size, texture] $k : $v);
        )*

        $crate::ui::actors::Actor::Sprite { anchor, offset, size, texture }
    }};
}

#[macro_export]
macro_rules! text {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor  = $crate::ui::actors::Anchor::TopLeft;
        let mut offset  = [0.0_f32, 0.0_f32];
        let mut size    = $crate::ui::actors::SizeSpec::Px { w: 0.0, h: 0.0 };
        let mut px      = 32.0_f32;
        let mut color   = [1.0_f32, 1.0, 1.0, 1.0];
        let mut font    = "wendy";
        let mut content = String::new();
        let mut align   = $crate::ui::actors::TextAlign::default();

        $(
            $crate::__assign_text_kv!([anchor, offset, size, px, color, font, content, align] $k : $v);
        )*

        $crate::ui::actors::Actor::Text { anchor, offset, size, px, color, font, content, align }
    }};
}

#[macro_export]
macro_rules! frame {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor   = $crate::ui::actors::Anchor::TopLeft;
        let mut offset   = [0.0_f32, 0.0_f32];
        let mut size     = $crate::ui::actors::SizeSpec::Px { w: 0.0, h: 0.0 };
        let mut children: ::std::vec::Vec<$crate::ui::actors::Actor> = ::std::vec![];
        let mut background: Option<$crate::ui::actors::Background> = None;

        $(
            $crate::__assign_frame_kv!([anchor, offset, size, children, background] $k : $v);
        )*

        $crate::ui::actors::Actor::Frame { anchor, offset, size, children, background }
    }};
}

/* -------------------- KV helpers (exported) -------------------- */

#[macro_export]
macro_rules! __assign_anchor {
    ( $var:ident = $name:ident ) => {
        $var = $crate::ui::actors::Anchor::$name;
    };
}

#[macro_export]
macro_rules! __assign_size {
    ( $var:ident = [ $w:expr , $h:expr ] ) => {
        $var = $crate::ui::actors::SizeSpec::Px { w: ($w) as f32, h: ($h) as f32 };
    };
}

#[macro_export]
macro_rules! __actor_common_props {
    ( [ $a:ident, $o:ident, $s:ident ] anchor : $v:ident ) => { $crate::__assign_anchor!($a = $v); };
    ( [ $a:ident, $o:ident, $s:ident ] offset : [ $x:expr , $y:expr ] ) => { $o = [ ($x) as f32, ($y) as f32 ]; };
    ( [ $a:ident, $o:ident, $s:ident ] size   : [ $w:expr , $h:expr ] ) => { $crate::__assign_size!($s = [ $w , $h ]); };
    ( [ $a:ident, $o:ident, $s:ident ] fill   : true ) => { $s = $crate::ui::actors::SizeSpec::Fill; };
}

#[macro_export]
macro_rules! __assign_quad_kv {
    // Quad-specific properties (MUST COME FIRST)
    ( [ $a:ident, $o:ident, $s:ident, $c:ident ] color  : [ $r:expr , $g:expr , $b:expr , $a4:expr ] ) => {
        $c = [ ($r) as f32, ($g) as f32, ($b) as f32, ($a4) as f32 ];
    };
    ( [ $a:ident, $o:ident, $s:ident, $c:ident ] color  : $expr:expr ) => {
        $c = $expr;
    };
    // Delegate common properties (catch-all MUST COME LAST)
    ( [ $a:ident, $o:ident, $s:ident, $c:ident ] $k:ident : $v:tt ) => {
        $crate::__actor_common_props!([$a, $o, $s] $k: $v);
    };
}

#[macro_export]
macro_rules! __assign_sprite_kv {
    // Sprite-specific properties (MUST COME FIRST)
    ( [ $a:ident, $o:ident, $s:ident, $t:ident ] texture : $tex:expr ) => { $t = $tex; };
    // Delegate common properties (catch-all MUST COME LAST)
    ( [ $a:ident, $o:ident, $s:ident, $t:ident ] $k:ident : $v:tt ) => {
        $crate::__actor_common_props!([$a, $o, $s] $k: $v);
    };
}

#[macro_export]
macro_rules! __assign_text_kv {
    // Text-specific properties (MUST COME FIRST)
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] px     : $v:expr ) => { $px = ($v) as f32; };
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] color  : [ $r:expr , $g:expr , $b:expr , $a4:expr ] ) => {
        $c = [ ($r) as f32, ($g) as f32, ($b) as f32, ($a4) as f32 ];
    };
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] color  : $expr:expr ) => { $c = $expr; };
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] font   : $name:expr ) => { $f = $name; };
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] text   : $val:expr ) => { $t = ($val).to_string(); };
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] align  : $v:ident ) => { $al = $crate::ui::actors::TextAlign::$v; };
    // Delegate common properties (catch-all MUST COME LAST)
    ( [ $a:ident, $o:ident, $s:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident ] $k:ident : $v:tt ) => {
        $crate::__actor_common_props!([$a, $o, $s] $k: $v);
    };
}

#[macro_export]
macro_rules! __assign_frame_kv {
    // Frame-specific properties (MUST COME FIRST)
    ( [ $a:ident, $o:ident, $s:ident, $ch:ident, $b:ident ] children : [ $( $child:expr ),* $(,)? ] ) => {
        $ch = ::std::vec![ $( $child ),* ];
    };
    ( [ $a:ident, $o:ident, $s:ident, $ch:ident, $b:ident ] bg_color : $expr:expr ) => {
        $b = Some($crate::ui::actors::Background::Color($expr));
    };
    ( [ $a:ident, $o:ident, $s:ident, $ch:ident, $b:ident ] bg_texture : $tex:expr ) => {
        $b = Some($crate::ui::actors::Background::Texture($tex));
    };
    // Delegate common properties (catch-all MUST COME LAST)
    ( [ $a:ident, $o:ident, $s:ident, $ch:ident, $b:ident ] $k:ident : $v:tt ) => {
        $crate::__actor_common_props!([$a, $o, $s] $k: $v);
    };
}
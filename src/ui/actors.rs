// src/ui/actors.rs
use crate::ui::primitives::{UIElement, Quad, Sprite, Text};
use crate::ui::build::{sm_rect_to_center_size, screen_right, screen_bottom};
use crate::core::space::Metrics;
use cgmath::Vector2;

/* ---------- Declarative actor model (pure data) ---------- */

#[derive(Clone, Copy)]
pub enum Anchor {
    TopLeft, TopRight, BottomLeft, BottomRight,
    Center, TopCenter, BottomCenter, LeftCenter, RightCenter,
}

#[derive(Clone, Copy)]
pub enum SizeSpec {
    Px { w: f32, h: f32 },
    SquarePx(f32),
    WPercentHpx { w_pct: f32, h: f32 },
    HPercentWpx { h_pct: f32, w: f32 },
    FillParent,
}

#[derive(Clone, Copy)]
pub struct Layout {
    pub anchor: Anchor,
    pub offset_px: [f32; 2], // top-left UI space, pixels
    pub size: SizeSpec,
}

#[derive(Clone)]
pub enum Actor {
    Frame { layout: Layout, children: Vec<Actor> },
    Quad  { layout: Layout, color: [f32;4] },
    Sprite{ layout: Layout, texture_id: &'static str },
    Text  { layout: Layout, font_id: &'static str, pixel_height: f32, color: [f32;4], content: String },
}

/* ---------- Pure resolver: Actor tree -> Vec<UIElement> ---------- */

#[inline(always)]
fn resolve_size(size: SizeSpec, avail_w: f32, avail_h: f32) -> (f32, f32) {
    match size {
        SizeSpec::Px { w, h } => (w, h),
        SizeSpec::SquarePx(s) => (s, s),
        SizeSpec::WPercentHpx { w_pct, h } => (w_pct.clamp(0.0, 1.0) * avail_w, h),
        SizeSpec::HPercentWpx { h_pct, w } => (w, h_pct.clamp(0.0, 1.0) * avail_h),
        SizeSpec::FillParent => (avail_w, avail_h),
    }
}

#[inline(always)]
fn anchor_tl(anchor: Anchor, pw: f32, ph: f32, w: f32, h: f32) -> (f32, f32) {
    match anchor {
        Anchor::TopLeft       => (0.0,         0.0),
        Anchor::TopRight      => (pw - w,      0.0),
        Anchor::BottomLeft    => (0.0,         ph - h),
        Anchor::BottomRight   => (pw - w,      ph - h),
        Anchor::Center        => (0.5*(pw-w),  0.5*(ph-h)),
        Anchor::TopCenter     => (0.5*(pw-w),  0.0),
        Anchor::BottomCenter  => (0.5*(pw-w),  ph - h),
        Anchor::LeftCenter    => (0.0,         0.5*(ph-h)),
        Anchor::RightCenter   => (pw - w,      0.5*(ph-h)),
    }
}

#[inline(always)]
fn push_rect(out: &mut Vec<UIElement>, m: &Metrics, x_tl: f32, y_tl: f32, w: f32, h: f32,
             make: impl FnOnce([f32;2],[f32;2]) -> UIElement) {
    let (c, s) = sm_rect_to_center_size(x_tl, y_tl, w, h, m);
    out.push(make(c, s));
}

/// Public: build a full UI list from roots. Parent is whole screen (0..W, 0..H in UI px).
pub fn build_actors(actors: &[Actor], m: &Metrics) -> Vec<UIElement> {
    let mut out = Vec::new();
    let pw = screen_right(m);
    let ph = screen_bottom(m);
    for a in actors {
        expand(a, m, 0.0, 0.0, pw, ph, &mut out);
    }
    out
}

fn expand(a: &Actor, m: &Metrics, px: f32, py: f32, pw: f32, ph: f32, out: &mut Vec<UIElement>) {
    match a {
        Actor::Frame { layout, children } => {
            let (w,h)   = resolve_size(layout.size, pw, ph);
            let (ax,ay) = anchor_tl(layout.anchor, pw, ph, w, h);
            let x = px + ax + layout.offset_px[0];
            let y = py + ay + layout.offset_px[1];

            // Sub-rectangle metrics for children
            let subm = Metrics { left: m.left + x, right: m.left + x + w, top: m.top - y, bottom: m.top - y - h };
            for c in children { expand(c, &subm, 0.0, 0.0, w, h, out); }
        }
        Actor::Quad { layout, color } => {
            let (w,h)   = resolve_size(layout.size, pw, ph);
            let (ax,ay) = anchor_tl(layout.anchor, pw, ph, w, h);
            let x = px + ax + layout.offset_px[0];
            let y = py + ay + layout.offset_px[1];
            push_rect(out, m, x, y, w, h, |c,s| UIElement::Quad(Quad {
                center: Vector2::new(c[0], c[1]), size: Vector2::new(s[0], s[1]), color: *color
            }));
        }
        Actor::Sprite { layout, texture_id } => {
            let (w,h)   = resolve_size(layout.size, pw, ph);
            let (ax,ay) = anchor_tl(layout.anchor, pw, ph, w, h);
            let x = px + ax + layout.offset_px[0];
            let y = py + ay + layout.offset_px[1];
            push_rect(out, m, x, y, w, h, |c,s| UIElement::Sprite(Sprite {
                center: Vector2::new(c[0], c[1]), size: Vector2::new(s[0], s[1]), texture_id: *texture_id
            }));
        }
        Actor::Text { layout, font_id, pixel_height, color, content } => {
            let (w,h)   = resolve_size(layout.size, pw, ph);
            let (ax,ay) = anchor_tl(layout.anchor, pw, ph, w, h);
            let x_tl = px + ax + layout.offset_px[0];
            let y_tl = py + ay + layout.offset_px[1];
            let origin_world_x = m.left + x_tl;
            let origin_world_y = m.top  - y_tl;
            out.push(UIElement::Text(Text {
                origin: Vector2::new(origin_world_x, origin_world_y),
                pixel_height: *pixel_height,
                color: *color,
                font_id: *font_id,
                content: content.clone(),
            }));
        }
    }
}

/* ---------- Tiny DSL (named-arg-ish macros) ---------- */

// ---- quad! ----
#[macro_export]
macro_rules! quad {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        use $crate::ui::actors::{Actor, Layout, Anchor, SizeSpec};
        let (mut anchor, mut offset, mut size, mut color) =
            (Anchor::TopLeft, [0.0f32,0.0], SizeSpec::SquarePx(10.0), [1.0f32,1.0,1.0,1.0]);
        $( $crate::quad!(@set anchor, offset, size, color; $k : $v); )*
        Actor::Quad { layout: Layout { anchor, offset_px: offset, size }, color }
    }};

    (@set $a:ident,$o:ident,$s:ident,$c:ident; anchor : $v:ident) => { $a = Anchor::$v; };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; offset : [ $x:expr , $y:expr ]) => { $o = [$x as f32, $y as f32]; };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; size   : [ $w:expr , $h:expr ]) => { $s = SizeSpec::Px { w: $w as f32, h: $h as f32 }; };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; square : $n:expr ) => { $s = SizeSpec::SquarePx($n as f32); };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; fill   : $flag:expr) => { if $flag { $s = SizeSpec::FillParent; } };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; wpercent_hpx : [ $pct:expr , $h:expr ]) => {
        $s = SizeSpec::WPercentHpx { w_pct: $pct as f32, h: $h as f32 };
    };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; hpercent_wpx : [ $pct:expr , $w:expr ]) => {
        $s = SizeSpec::HPercentWpx { h_pct: $pct as f32, w: $w as f32 };
    };
    (@set $a:ident,$o:ident,$s:ident,$c:ident; color  : [ $r:expr , $g:expr , $b:expr , $al:expr ]) => {
        $c = [$r as f32, $g as f32, $b as f32, $al as f32];
    };
}

// ---- sprite! ----
#[macro_export]
macro_rules! sprite {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        use $crate::ui::actors::{Actor, Layout, Anchor, SizeSpec};
        let (mut anchor, mut offset, mut size, mut tex) =
            (Anchor::TopLeft, [0.0f32,0.0], SizeSpec::SquarePx(64.0), "");
        $( $crate::sprite!(@set anchor, offset, size, tex; $k : $v); )*
        Actor::Sprite { layout: Layout { anchor, offset_px: offset, size }, texture_id: tex }
    }};

    (@set $a:ident,$o:ident,$s:ident,$t:ident; anchor : $v:ident) => { $a = Anchor::$v; };
    (@set $a:ident,$o:ident,$s:ident,$t:ident; offset : [ $x:expr , $y:expr ]) => { $o = [$x as f32, $y as f32]; };
    (@set $a:ident,$o:ident,$s:ident,$t:ident; size   : [ $w:expr , $h:expr ]) => { $s = SizeSpec::Px { w: $w as f32, h: $h as f32 }; };
    (@set $a:ident,$o:ident,$s:ident,$t:ident; square : $n:expr ) => { $s = SizeSpec::SquarePx($n as f32); };
    (@set $a:ident,$o:ident,$s:ident,$t:ident; fill   : $flag:expr) => { if $flag { $s = SizeSpec::FillParent; } };
    (@set $a:ident,$o:ident,$s:ident,$t:ident; texture: $id:expr ) => { $t = $id; };
}

// ---- text! ----
#[macro_export]
macro_rules! text {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        use $crate::ui::actors::{Actor, Layout, Anchor, SizeSpec};
        let (mut anchor, mut offset, mut size) = (Anchor::TopLeft, [0.0f32,0.0], SizeSpec::Px{w:0.0,h:0.0});
        let (mut font, mut px, mut color, mut content) = ("wendy", 32.0f32, [1.0f32,1.0,1.0,1.0], String::new());
        $( $crate::text!(@set anchor, offset, size, font, px, color, content; $k : $v); )*
        Actor::Text { layout: Layout { anchor, offset_px: offset, size }, font_id: font, pixel_height: px, color, content }
    }};

    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; anchor : $v:ident) => { $a = Anchor::$v; };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; offset : [ $x:expr , $y:expr ]) => { $o = [$x as f32, $y as f32]; };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; size   : [ $w:expr , $h:expr ]) => { $s = SizeSpec::Px { w: $w as f32, h: $h as f32 }; };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; font   : $id:expr ) => { $f = $id; };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; px     : $v:expr ) => { $px = $v as f32; };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; color  : [ $r:expr , $g:expr , $b:expr , $al:expr ]) => {
        $c = [$r as f32, $g as f32, $b as f32, $al as f32];
    };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; content: $txt:expr) => { $t = $txt.to_string(); };
    (@set $a:ident,$o:ident,$s:ident,$f:ident,$px:ident,$c:ident,$t:ident; text   : $txt:expr) => { $t = $txt.to_string(); };
}

// ---- frame! ----
#[macro_export]
macro_rules! frame {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        use $crate::ui::actors::{Actor, Layout, Anchor, SizeSpec};
        let (mut anchor, mut offset, mut size) = (Anchor::TopLeft, [0.0f32,0.0], SizeSpec::Px { w:0.0, h:0.0 });
        let mut children: Vec<Actor> = Vec::new();
        $( $crate::frame!(@set anchor, offset, size, children; $k : $v); )*
        Actor::Frame { layout: Layout { anchor, offset_px: offset, size }, children }
    }};

    (@set $a:ident,$o:ident,$s:ident,$ch:ident; anchor : $v:ident) => { $a = Anchor::$v; };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; offset : [ $x:expr , $y:expr ]) => { $o = [$x as f32, $y as f32]; };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; size   : [ $w:expr , $h:expr ]) => { $s = SizeSpec::Px { w: $w as f32, h: $h as f32 }; };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; square : $n:expr ) => { $s = SizeSpec::SquarePx($n as f32); };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; fill   : $flag:expr) => { if $flag { $s = SizeSpec::FillParent; } };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; wpercent_hpx : [ $pct:expr , $h:expr ]) => {
        $s = SizeSpec::WPercentHpx { w_pct: $pct as f32, h: $h as f32 };
    };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; hpercent_wpx : [ $pct:expr , $w:expr ]) => {
        $s = SizeSpec::HPercentWpx { h_pct: $pct as f32, w: $w as f32 };
    };
    (@set $a:ident,$o:ident,$s:ident,$ch:ident; children : [ $( $child:expr ),* $(,)? ]) => {
        $ch = vec![ $( $child ),* ];
    };
}

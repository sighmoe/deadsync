#![allow(clippy::too_many_arguments)]

use std::borrow::Cow;

use crate::core::gfx::BlendMode;
use crate::ui::actors::{Actor, SizeSpec, SpriteSource, TextAlign};
use crate::ui::{anim, runtime};

/* =============================== MODS =============================== */

/// All commands as data.
#[allow(dead_code)]
#[derive(Clone)]
pub enum Mod<'a> {
    // position
    Xy(f32, f32),
    SetX(f32),
    SetY(f32),
    AddX(f32),
    AddY(f32),

    // alignment & depth
    Align(f32, f32),
    Z(i16),

    // color
    Tint([f32; 4]),
    Alpha(f32),

    // rotation & blend
    RotationZ(f32),
    Blend(BlendMode),

    // size (pixels)
    SizePx(f32, f32),
    Zoom(f32),
    ZoomX(f32),
    ZoomY(f32),
    AddZoomX(f32),
    AddZoomY(f32),

    // visibility/orientation
    Visible(bool),
    FlipX(bool),
    FlipY(bool),

    // cropping
    CropLeft(f32),
    CropRight(f32),
    CropTop(f32),
    CropBottom(f32),

    // sprite-only
    Cell(u32, u32),
    Grid(u32, u32),
    UvRect([f32; 4]),
    TexVel([f32; 2]),

    // text-only
    Px(f32),
    Font(&'static str),
    Content(Cow<'a, str>), // <-- accepts &str or String
    TAlign(TextAlign),

    // animation (sprite/quad)
    Tween(&'a [anim::Step]),
    SiteId(u64),
}

/* ======================== SPRITE / QUAD CORE ======================== */

#[derive(Clone, Copy)]
struct SpriteSpec {
    align: [f32; 2],
    offset: [f32; 2],
    size: [SizeSpec; 2],
    tint: [f32; 4],
    z: i16,
    visible: bool,
    flip_x: bool,
    flip_y: bool,
    crop: [f32; 4], // L,R,T,B
    blend: BlendMode,
    rot_z_deg: f32,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    uv_rect: Option<[f32; 4]>,
    texvel: Option<[f32; 2]>,
    source: SpriteSource,
}

impl Default for SpriteSpec {
    #[inline(always)]
    fn default() -> Self {
        Self {
            align: [0.5, 0.5],
            offset: [0.0, 0.0],
            size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
            tint: [1.0, 1.0, 1.0, 1.0],
            z: 0,
            visible: true,
            flip_x: false,
            flip_y: false,
            crop: [0.0, 0.0, 0.0, 0.0],
            blend: BlendMode::Alpha,
            rot_z_deg: 0.0,
            cell: None,
            grid: None,
            uv_rect: None,
            texvel: None,
            source: SpriteSource::Solid,
        }
    }
}

#[inline(always)]
fn finish_sprite_spec(s: SpriteSpec) -> Actor {
    Actor::Sprite {
        align: s.align,
        offset: s.offset,
        size: s.size,
        source: s.source,
        tint: s.tint,
        z: s.z,
        cell: s.cell,
        grid: s.grid,
        uv_rect: s.uv_rect,
        visible: s.visible,
        flip_x: s.flip_x,
        flip_y: s.flip_y,
        cropleft:  s.crop[0],
        cropright: s.crop[1],
        croptop:   s.crop[2],
        cropbottom:s.crop[3],
        blend: s.blend,
        rot_z_deg: s.rot_z_deg,
        texcoordvelocity: s.texvel,
    }
}

#[inline(always)]
fn apply_sprite_mods_core<'a>(s: &mut SpriteSpec, mods: &[Mod<'a>]) -> (Option<&'a [anim::Step]>, u64) {
    let mut tween: Option<&'a [anim::Step]> = None;
    let mut site_extra: u64 = 0;

    for m in mods {
        match *m {
            Mod::Xy(x,y)        => { s.offset = [x,y]; }
            Mod::SetX(x)        => { s.offset[0] = x; }
            Mod::SetY(y)        => { s.offset[1] = y; }
            Mod::AddX(dx)       => { s.offset[0] += dx; }
            Mod::AddY(dy)       => { s.offset[1] += dy; }

            Mod::Align(h,v)     => { s.align = [h,v]; }
            Mod::Z(v)           => { s.z = v; }

            Mod::Tint(rgba)     => { s.tint = rgba; }
            Mod::Alpha(a)       => { s.tint[3] = a; }

            Mod::RotationZ(z)   => { s.rot_z_deg = z; }
            Mod::Blend(b)       => { s.blend = b; }

            Mod::SizePx(w,h)    => { s.size = [SizeSpec::Px(w), SizeSpec::Px(h)]; }
            Mod::Zoom(f)        => { s.size = [SizeSpec::Px(f), SizeSpec::Px(f)]; }
            Mod::ZoomX(w)       => { s.size[0] = SizeSpec::Px(w); }
            Mod::ZoomY(h)       => { s.size[1] = SizeSpec::Px(h); }
            Mod::AddZoomX(dw)   => {
                if let SizeSpec::Px(w) = s.size[0] { s.size[0] = SizeSpec::Px(w + dw); }
                else { s.size[0] = SizeSpec::Px(dw); }
            }
            Mod::AddZoomY(dh)   => {
                if let SizeSpec::Px(h) = s.size[1] { s.size[1] = SizeSpec::Px(h + dh); }
                else { s.size[1] = SizeSpec::Px(dh); }
            }

            Mod::Visible(v)     => { s.visible = v; }
            Mod::FlipX(v)       => { s.flip_x = v; }
            Mod::FlipY(v)       => { s.flip_y = v; }

            Mod::CropLeft(v)    => { s.crop[0] = v; }
            Mod::CropRight(v)   => { s.crop[1] = v; }
            Mod::CropTop(v)     => { s.crop[2] = v; }
            Mod::CropBottom(v)  => { s.crop[3] = v; }

            Mod::Cell(c,r)      => { s.cell = Some((c,r)); }
            Mod::Grid(c,r)      => { s.grid = Some((c,r)); }
            Mod::UvRect(uv)     => { s.uv_rect = Some(uv); }
            Mod::TexVel(v)      => { s.texvel = Some(v); }

            Mod::Px(_) | Mod::Font(_) | Mod::Content(_) | Mod::TAlign(_)
                => { debug_assert!(false, "text-only Mod on sprite/quad"); }

            Mod::Tween(steps)   => { tween = Some(steps); }
            Mod::SiteId(id)     => { site_extra = id; }
        }
    }

    (tween, site_extra)
}

#[inline(always)]
fn apply_sprite_tween<'a>(
    s: &mut SpriteSpec,
    tween: Option<&'a [anim::Step]>,
    site_file: &'static str,
    site_line: u32,
    site_col: u32,
    site_extra: u64,
) {
    if let Some(steps) = tween {
        let mut init = anim::TweenState::default();
        init.x = s.offset[0];
        init.y = s.offset[1];
        if let (SizeSpec::Px(w), SizeSpec::Px(h)) = (s.size[0], s.size[1]) {
            init.w = w; init.h = h;
        }
        init.hx = s.align[0];
        init.vy = s.align[1];
        init.tint = s.tint;
        init.visible = s.visible;
        init.flip_x = s.flip_x;
        init.flip_y = s.flip_y;

        let site = runtime::site_id(site_file, site_line, site_col, site_extra);
        let st = runtime::materialize(site, init, steps);

        s.offset = [st.x, st.y];
        s.size   = [SizeSpec::Px(st.w), SizeSpec::Px(st.h)];
        s.align  = [st.hx, st.vy];
        s.tint   = st.tint;
        s.visible= st.visible;
        s.flip_x = st.flip_x;
        s.flip_y = st.flip_y;
    }
}

#[inline(always)]
pub fn sprite<'a>(
    texture: &'static str,
    mods: &[Mod<'a>],
    file: &'static str,
    line: u32,
    col: u32,
) -> Actor {
    let mut s = SpriteSpec { source: SpriteSource::Texture(texture), ..Default::default() };
    let (tween, site_extra) = apply_sprite_mods_core(&mut s, mods);
    apply_sprite_tween(&mut s, tween, file, line, col, site_extra);
    finish_sprite_spec(s)
}

#[inline(always)]
pub fn quad<'a>(
    mods: &[Mod<'a>],
    file: &'static str,
    line: u32,
    col: u32,
) -> Actor {
    let mut s = SpriteSpec::default(); // Solid quad
    let (tween, site_extra) = apply_sprite_mods_core(&mut s, mods);
    apply_sprite_tween(&mut s, tween, file, line, col, site_extra);
    finish_sprite_spec(s)
}

/* ============================= TEXT CORE ============================ */

#[derive(Clone)]
struct TextSpec<'a> {
    align: [f32; 2],
    offset: [f32; 2],
    px: f32,
    color: [f32; 4],
    font: &'static str,
    content: Cow<'a, str>, // <-- Cow
    talign: TextAlign,
    z: i16,
}

impl<'a> Default for TextSpec<'a> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            align: [0.5, 0.5],
            offset: [0.0, 0.0],
            px: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            font: "miso",
            content: Cow::Borrowed(""),
            talign: TextAlign::Left,
            z: 0,
        }
    }
}

#[inline(always)]
fn apply_text_mods<'a>(t: &mut TextSpec<'a>, mods: &[Mod<'a>]) {
    for m in mods {
        match m {
            Mod::Xy(x, y)    => { t.offset = [*x, *y]; }
            Mod::SetX(x)     => { t.offset[0] = *x; }
            Mod::SetY(y)     => { t.offset[1] = *y; }
            Mod::AddX(dx)    => { t.offset[0] += *dx; }
            Mod::AddY(dy)    => { t.offset[1] += *dy; }

            Mod::Align(h, v) => { t.align = [*h, *v]; }
            Mod::Px(p)       => { t.px = *p; }
            Mod::Tint(rgba)  => { t.color = *rgba; }
            Mod::Alpha(a)    => { t.color[3] = *a; }
            Mod::Font(f)     => { t.font = *f; }
            Mod::Content(s)  => { t.content = s.clone(); }
            Mod::TAlign(a)   => { t.talign = *a; }
            Mod::Z(v)        => { t.z = *v; }

            // sprite-only → guard in dev
            Mod::RotationZ(_) | Mod::Blend(_)
            | Mod::SizePx(..) | Mod::Zoom(..) | Mod::ZoomX(..) | Mod::ZoomY(..)
            | Mod::AddZoomX(..) | Mod::AddZoomY(..)
            | Mod::Visible(..) | Mod::FlipX(..) | Mod::FlipY(..)
            | Mod::CropLeft(..) | Mod::CropRight(..) | Mod::CropTop(..) | Mod::CropBottom(..)
            | Mod::Cell(..) | Mod::Grid(..) | Mod::UvRect(..) | Mod::TexVel(..)
            | Mod::Tween(..) | Mod::SiteId(..)
            => { debug_assert!(false, "sprite-only Mod used on text"); }
        }
    }
}

#[inline(always)]
pub fn text<'a>(mods: &[Mod<'a>]) -> Actor {
    let mut t = TextSpec::default();
    apply_text_mods(&mut t, mods);
    Actor::Text {
        align: t.align,
        offset: t.offset,
        px: t.px,
        color: t.color,
        font: t.font,
        content: t.content.into_owned(), // <-- String
        align_text: t.talign,
        z: t.z,
    }
}

/* ================ text-align ident helper (for act!) ================= */

#[macro_export]
macro_rules! __ui_textalign_from_ident {
    (left)   => { $crate::ui::actors::TextAlign::Left };
    (center) => { $crate::ui::actors::TextAlign::Center };
    (right)  => { $crate::ui::actors::TextAlign::Right };
    ($other:ident) => {
        compile_error!(concat!("talign expects left|center|right, got: ", stringify!($other)));
    };
}

/* ========================= compat: act! ============================== */

/// Backward-compatible surface:
///   act!(sprite(tex): align(...): xy(...): zoomto(...): diffuse(...): z(600))
///   act!(quad: ... )
///   act!(text: align(...): xy(...): px(...): font("miso"): text("..."): talign(center): diffuse(...))
#[macro_export]
macro_rules! act {
    // sprite(texture_key):  ...commands...
    (sprite($tex:expr): $($tail:tt)+) => {{
        use ::std::vec::Vec;
        let mut __mods: Vec<$crate::ui::dsl::Mod> = Vec::new();
        let mut __tw_steps: Vec<$crate::ui::anim::Step> = Vec::new();
        let mut __tw_cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None;
        let mut __site_extra: u64 = 0;

        $crate::__dsl_apply!( ($($tail)+) __mods __tw_steps __tw_cur __site_extra );

        if let ::core::option::Option::Some(seg) = __tw_cur.take() { __tw_steps.push(seg.build()); }
        if !__tw_steps.is_empty() {
            __mods.push($crate::ui::dsl::Mod::Tween(&__tw_steps));
        }
        if __site_extra != 0 {
            __mods.push($crate::ui::dsl::Mod::SiteId(__site_extra));
        }

        $crate::ui::dsl::sprite($tex, &__mods, file!(), line!(), column!())
    }};

    // quad: ...commands...
    (quad: $($tail:tt)+) => {{
        use ::std::vec::Vec;
        let mut __mods: Vec<$crate::ui::dsl::Mod> = Vec::new();
        let mut __tw_steps: Vec<$crate::ui::anim::Step> = Vec::new();
        let mut __tw_cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None;
        let mut __site_extra: u64 = 0;

        $crate::__dsl_apply!( ($($tail)+) __mods __tw_steps __tw_cur __site_extra );

        if let ::core::option::Option::Some(seg) = __tw_cur.take() { __tw_steps.push(seg.build()); }
        if !__tw_steps.is_empty() {
            __mods.push($crate::ui::dsl::Mod::Tween(&__tw_steps));
        }
        if __site_extra != 0 {
            __mods.push($crate::ui::dsl::Mod::SiteId(__site_extra));
        }

        $crate::ui::dsl::quad(&__mods, file!(), line!(), column!())
    }};

    // text: ...commands...   (animation commands are ignored for text)
    (text: $($tail:tt)+) => {{
        use ::std::vec::Vec;
        let mut __mods: Vec<$crate::ui::dsl::Mod> = Vec::new();
        let mut __tw_steps: Vec<$crate::ui::anim::Step> = Vec::new(); // ignored
        let mut __tw_cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None; // ignored
        let mut __site_extra: u64 = 0; // ignored

        $crate::__dsl_apply!( ($($tail)+) __mods __tw_steps __tw_cur __site_extra );
        $crate::ui::dsl::text(&__mods)
    }};
}

/* ===================== munchers for act! ============================= */

#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_apply {
    ( () $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident ) => { () };

    ( ($cmd:ident ( $($args:tt)* ) : $($rest:tt)* )
      $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident
    ) => {{
        $crate::__dsl_apply_one!{ $cmd ( $($args)* ) $mods $tw_steps $tw_cur $site_extra }
        $crate::__dsl_apply!( ($($rest)*) $mods $tw_steps $tw_cur $site_extra );
    }};

    ( ($cmd:ident ( $($args:tt)* ) )
      $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident
    ) => {{
        $crate::__dsl_apply_one!{ $cmd ( $($args)* ) $mods $tw_steps $tw_cur $site_extra }
        $crate::__dsl_apply!( () $mods $tw_steps $tw_cur $site_extra );
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_apply_one {
    // -------- meta
    (id ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $site_extra = ($v) as u64;
    }};

    // -------- tween segment controls
    (linear ($d:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(seg) = $tw_cur.take() { $tw_steps.push(seg.build()); }
        $tw_cur = ::core::option::Option::Some($crate::ui::anim::linear(($d) as f32));
    }};
    (accelerate ($d:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(seg) = $tw_cur.take() { $tw_steps.push(seg.build()); }
        $tw_cur = ::core::option::Option::Some($crate::ui::anim::accelerate(($d) as f32));
    }};
    (decelerate ($d:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(seg) = $tw_cur.take() { $tw_steps.push(seg.build()); }
        $tw_cur = ::core::option::Option::Some($crate::ui::anim::decelerate(($d) as f32));
    }};
    (sleep ($d:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(seg) = $tw_cur.take() { $tw_steps.push(seg.build()); }
        $tw_steps.push($crate::ui::anim::sleep(($d) as f32));
    }};

    // -------- tweenable properties (sprite/quad)
    (xy ($xv:expr, $yv:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.xy(($xv) as f32, ($yv) as f32);
            $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::Xy(($xv) as f32, ($yv) as f32));
        }
    }};
    (x ($xv:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.x(($xv) as f32);
            $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::SetX(($xv) as f32));
        }
    }};
    (y ($yv:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.y(($yv) as f32);
            $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::SetY(($yv) as f32));
        }
    }};
    (addx ($dx:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.addx(($dx) as f32);
            $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::AddX(($dx) as f32));
        }
    }};
    (addy ($dy:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.addy(($dy) as f32);
            $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::AddY(($dy) as f32));
        }
    }};
    (zoom ($f:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        let f = ($f) as f32;
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.zoom(f, f); $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::Zoom(f));
        }
    }};
    (zoomto ($nw:expr, $nh:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.zoom(($nw) as f32, ($nh) as f32); $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::SizePx(($nw) as f32, ($nh) as f32));
        }
    }};
    (setsize ($nw:expr, $nh:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $crate::__dsl_apply_one!(zoomto(($nw), ($nh)) $mods $tw_steps $tw_cur $site_extra)
    }};
    (zoomx ($nw:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.zoomx(($nw) as f32); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::ZoomX(($nw) as f32)); }
    }};
    (zoomy ($nh:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.zoomy(($nh) as f32); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::ZoomY(($nh) as f32)); }
    }};
    (addzoomx ($dw:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.addzoomx(($dw) as f32); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddZoomX(($dw) as f32)); }
    }};
    (addzoomy ($dh:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.addzoomy(($dh) as f32); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddZoomY(($dh) as f32)); }
    }};
    (diffuse ($r:expr, $g:expr, $b:expr, $a:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() {
            seg = seg.diffuse(($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32);
            $tw_cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::Tint([($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32]));
        }
    }};
    (alpha ($a:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.alpha(($a) as f32); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Alpha(($a) as f32)); }
    }};
    (diffusealpha ($a:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $crate::__dsl_apply_one!(alpha(($a)) $mods $tw_steps $tw_cur $site_extra)
    }};
    (set_visible ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.set_visible(($v) as bool); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Visible(($v) as bool)); }
    }};
    (visible ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $crate::__dsl_apply_one!(set_visible(($v)) $mods $tw_steps $tw_cur $site_extra)
    }};
    (flipx ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.flip_x(($v) as bool); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FlipX(($v) as bool)); }
    }};
    (flipy ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $tw_cur.take() { seg = seg.flip_y(($v) as bool); $tw_cur = ::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FlipY(($v) as bool)); }
    }};

    // -------- static (non-animated) sprite/quad
    (align ($h:expr, $v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Align(($h) as f32, ($v) as f32));
    }};
    (z ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Z(($v) as i16));
    }};
    (cell ($c:expr, $r:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Cell(($c) as u32, ($r) as u32));
    }};
    (setstate ($i:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Cell(($i) as u32, u32::MAX));
    }};
    (grid ($c:expr, $r:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Grid(($c) as u32, ($r) as u32));
    }};
    (texrect ($u0:expr, $v0:expr, $u1:expr, $v1:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::UvRect([($u0) as f32, ($v0) as f32, ($u1) as f32, ($v1) as f32]));
    }};
    (customtexturerect ($u0:expr, $v0:expr, $u1:expr, $v1:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $crate::__dsl_apply_one!(texrect(($u0), ($v0), ($u1), ($v1)) $mods $tw_steps $tw_cur $site_extra)
    }};
    (texcoordvelocity ($vx:expr, $vy:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TexVel([($vx) as f32, ($vy) as f32]));
    }};
    (cropleft ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::CropLeft(($v) as f32));
    }};
    (cropright ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::CropRight(($v) as f32));
    }};
    (croptop ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::CropTop(($v) as f32));
    }};
    (cropbottom ($v:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::CropBottom(($v) as f32));
    }};
    (blend (alpha) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Alpha));
    }};
    (blend (normal) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Alpha));
    }};
    (blend (add) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Add));
    }};
    (blend (additive) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $crate::__dsl_apply_one!(blend(add) $mods $tw_steps $tw_cur $site_extra)
    }};
    (blend (multiply) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Multiply));
    }};
    (rotation ($z:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::RotationZ(($z) as f32));
    }};
    (rotationz ($z:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $crate::__dsl_apply_one!(rotation(($z)) $mods $tw_steps $tw_cur $site_extra)
    }};

    // -------- text-only
    (px ($p:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Px(($p) as f32));
    }};
    (font ($name:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Font($name));
    }};
    (text ($s:expr) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        // Accept &str or String
        $mods.push($crate::ui::dsl::Mod::Content(::std::borrow::Cow::from(($s))));
    }};
    (talign ($dir:ident) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TAlign($crate::__ui_textalign_from_ident!($dir)));
    }};

    // -------- unknown
    ($other:ident ( $($args:expr),* ) $mods:ident $tw_steps:ident $tw_cur:ident $site_extra:ident) => {
        compile_error!(concat!("act!: unknown command: ", stringify!($other)));
    };
}

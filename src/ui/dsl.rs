use crate::core::gfx::BlendMode;
use crate::ui::actors::{Actor, SizeSpec, SpriteSource, TextAlign};
use crate::ui::{anim, runtime};
use std::borrow::Cow;

#[allow(dead_code)]
#[derive(Clone)]
pub enum Mod<'a> {
    // position
    Xy(f32, f32),
    SetX(f32),
    SetY(f32),
    AddX(f32),
    AddY(f32),

    // pivot inside the rect (0..1)
    Align(f32, f32),
    HAlign(f32),
    VAlign(f32),

    // draw order & color
    Z(i16),
    Tint([f32; 4]),
    Alpha(f32),
    Blend(BlendMode),

    // absolute size (pre-zoom) in SM TL space
    SizePx(f32, f32),

    // StepMania zoom semantics (scale factors)
    Zoom(f32),
    ZoomX(f32),
    ZoomY(f32),
    AddZoomX(f32),
    AddZoomY(f32),

    // helpers that set one axis and preserve aspect
    ZoomToWidth(f32),
    ZoomToHeight(f32),

    // misc
    Flip(bool),

    // cropping (fractions 0..1)
    CropLeft(f32),
    CropRight(f32),
    CropTop(f32),
    CropBottom(f32),

    // NEW: edge fades
    FadeLeft(f32),
    FadeRight(f32),
    FadeTop(f32),
    FadeBottom(f32),

    // texture scroll (kept)
    TexVel([f32; 2]),

    // text
    Font(&'static str),
    Content(std::borrow::Cow<'a, str>),
    TAlign(TextAlign),

    // visibility + rotation
    Visible(bool),
    RotZ(f32),
    AddRotZ(f32),

    // ---- NEW: SM/ITG-compatible sprite controls ----
    /// `setstate(i)` — linear state index (row-major); grid inferred from filename `_CxR`.
    State(u32),
    /// `SetAllStateDelays(seconds)` — uniform delay for each state while animating.
    StateDelay(f32),
    /// `animate(true/false)` — toggles auto state advance.
    Animate(bool),
    /// `customtexturerect(u0,v0,u1,v1)` — normalized UVs, top-left origin.
    UvRect([f32; 4]),

    // runtime/tween plumbing
    Tween(&'a [anim::Step]),
}

/* ======================== SPRITE/QUAD CORE ======================== */

#[inline(always)]
fn build_sprite_like<'a>(
    source: SpriteSource,
    mods: &[Mod<'a>],
    file: &'static str,
    line: u32,
    col: u32,
) -> Actor {
    // defaults
    let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 0.0, 0.0);
    let (mut hx, mut vy) = (0.5, 0.5);
    let mut tint = [1.0, 1.0, 1.0, 1.0];
    let mut z: i16 = 0;
    let (mut vis, mut fx, mut fy) = (true, false, false);
    let (mut cl, mut cr, mut ct, mut cb) = (0.0, 0.0, 0.0, 0.0);
    let (mut fl, mut fr, mut ft, mut fb) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    let mut blend = BlendMode::Alpha;
    let mut rot = 0.0_f32;
    let mut uv: Option<[f32; 4]> = None;
    let mut cell: Option<(u32, u32)> = None;
    let mut grid: Option<(u32, u32)> = None;
    let mut texv: Option<[f32; 2]> = None;
    // animation
    let mut anim_enable = false;
    let mut state_delay = 0.1_f32;
    let (mut tw, _site_ignored): (Option<&[anim::Step]>, u64) = (None, 0);

    // StepMania zoom (scale factors) — allow negatives (we’ll fold to flips)
    let (mut sx, mut sy) = (1.0_f32, 1.0_f32);

    // fold mods in order
    for m in mods {
        match m {
            Mod::Xy(a, b) => { x = *a; y = *b; }
            Mod::SetX(a) => { x = *a; }
            Mod::SetY(b) => { y = *b; }
            Mod::AddX(a) => { x += *a; }
            Mod::AddY(b) => { y += *b; }

            Mod::HAlign(a)   => { hx = *a; }
            Mod::VAlign(b)   => { vy = *b; }
            Mod::Align(a, b) => { hx = *a; vy = *b; }

            Mod::Z(v) => { z = *v; }
            Mod::Tint(rgba) => { tint = *rgba; }
            Mod::Alpha(a) => { tint[3] = *a; }
            Mod::Blend(bm) => { blend = *bm; }

            Mod::SizePx(a, b) => { w = *a; h = *b; }

            // StepMania zoom semantics (scale factors). Keep signs for now.
            Mod::Zoom(f)     => { sx = *f; sy = *f; }
            Mod::ZoomX(a)    => { sx = *a; }
            Mod::ZoomY(b)    => { sy = *b; }
            Mod::AddZoomX(a) => { sx += *a; }
            Mod::AddZoomY(b) => { sy += *b; }

            // aspect-preserving absolute sizes
            Mod::ZoomToWidth(new_w) => {
                if w > 0.0 && h > 0.0 {
                    let aspect = h / w;
                    w = *new_w;
                    h = w * aspect;
                } else {
                    w = *new_w;
                }
            }
            Mod::ZoomToHeight(new_h) => {
                if w > 0.0 && h > 0.0 {
                    let aspect = w / h;
                    h = *new_h;
                    w = h * aspect;
                } else {
                    h = *new_h;
                }
            }

            Mod::Flip(v) => { fx = *v; }
            Mod::CropLeft(v)   => { cl = *v; }
            Mod::CropRight(v)  => { cr = *v; }
            Mod::CropTop(v)    => { ct = *v; }
            Mod::CropBottom(v) => { cb = *v; }

            Mod::FadeLeft(v)    => { fl = *v; }
            Mod::FadeRight(v)   => { fr = *v; }
            Mod::FadeTop(v)     => { ft = *v; }
            Mod::FadeBottom(v)  => { fb = *v; }

            Mod::TexVel(v)     => { texv = Some(*v); }

            Mod::Visible(v) => { vis = *v; }
            Mod::RotZ(d)    => { rot = *d; }
            Mod::AddRotZ(dd)=> { rot += *dd; }

            // text-only mods ignored here
            Mod::Font(_) | Mod::Content(_) | Mod::TAlign(_) => {}
            Mod::Tween(steps) => { tw = Some(steps); }
            Mod::State(i) => {
                // sentinel (i, u32::MAX) = linear frame index; grid inferred from file name (_CxR)
                cell = Some((*i, u32::MAX));
                grid = None; // let filename inference choose cols/rows
                uv   = None; // state selection overrides any custom UV rect
            }
            Mod::UvRect(r) => {
                uv   = Some(*r); // normalized TL-origin [u0,v0,u1,v1]
                cell = None;     // explicit rect overrides grid/cell
                grid = None;
            }
            Mod::Animate(v) => { anim_enable = *v; }
            Mod::StateDelay(s) => { state_delay = (*s).max(0.0); }
        }
    }

    // tween (optional)
    if let Some(steps) = tw {
        let mut init = anim::TweenState::default();
        init.x = x; init.y = y; init.w = w; init.h = h;
        init.hx = hx; init.vy = vy;
        init.tint = tint;
        init.visible = vis; init.flip_x = fx; init.flip_y = fy;
        init.rot_z = rot;
        init.fade_l = fl; init.fade_r = fr; init.fade_t = ft; init.fade_b = fb;

        let sid = runtime::site_id(file, line, col, 0);
        let s = runtime::materialize(sid, init, steps);

        x = s.x; y = s.y; w = s.w; h = s.h;
        hx = s.hx; vy = s.vy;
        tint = s.tint; vis = s.visible; fx = s.flip_x; fy = s.flip_y;
        rot = s.rot_z;
        fl = s.fade_l; fr = s.fade_r; ft = s.fade_t; fb = s.fade_b;
        // tweened crops override static ones if present
        cl = s.crop_l; cr = s.crop_r; ct = s.crop_t; cb = s.crop_b;
        
    }

    // --- SM/ITG semantics: negative zoom flips, not negative geometry ---
    // Convert sign of zoom into flip flags, keep positive magnitudes.
    if sx < 0.0 { fx = !fx; sx = -sx; }
    if sy < 0.0 { fy = !fy; sy = -sy; }

    // apply zoom last
    if w != 0.0 || h != 0.0 {
        w *= sx;
        h *= sy;
    }

    Actor::Sprite {
        align: [hx, vy],
        offset: [x, y],
        size: [SizeSpec::Px(w), SizeSpec::Px(h)],
        source,
        tint,
        z,
        cell,
        grid,
        uv_rect: uv,
        visible: vis,
        flip_x: fx,
        flip_y: fy,
        cropleft: cl,
        cropright: cr,
        croptop: ct,
        cropbottom: cb,
        fadeleft: fl,
        faderight: fr,
        fadetop: ft,
        fadebottom: fb,
        blend,
        rot_z_deg: rot,
        texcoordvelocity: texv,
        animate: anim_enable,
        state_delay,
    }
}

#[inline(always)]
pub fn sprite<'a>(tex: &'static str, mods: &[Mod<'a>], f: &'static str, l: u32, c: u32) -> Actor {
    build_sprite_like(SpriteSource::Texture(tex), mods, f, l, c)
}
#[inline(always)]
pub fn quad<'a>(mods: &[Mod<'a>], f: &'static str, l: u32, c: u32) -> Actor {
    build_sprite_like(SpriteSource::Solid, mods, f, l, c)
}

/* ============================== TEXT =============================== */

#[inline(always)]
pub fn text<'a>(mods: &[Mod<'a>]) -> Actor {
    let (mut x, mut y) = (0.0, 0.0);
    let (mut hx, mut vy) = (0.5, 0.5);
    let px = 16.0_f32; // internal base pixel height (no public `px` command)
    let mut color = [1.0, 1.0, 1.0, 1.0];
    let mut font: &'static str = "miso";
    let mut content: Cow<'a, str> = Cow::Borrowed("");
    let mut talign = TextAlign::Left;
    let mut z: i16 = 0;

    // zoom + optional fit targets
    let (mut sx, mut sy) = (1.0_f32, 1.0_f32);
    let (mut fit_w, mut fit_h): (Option<f32>, Option<f32>) = (None, None);

    // NEW: SM-compatible — text respects blend mode (default Normal/Alpha)
    let mut blend = BlendMode::Alpha;

    for m in mods {
        match m {
            Mod::Xy(a, b)    => { x = *a; y = *b; }
            Mod::SetX(a)     => { x = *a; }
            Mod::SetY(b)     => { y = *b; }
            Mod::AddX(a)     => { x += *a; }
            Mod::AddY(b)     => { y += *b; }

            Mod::HAlign(a)   => { hx = *a; }
            Mod::VAlign(b)   => { vy = *b; }
            Mod::Align(a, b) => { hx = *a; vy = *b; }

            Mod::Tint(r)     => { color = *r; }
            Mod::Alpha(a)    => { color[3] = *a; }
            Mod::Font(f)     => { font = *f; }
            Mod::Content(s)  => { content = s.clone(); }
            Mod::TAlign(a)   => { talign = *a; }
            Mod::Z(v)        => { z = *v; }

            Mod::Zoom(f)     => { sx = *f; sy = *f; }
            Mod::ZoomX(a)    => { sx = *a; }
            Mod::ZoomY(b)    => { sy = *b; }
            Mod::AddZoomX(a) => { sx += *a; }
            Mod::AddZoomY(b) => { sy += *b; }

            // capture fit targets (applied at layout with actual metrics)
            Mod::ZoomToWidth(w)  => { fit_w = Some(*w); }
            Mod::ZoomToHeight(h) => { fit_h = Some(*h); }

            // NEW: honor blend mode for text
            Mod::Blend(bm)       => { blend = *bm; }

            // sprite-only mods ignored here
            _ => {}
        }
    }

    Actor::Text {
        align: [hx, vy],
        offset: [x, y],
        px,
        color,
        font,
        content: content.into_owned(),
        align_text: talign,
        z,
        scale: [sx, sy],
        fit_width: fit_w,
        fit_height: fit_h,
        blend,
    }
}

/* ========================= compat: act! ============================== */

#[macro_export]
macro_rules! __ui_textalign_from_ident {
    (left) => {
        $crate::ui::actors::TextAlign::Left
    };
    (center) => {
        $crate::ui::actors::TextAlign::Center
    };
    (right) => {
        $crate::ui::actors::TextAlign::Right
    };
    ($other:ident) => {
        compile_error!(concat!(
            "horizalign expects left|center|right, got: ",
            stringify!($other)
        ));
    };
}

#[macro_export]
macro_rules! __ui_halign_from_ident {
    (left) => { 0.0f32 };
    (center) => { 0.5f32 };
    (right) => { 1.0f32 };
    ($other:ident) => {
        compile_error!(concat!("halign expects left|center|right, got: ", stringify!($other)));
    };
}

#[macro_export]
macro_rules! __ui_valign_from_ident {
    (top) => { 0.0f32 };
    (middle) => { 0.5f32 };
    (center) => { 0.5f32 };
    (bottom) => { 1.0f32 };
    ($other:ident) => {
        compile_error!(concat!("valign expects top|middle|center|bottom, got: ", stringify!($other)));
    };
}

#[macro_export]
macro_rules! act {
    (sprite($tex:expr): $($tail:tt)+) => {{
        let mut __mods = ::std::vec::Vec::new();
        let mut __tw = ::std::vec::Vec::new();
        let mut __cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None;
        $crate::__dsl_apply!( ($($tail)+) __mods __tw __cur _dummy_site );
        if let ::core::option::Option::Some(seg)=__cur.take(){__tw.push(seg.build());}
        if !__tw.is_empty(){ __mods.push($crate::ui::dsl::Mod::Tween(&__tw)); }
        $crate::ui::dsl::sprite($tex, &__mods, file!(), line!(), column!())
    }};
    (quad: $($tail:tt)+) => {{
        let mut __mods = ::std::vec::Vec::new();
        let mut __tw = ::std::vec::Vec::new();
        let mut __cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None;
        $crate::__dsl_apply!( ($($tail)+) __mods __tw __cur _dummy_site );
        if let ::core::option::Option::Some(seg)=__cur.take(){__tw.push(seg.build());}
        if !__tw.is_empty(){ __mods.push($crate::ui::dsl::Mod::Tween(&__tw)); }
        $crate::ui::dsl::quad(&__mods, file!(), line!(), column!())
    }};
    (text: $($tail:tt)+) => {{
        let mut __mods = ::std::vec::Vec::new();
        let mut __tw: ::std::vec::Vec<$crate::ui::anim::Step> = ::std::vec::Vec::new(); // typed
        let mut __cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None; // ignored
        $crate::__dsl_apply!( ($($tail)+) __mods __tw __cur _dummy_site );
        $crate::ui::dsl::text(&__mods)
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_apply {
    ( () $mods:ident $tw:ident $cur:ident $site:ident ) => { () };
    ( ($cmd:ident ( $($args:tt)* ) : $($rest:tt)* ) $mods:ident $tw:ident $cur:ident $site:ident ) => {{
        $crate::__dsl_apply_one!{ $cmd ( $($args)* ) $mods $tw $cur $site }
        $crate::__dsl_apply!( ($($rest)*) $mods $tw $cur $site );
    }};
    ( ($cmd:ident ( $($args:tt)* ) ) $mods:ident $tw:ident $cur:ident $site:ident ) => {{
        $crate::__dsl_apply_one!{ $cmd ( $($args)* ) $mods $tw $cur $site }
        $crate::__dsl_apply!( () $mods $tw $cur $site );
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __dsl_apply_one {
    // --- segment controls ---
    (linear ($d:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(seg)=$cur.take(){$tw.push(seg.build());}
        $cur = ::core::option::Option::Some($crate::ui::anim::linear(($d) as f32));
    }};
    (accelerate ($d:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(seg)=$cur.take(){$tw.push(seg.build());}
        $cur = ::core::option::Option::Some($crate::ui::anim::accelerate(($d) as f32));
    }};
    (decelerate ($d:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(seg)=$cur.take(){$tw.push(seg.build());}
        $cur = ::core::option::Option::Some($crate::ui::anim::decelerate(($d) as f32));
    }};
    (sleep ($d:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(seg)=$cur.take(){$tw.push(seg.build());}
        $tw.push($crate::ui::anim::sleep(($d) as f32));
    }};

    // --- tweenable props ---
    (xy ($x:expr, $y:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.x(($x) as f32).y(($y) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Xy(($x) as f32, ($y) as f32)); }
    }};
    (x ($x:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.x(($x) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::SetX(($x) as f32)); }
    }};
    (y ($y:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.y(($y) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::SetY(($y) as f32)); }
    }};
    (addx ($dx:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.addx(($dx) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddX(($dx) as f32)); }
    }};
    (addy ($dy:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.addy(($dy) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddY(($dy) as f32)); }
    }};

    // --- color (present both for sprite & text) ---
    (diffuse ($r:expr,$g:expr,$b:expr,$a:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $cur.take() {
            seg = seg.diffuse(($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32);
            $cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::Tint([($r) as f32,($g) as f32,($b) as f32,($a) as f32]));
        }
    }};
    (alpha ($a:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg) = $cur.take() {
            seg = seg.alpha(($a) as f32);
            $cur = ::core::option::Option::Some(seg);
        } else {
            $mods.push($crate::ui::dsl::Mod::Alpha(($a) as f32));
        }
    }};
    (diffusealpha ($a:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Alpha(($a) as f32));
    }};

    // --- StepMania zoom semantics (scale) ---
    (zoom ($f:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let f=($f) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.zoom(f,f); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Zoom(f)); }
    }};
    (zoomx ($f:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let f=($f) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.zoomx(f); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::ZoomX(f)); }
    }};
    (zoomy ($f:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let f=($f) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.zoomy(f); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::ZoomY(f)); }
    }};
    (addzoomx ($df:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let df=($df) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.addzoomx(df); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddZoomX(df)); }
    }};
    (addzoomy ($df:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let df=($df) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.addzoomy(df); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddZoomY(df)); }
    }};

    // Absolute size (zoomto/setsize) — tweenable size op
    (zoomto ($w:expr, $h:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.size(($w) as f32, ($h) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::SizePx(($w) as f32, ($h) as f32)); }
    }};
    (setsize ($w:expr, $h:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $crate::__dsl_apply_one!(zoomto(($w), ($h)) $mods $tw $cur $site)
    }};

    // --- absolute size helpers preserving aspect ---------------------
    (zoomtowidth ($w:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::ZoomToWidth(($w) as f32));
    }};
    (zoomtoheight ($h:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::ZoomToHeight(($h) as f32));
    }};

    // static sprite bits / cropping / uv / blend ---------------------
    (align ($h:expr,$v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Align(($h) as f32, ($v) as f32));
    }};
    (halign ($dir:ident) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::HAlign($crate::__ui_halign_from_ident!($dir)));
    }};
    (halign ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::HAlign(($v) as f32));
    }};
    (valign ($dir:ident) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::VAlign($crate::__ui_valign_from_ident!($dir)));
    }};
    (valign ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::VAlign(($v) as f32));
    }};

    (z ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Z(($v) as i16)); }};
    (texcoordvelocity ($vx:expr,$vy:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TexVel([($vx) as f32, ($vy) as f32]));
    }};
    (cropleft ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.cropleft(($v) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::CropLeft(($v) as f32)); }
    }};
    (cropright ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.cropright(($v) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::CropRight(($v) as f32)); }
    }};
    (croptop ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.croptop(($v) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::CropTop(($v) as f32)); }
    }};
    (cropbottom ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.cropbottom(($v) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::CropBottom(($v) as f32)); }
    }};
    // edge fades (0..1 of visible width/height)
    (fadeleft ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let vv = ($v) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.fadeleft(vv); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FadeLeft(vv)); }
    }};
    (faderight ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let vv = ($v) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.faderight(vv); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FadeRight(vv)); }
    }};
    (fadetop ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let vv = ($v) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.fadetop(vv); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FadeTop(vv)); }
    }};
    (fadebottom ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let vv = ($v) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.fadebottom(vv); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FadeBottom(vv)); }
    }};
    // --- SM/ITG Sprite: choose frame ---
    (setstate ($i:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::State(($i) as u32));
    }};
    // animation control
    (animate ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Animate(($v) as bool));
    }};
    (setallstatedelays ($s:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::StateDelay(($s) as f32));
    }};
    // --- SM/ITG Sprite: explicit UVs (normalized, top-left origin) ---
    (customtexturerect ($u0:expr, $v0:expr, $u1:expr, $v1:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::UvRect([($u0) as f32, ($v0) as f32, ($u1) as f32, ($v1) as f32]));
    }};

    // flip (horizontal)
    (flip ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Flip(($v) as bool));
    }};

    // --- visibility (immediate or inside a tween) ---
    (visible ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.set_visible(($v) as bool); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Visible(($v) as bool)); }
    }};

    // --- rotationz (degrees) ---
    (rotationz ($deg:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let d=($deg) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.rotationz(d); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::RotZ(d)); }
    }};

    (addrotationz ($ddeg:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        let dd=($ddeg) as f32;
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.addrotationz(dd); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::AddRotZ(dd)); }
    }};

    // blends: normal, add, multiply, subtract
    (blend (normal) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Alpha));
    }};
    (blend (add) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Add));
    }};
    (blend (multiply) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Multiply));
    }};
    (blend (subtract) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Subtract));
    }};

    // Text properties (SM-compatible)
    (font ($n:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Font($n)); }};
    (settext ($s:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Content(::std::borrow::Cow::from(($s))));
    }};
    (horizalign ($dir:ident) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TAlign($crate::__ui_textalign_from_ident!($dir)));
    }};

    // unknown
    ($other:ident ( $($args:expr),* ) $mods:ident $tw:ident $cur:ident $site:ident) => {
        compile_error!(concat!("act!: unknown or removed command: ", stringify!($other)));
    };
}

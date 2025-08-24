use crate::core::gfx::BlendMode;
use crate::ui::actors::{Actor, SizeSpec, SpriteSource, TextAlign};
use crate::ui::{anim, runtime};
use std::borrow::Cow;

#[allow(dead_code)]
#[derive(Clone)]
pub enum Mod<'a> {
    Xy(f32, f32),
    SetX(f32),
    SetY(f32),
    AddX(f32),
    AddY(f32),

    Align(f32, f32),
    HAlign(f32),
    VAlign(f32),

    Z(i16),
    Tint([f32; 4]),
    Alpha(f32),
    RotationZ(f32),
    Blend(BlendMode),

    // Absolute size
    SizePx(f32, f32),

    // StepMania zoom (scale factors)
    Zoom(f32),
    ZoomX(f32),
    ZoomY(f32),
    AddZoomX(f32),
    AddZoomY(f32),

    // NEW: helpers that set one axis and preserve aspect (sprites immediate; text via layout)
    ZoomToWidth(f32),
    ZoomToHeight(f32),

    Visible(bool),
    FlipX(bool),
    FlipY(bool),

    CropLeft(f32),
    CropRight(f32),
    CropTop(f32),
    CropBottom(f32),

    Cell(u32, u32),
    Grid(u32, u32),
    UvRect([f32; 4]),
    TexVel([f32; 2]),

    // text
    Px(f32),
    Font(&'static str),
    Content(Cow<'a, str>),
    TAlign(TextAlign),

    // runtime/tween plumbing
    Tween(&'a [anim::Step]),
    SiteId(u64),
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
    let mut blend = BlendMode::Alpha;
    let mut rot = 0.0_f32;
    let mut cell: Option<(u32, u32)> = None;
    let mut grid: Option<(u32, u32)> = None;
    let mut uv: Option<[f32; 4]> = None;
    let mut texv: Option<[f32; 2]> = None;
    let (mut tw, mut site_extra): (Option<&[anim::Step]>, u64) = (None, 0);

    // StepMania zoom (scale)
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
            Mod::Tint(r) => { tint = *r; }
            Mod::Alpha(a) => { tint[3] = *a; }
            Mod::RotationZ(r) => { rot = *r; }
            Mod::Blend(bm) => { blend = *bm; }

            // absolute size (pre-scale)
            Mod::SizePx(a, b) => { w = *a; h = *b; }

            // StepMania zoom (scale) — post-size
            Mod::Zoom(f) => { sx = *f; sy = *f; }
            Mod::ZoomX(a) => { sx = *a; }
            Mod::ZoomY(b) => { sy = *b; }
            Mod::AddZoomX(a) => { sx += *a; }
            Mod::AddZoomY(b) => { sy += *b; }

            // NEW: keep aspect: change one axis of base size
            Mod::ZoomToWidth(new_w) => {
                if w > 0.0 && h > 0.0 {
                    let aspect = h / w;
                    w = *new_w;
                    h = w * aspect;
                } else {
                    // unknown aspect: best effort
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

            Mod::Visible(v) => { vis = *v; }
            Mod::FlipX(v) => { fx = *v; }
            Mod::FlipY(v) => { fy = *v; }

            Mod::CropLeft(v) => { cl = *v; }
            Mod::CropRight(v) => { cr = *v; }
            Mod::CropTop(v) => { ct = *v; }
            Mod::CropBottom(v) => { cb = *v; }

            Mod::Cell(c, r) => { cell = Some((*c, *r)); }
            Mod::Grid(c, r) => { grid = Some((*c, *r)); }
            Mod::UvRect(u) => { uv = Some(*u); }
            Mod::TexVel(v) => { texv = Some(*v); }

            Mod::Px(_) | Mod::Font(_) | Mod::Content(_) | Mod::TAlign(_) => {}
            Mod::Tween(steps) => { tw = Some(steps); }
            Mod::SiteId(id) => { site_extra = *id; }
        }
    }

    // tween (optional)
    if let Some(steps) = tw {
        let mut init = anim::TweenState::default();
        init.x = x; init.y = y; init.w = w; init.h = h;
        init.hx = hx; init.vy = vy;
        init.tint = tint;
        init.visible = vis; init.flip_x = fx; init.flip_y = fy;

        let sid = runtime::site_id(file, line, col, site_extra);
        let s = runtime::materialize(sid, init, steps);

        x = s.x; y = s.y; w = s.w; h = s.h;
        hx = s.hx; vy = s.vy;
        tint = s.tint; vis = s.visible; fx = s.flip_x; fy = s.flip_y;
    }

    // apply scale last
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
        blend,
        rot_z_deg: rot,
        texcoordvelocity: texv,
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
    let mut px = 16.0_f32;
    let mut color = [1.0, 1.0, 1.0, 1.0];
    let mut font: &'static str = "miso";
    let mut content: Cow<'a, str> = Cow::Borrowed("");
    let mut talign = TextAlign::Left;
    let mut z: i16 = 0;

    // zoom + optional fit targets
    let (mut sx, mut sy) = (1.0_f32, 1.0_f32);
    let (mut fit_w, mut fit_h): (Option<f32>, Option<f32>) = (None, None);

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

            Mod::Px(p)       => { px = *p; }
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

            // capture fit targets (applied at layout with actual text metrics)
            Mod::ZoomToWidth(w)  => { fit_w = Some(*w); }
            Mod::ZoomToHeight(h) => { fit_h = Some(*h); }

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
            "talign expects left|center|right, got: ",
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
        let mut __site: u64 = 0;
        $crate::__dsl_apply!( ($($tail)+) __mods __tw __cur __site );
        if let ::core::option::Option::Some(seg)=__cur.take(){__tw.push(seg.build());}
        if !__tw.is_empty(){ __mods.push($crate::ui::dsl::Mod::Tween(&__tw)); }
        if __site!=0{ __mods.push($crate::ui::dsl::Mod::SiteId(__site)); }
        $crate::ui::dsl::sprite($tex, &__mods, file!(), line!(), column!())
    }};
    (quad: $($tail:tt)+) => {{
        let mut __mods = ::std::vec::Vec::new();
        let mut __tw = ::std::vec::Vec::new();
        let mut __cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None;
        let mut __site: u64 = 0;
        $crate::__dsl_apply!( ($($tail)+) __mods __tw __cur __site );
        if let ::core::option::Option::Some(seg)=__cur.take(){__tw.push(seg.build());}
        if !__tw.is_empty(){ __mods.push($crate::ui::dsl::Mod::Tween(&__tw)); }
        if __site!=0{ __mods.push($crate::ui::dsl::Mod::SiteId(__site)); }
        $crate::ui::dsl::quad(&__mods, file!(), line!(), column!())
    }};
    (text: $($tail:tt)+) => {{
        let mut __mods = ::std::vec::Vec::new();
        let mut __tw: ::std::vec::Vec<$crate::ui::anim::Step> = ::std::vec::Vec::new(); // <-- typed
        let mut __cur: ::core::option::Option<$crate::ui::anim::SegmentBuilder> = None; // ignored
        let mut __site: u64 = 0; // ignored
        $crate::__dsl_apply!( ($($tail)+) __mods __tw __cur __site );
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
    (id ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $site = ($v) as u64; }};

    // segments
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

    // tweenable props
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

    // Absolute size (zoomto/setsize) — uses dedicated tween op now
    (zoomto ($w:expr, $h:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.size(($w) as f32, ($h) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::SizePx(($w) as f32, ($h) as f32)); }
    }};
    (setsize ($w:expr, $h:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $crate::__dsl_apply_one!(zoomto(($w), ($h)) $mods $tw $cur $site)
    }};

    (diffuse ($r:expr,$g:expr,$b:expr,$a:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.diffuse(($r) as f32,($g) as f32,($b) as f32,($a) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Tint([($r) as f32,($g) as f32,($b) as f32,($a) as f32])); }
    }};
    (alpha ($a:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.alpha(($a) as f32); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Alpha(($a) as f32)); }
    }};
    (diffusealpha ($a:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $crate::__dsl_apply_one!(alpha(($a)) $mods $tw $cur $site) }};

    (set_visible ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.set_visible(($v) as bool); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::Visible(($v) as bool)); }
    }};
    (visible ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $crate::__dsl_apply_one!(set_visible(($v)) $mods $tw $cur $site) }};
    (flipx ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.flip_x(($v) as bool); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FlipX(($v) as bool)); }
    }};
    (flipy ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        if let ::core::option::Option::Some(mut seg)=$cur.take(){ seg=seg.flip_y(($v) as bool); $cur=::core::option::Option::Some(seg); }
        else { $mods.push($crate::ui::dsl::Mod::FlipY(($v) as bool)); }
    }};

    // --- absolute size helpers that preserve aspect ---------------------
    (zoomtowidth ($w:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::ZoomToWidth(($w) as f32));
    }};
    (zoomtoheight ($h:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::ZoomToHeight(($h) as f32));
    }};

    // static sprite bits / cropping / uv / blend / rotation ---------------
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
    (cell ($c:expr,$r:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Cell(($c) as u32, ($r) as u32)); }};
    (setstate ($i:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Cell(($i) as u32, u32::MAX)); }};
    (grid ($c:expr,$r:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Grid(($c) as u32, ($r) as u32)); }};
    (texrect ($u0:expr,$v0:expr,$u1:expr,$v1:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::UvRect([($u0) as f32,($v0) as f32,($u1) as f32,($v1) as f32]));
    }};
    (customtexturerect ($u0:expr,$v0:expr,$u1:expr,$v1:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $crate::__dsl_apply_one!(texrect(($u0),($v0),($u1),($v1)) $mods $tw $cur $site)
    }};
    (texcoordvelocity ($vx:expr,$vy:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TexVel([($vx) as f32, ($vy) as f32]));
    }};
    (cropleft ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::CropLeft(($v) as f32)); }};
    (cropright ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::CropRight(($v) as f32)); }};
    (croptop ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::CropTop(($v) as f32)); }};
    (cropbottom ($v:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::CropBottom(($v) as f32)); }};
    (blend (alpha) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Alpha)); }};
    (blend (normal) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Alpha)); }};
    (blend (add) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Add)); }};
    (blend (additive) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $crate::__dsl_apply_one!(blend(add) $mods $tw $cur $site) }};
    (blend (multiply) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Blend($crate::core::gfx::BlendMode::Multiply)); }};
    (rotation ($z:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::RotationZ(($z) as f32)); }};
    (rotationz ($z:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $crate::__dsl_apply_one!(rotation(($z)) $mods $tw $cur $site) }};

    // Text properties (SM-compatible only)
    (px ($p:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Px(($p) as f32)); }};
    (font ($n:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{ $mods.push($crate::ui::dsl::Mod::Font($n)); }};
    (settext ($s:expr) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::Content(::std::borrow::Cow::from(($s))));
    }};
    (talign ($dir:ident) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TAlign($crate::__ui_textalign_from_ident!($dir)));
    }};
    (horizalign ($dir:ident) $mods:ident $tw:ident $cur:ident $site:ident) => {{
        $mods.push($crate::ui::dsl::Mod::TAlign($crate::__ui_textalign_from_ident!($dir)));
    }};

    ($other:ident ( $($args:expr),* ) $mods:ident $tw:ident $cur:ident $site:ident) => {
        compile_error!(concat!("act!: unknown command: ", stringify!($other)));
    };
}

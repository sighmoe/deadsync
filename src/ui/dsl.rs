use crate::core::gfx::types::BlendMode;
use crate::ui::actors::{SizeSpec, SpriteSource, Actor, TextAlign};

#[inline(always)]
pub fn finish_sprite(
    texture: &'static str,
    x: f32, y: f32, w: f32, h: f32,
    hx: f32, vy: f32,
    tint: [f32; 4],
    z: i16,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    uv_rect: Option<[f32; 4]>,
    visible: bool, flip_x: bool, flip_y: bool,
    cropleft: f32, cropright: f32, croptop: f32, cropbottom: f32,
    blend: BlendMode,
    rot_z_deg: f32,
) -> Actor {
    Actor::Sprite {
        align: [hx, vy],
        offset: [x, y],
        size:   [SizeSpec::Px(w), SizeSpec::Px(h)],
        source: SpriteSource::Texture(texture),
        tint,
        z,
        cell,
        grid,
        uv_rect,
        visible,
        flip_x,
        flip_y,
        cropleft,
        cropright,
        croptop,
        cropbottom,
        blend,
        rot_z_deg,
    }
}

#[inline(always)]
pub fn finish_quad(
    x: f32, y: f32, w: f32, h: f32,
    hx: f32, vy: f32,
    tint: [f32; 4],
    z: i16,
    visible: bool, flip_x: bool, flip_y: bool,
    cropleft: f32, cropright: f32, croptop: f32, cropbottom: f32,
    blend: BlendMode,
    rot_z_deg: f32,
) -> Actor {
    Actor::Sprite {
        align: [hx, vy],
        offset: [x, y],
        size:   [SizeSpec::Px(w), SizeSpec::Px(h)],
        source: SpriteSource::Solid,
        tint,
        z,
        cell: None,
        grid: None,
        uv_rect: None,
        visible,
        flip_x,
        flip_y,
        cropleft,
        cropright,
        croptop,
        cropbottom,
        blend,
        rot_z_deg,
    }
}

#[inline(always)]
pub fn finish_text(
    text: String,
    x: f32, y: f32,
    hx: f32, vy: f32,
    px: f32,
    color: [f32; 4],
    font: &'static str,
    align: TextAlign,
    z: i16,
) -> Actor {
    Actor::Text {
        align: [hx, vy],
        offset: [x, y],
        px,
        color,
        font,
        content: text,
        align_text: align,
        z,
    }
}

/* ========================== PUBLIC MACRO ========================== */

#[macro_export]
macro_rules! act {
    (sprite($tex:expr): $($tail:tt)+) => {{
        #[allow(unused_assignments)]
        {
            use $crate::core::gfx::types::BlendMode;
            use $crate::ui::anim as __anim;
            let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            let (mut hx, mut vy) = (0.5f32, 0.5f32);
            let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
            let mut z: i16 = 0;
            let mut cell: Option<(u32,u32)> = None;
            let mut grid: Option<(u32,u32)> = None;
            let mut uv_rect: Option<[f32;4]> = None;
            let mut visible: bool = true;
            let mut flip_x: bool = false;
            let mut flip_y: bool = false;
            let mut cropleft: f32 = 0.0;
            let mut cropright: f32 = 0.0;
            let mut croptop: f32 = 0.0;
            let mut cropbottom: f32 = 0.0;
            let mut blend: BlendMode = BlendMode::Alpha;
            let mut rot_z_deg: f32 = 0.0;

            // Inline tween bookkeeping
            let mut __tw_steps: Vec<__anim::Step> = Vec::new();
            let mut __tw_cur: Option<__anim::SegmentBuilder> = None;
            let mut __site_extra: u64 = 0;

            $crate::__ui_act_apply!( ($($tail)+)
                x y w h hx vy tint z cell grid uv_rect visible flip_x flip_y
                cropleft cropright croptop cropbottom blend rot_z_deg
                __tw_steps __tw_cur __site_extra
            );

            // close any open segment
            if let Some(seg) = __tw_cur.take() {
                __tw_steps.push(seg.build());
            }

            if !__tw_steps.is_empty() {
                // Evaluate tween at this frame
                let mut __init = $crate::ui::anim::TweenState::default();
                __init.x = x; __init.y = y; __init.w = w; __init.h = h;
                __init.hx = hx; __init.vy = vy;
                __init.tint = tint;
                __init.visible = visible;
                __init.flip_x = flip_x; __init.flip_y = flip_y;

                let __id = $crate::ui::runtime::site_id(file!(), line!(), column!(), __site_extra);
                let __s = $crate::ui::runtime::materialize(__id, __init, &__tw_steps);

                x = __s.x; y = __s.y; w = __s.w; h = __s.h;
                hx = __s.hx; vy = __s.vy;
                tint = __s.tint;
                visible = __s.visible;
                flip_x = __s.flip_x; flip_y = __s.flip_y;
            }

            $crate::ui::dsl::finish_sprite(
                $tex, x,y,w,h,hx,vy,tint,z,cell,grid,uv_rect,visible,flip_x,flip_y,
                cropleft,cropright,croptop,cropbottom,blend,rot_z_deg
            )
        }
    }};
    (quad: $($tail:tt)+) => {{
        #[allow(unused_assignments)]
        {
            use $crate::core::gfx::types::BlendMode;
            use $crate::ui::anim as __anim;
            let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            let (mut hx, mut vy) = (0.5f32, 0.5f32);
            let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
            let mut z: i16 = 0;
            let mut visible: bool = true;
            let mut flip_x: bool = false;
            let mut flip_y: bool = false;
            let mut cropleft: f32 = 0.0;
            let mut cropright: f32 = 0.0;
            let mut croptop: f32 = 0.0;
            let mut cropbottom: f32 = 0.0;
            let mut blend: BlendMode = BlendMode::Alpha;
            let mut rot_z_deg: f32 = 0.0;

            // Inline tween bookkeeping
            let mut __tw_steps: Vec<__anim::Step> = Vec::new();
            let mut __tw_cur: Option<__anim::SegmentBuilder> = None;
            let mut __site_extra: u64 = 0;

            $crate::__ui_act_apply!( ($($tail)+)
                x y w h hx vy tint z __skip_cell __skip_grid __skip_uv_rect visible flip_x flip_y
                cropleft cropright croptop cropbottom blend rot_z_deg
                __tw_steps __tw_cur __site_extra
            );

            if let Some(seg) = __tw_cur.take() { __tw_steps.push(seg.build()); }

            if !__tw_steps.is_empty() {
                let mut __init = $crate::ui::anim::TweenState::default();
                __init.x = x; __init.y = y; __init.w = w; __init.h = h;
                __init.hx = hx; __init.vy = vy;
                __init.tint = tint;
                __init.visible = visible;
                __init.flip_x = flip_x; __init.flip_y = flip_y;

                let __id = $crate::ui::runtime::site_id(file!(), line!(), column!(), __site_extra);
                let __s = $crate::ui::runtime::materialize(__id, __init, &__tw_steps);

                x = __s.x; y = __s.y; w = __s.w; h = __s.h;
                hx = __s.hx; vy = __s.vy;
                tint = __s.tint;
                visible = __s.visible;
                flip_x = __s.flip_x; flip_y = __s.flip_y;
            }

            $crate::ui::dsl::finish_quad(
                x,y,w,h,hx,vy,tint,z,visible,flip_x,flip_y,
                cropleft,cropright,croptop,cropbottom,blend,rot_z_deg
            )
        }
    }};
    (text: $($tail:tt)+) => {{
        #[allow(unused_assignments)]
        {
            let (mut x, mut y) = (0.0f32, 0.0f32);
            let (mut hx, mut vy) = (0.5f32, 0.5f32);
            let mut px: f32 = 16.0;
            let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
            let mut font: &'static str = "miso";
            let mut content: String = String::new();
            let mut talign = $crate::ui::actors::TextAlign::Left;
            let mut z: i16 = 0;

            $crate::__ui_act_apply_text!( ($($tail)+)
                x y hx vy px tint font content talign z
            );

            $crate::ui::dsl::finish_text(content, x, y, hx, vy, px, tint, font, talign, z)
        }
    }};
}

/* =================== INTERNAL MUNCHERS (SPRITE/QUAD) =================== */

#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply {
    ( () $($vars:ident)+ ) => { () };

    ( ($cmd:ident ( $($args:expr),* ) : $($rest:tt)* )
      $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
      $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
      $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
      $rot_z_deg:ident
      $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $crate::__ui_act_apply_one!{
            $cmd ( $($args),* )
            $x $y $w $h $hx $vy $tint $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend $rot_z_deg
            $__tw_steps $__tw_cur $__site_extra
        }
        $crate::__ui_act_apply!( ($($rest)*) $x $y $w $h $hx $vy $tint 
            $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend $rot_z_deg
            $__tw_steps $__tw_cur $__site_extra
        );
    }};

    ( ($cmd:ident ( $($args:expr),* ) )
      $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
      $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
      $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
      $rot_z_deg:ident
      $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $crate::__ui_act_apply_one!{
            $cmd ( $($args),* )
            $x $y $w $h $hx $vy $tint $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend $rot_z_deg
            $__tw_steps $__tw_cur $__site_extra
        }
        $crate::__ui_act_apply!( () $x $y $w $h $hx $vy $tint $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend $rot_z_deg
            $__tw_steps $__tw_cur $__site_extra
        );
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply_one {
    // ---- meta / id ----
    (id ($v:expr)
        $($rest:tt)*
    ) => {{
        let v: u64 = ($v) as u64;
        #[allow(unused_variables)] { let (_,$_, $___) = (&v, &v, &v); }
        // trailing idents
        let (_x,_y,_w,_h,_hx,_vy,_tint,_z,_cell,_grid,_uv_rect,_visible,_flip_x,_flip_y,_cropleft,_cropright,_croptop,_cropbottom,_blend,_rot_z_deg,$__tw_steps,$__tw_cur,$__site_extra) = $($rest)*;
        $__site_extra = v;
    }};

    // ---- time segment starts ----
    (linear ($d:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(seg) = $__tw_cur.take() { $__tw_steps.push(seg.build()); }
        $__tw_cur = Some($crate::ui::anim::linear(($d) as f32));
    }};
    (accelerate ($d:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(seg) = $__tw_cur.take() { $__tw_steps.push(seg.build()); }
        $__tw_cur = Some($crate::ui::anim::accelerate(($d) as f32));
    }};
    (decelerate ($d:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(seg) = $__tw_cur.take() { $__tw_steps.push(seg.build()); }
        $__tw_cur = Some($crate::ui::anim::decelerate(($d) as f32));
    }};
    (sleep ($d:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(seg) = $__tw_cur.take() { $__tw_steps.push(seg.build()); }
        $__tw_steps.push($crate::ui::anim::sleep(($d) as f32));
    }};

    // ---- property commands: if in a segment, record; else assign ----

    (xy ($xv:expr, $yv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() {
            seg = seg.xy(($xv) as f32, ($yv) as f32);
            $__tw_cur = Some(seg);
        } else {
            $x = ($xv) as f32; $y = ($yv) as f32;
        }
    }};

    (x ($xv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.x(($xv) as f32); $__tw_cur = Some(seg); }
        else { $x = ($xv) as f32; }
    }};

    (y ($yv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.y(($yv) as f32); $__tw_cur = Some(seg); }
        else { $y = ($yv) as f32; }
    }};

    (addx ($dx:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.addx(($dx) as f32); $__tw_cur = Some(seg); }
        else { $x += ($dx) as f32; }
    }};

    (addy ($dy:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.addy(($dy) as f32); $__tw_cur = Some(seg); }
        else { $y += ($dy) as f32; }
    }};

    (zoomto ($nw:expr, $nh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.zoom(($nw) as f32, ($nh) as f32); $__tw_cur = Some(seg); }
        else { $w = ($nw) as f32; $h = ($nh) as f32; }
    }};

    (setsize ($nw:expr, $nh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.zoom(($nw) as f32, ($nh) as f32); $__tw_cur = Some(seg); }
        else { $w = ($nw) as f32; $h = ($nh) as f32; }
    }};

    (zoomx ($nw:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.zoomx(($nw) as f32); $__tw_cur = Some(seg); }
        else { $w = ($nw) as f32; }
    }};

    (zoomy ($nh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.zoomy(($nh) as f32); $__tw_cur = Some(seg); }
        else { $h = ($nh) as f32; }
    }};

    (addzoomx ($dw:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.addzoomx(($dw) as f32); $__tw_cur = Some(seg); }
        else { $w += ($dw) as f32; }
    }};

    (addzoomy ($dh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.addzoomy(($dh) as f32); $__tw_cur = Some(seg); }
        else { $h += ($dh) as f32; }
    }};

    (diffuse ($r:expr, $g:expr, $b:expr, $a:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() {
            seg = seg.diffuse(($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32);
            $__tw_cur = Some(seg);
        } else {
            $tint = [($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32];
        }
    }};

    (diffusealpha ($a:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.alpha(($a) as f32); $__tw_cur = Some(seg); }
        else { $tint[3] = ($a) as f32; }
    }};

    (alpha ($a:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.alpha(($a) as f32); $__tw_cur = Some(seg); }
        else { $tint[3] = ($a) as f32; }
    }};

    (set_visible ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.set_visible(($v) as bool); $__tw_cur = Some(seg); }
        else { $visible = ($v) as bool; }
    }};

    (flipx ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.flip_x(($v) as bool); $__tw_cur = Some(seg); }
        else { $flip_x = ($v) as bool; }
    }};

    (flipy ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        if let Some(mut seg) = $__tw_cur.take() { seg = seg.flip_y(($v) as bool); $__tw_cur = Some(seg); }
        else { $flip_y = ($v) as bool; }
    }};

    // ---- non-animated/static (align, z, uv/crop, blend, rotation, cell/grid/uv) ----

    (align ($hv:expr, $vv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $hx = ($hv) as f32; $vy = ($vv) as f32;
    }};

    (z ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $z = ($v) as i16;
    }};

    (cell ($c:expr, $r:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $cell = Some((($c) as u32, ($r) as u32));
    }};

    (setstate ($i:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $cell = Some((($i) as u32, u32::MAX));
    }};

    (grid ($c:expr, $r:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $grid = Some((($c) as u32, ($r) as u32));
    }};

    (texrect ($u0:expr, $v0:expr, $u1:expr, $v1:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $uv_rect = Some([($u0) as f32, ($v0) as f32, ($u1) as f32, ($v1) as f32]);
    }};

    (visible ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{
        $visible = ($v) as bool;
    }};

    (cropleft ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $cropleft = ($v) as f32; }};

    (cropright ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $cropright = ($v) as f32; }};

    (croptop ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $croptop = ($v) as f32; }};

    (cropbottom ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $cropbottom = ($v) as f32; }};

    (blend (alpha)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $blend = $crate::core::gfx::types::BlendMode::Alpha; }};

    (blend (normal)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $blend = $crate::core::gfx::types::BlendMode::Alpha; }};

    (blend (add)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $blend = $crate::core::gfx::types::BlendMode::Add; }};

    (blend (additive)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $blend = $crate::core::gfx::types::BlendMode::Add; }};

    (blend (multiply)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $blend = $crate::core::gfx::types::BlendMode::Multiply; }};

    (rotation ($zv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $rot_z_deg = ($zv) as f32; }};

    (rotationz ($zv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
        $rot_z_deg:ident
        $__tw_steps:ident $__tw_cur:ident $__site_extra:ident
    ) => {{ $rot_z_deg = ($zv) as f32; }};


    // ---- unknown ----
    ($other:ident ( $($args:expr),* )
        $($rest:tt)*
    ) => {
        compile_error!(concat!("act!: unknown command: ", stringify!($other)));
    };
}

/* =================== TEXT MUNCHERS (unchanged) =================== */

#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply_text {
    ( () $($vars:ident)+ ) => { () };
    ( ($cmd:ident $args:tt : $($rest:tt)* )
      $x:ident $y:ident $hx:ident $vy:ident $px:ident
      $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $crate::__ui_act_apply_one_text!{
            $cmd $args
            $x $y $hx $vy $px $tint $font $content $talign $z
        }
        $crate::__ui_act_apply_text!( ($($rest)*) $x $y $hx $vy $px $tint $font $content $talign $z );
    }};
    ( ($cmd:ident $args:tt )
      $x:ident $y:ident $hx:ident $vy:ident $px:ident
      $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $crate::__ui_act_apply_one_text!{
            $cmd $args
            $x $y $hx $vy $px $tint $font $content $talign $z
        }
        $crate::__ui_act_apply_text!( () $x $y $hx $vy $px $tint $font $content $talign $z );
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply_one_text {
    (xy ($xv:expr, $yv:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $x = ($xv) as f32; $y = ($yv) as f32;
    }};
    (x ($xv:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $x = ($xv) as f32;
    }};
    (y ($yv:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $y = ($yv) as f32;
    }};
    (align ($hv:expr, $vv:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $hx = ($hv) as f32; $vy = ($vv) as f32;
    }};
    (px ($s:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $px = ($s) as f32;
    }};
    (diffuse ($r:expr, $g:expr, $b:expr, $a:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $tint = [($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32];
    }};
    (diffusealpha ($a:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $tint[3] = ($a) as f32;
    }};
    (font ($name:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $font = $name;
    }};
    (text ($txt:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $content = ($txt).into();
    }};
    (talign ($dir:ident) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $align_mode:ident $z:ident) => {{
        $align_mode = $crate::__ui_textalign_from_ident!($dir);
    }};
    (z ($v:expr) $x:ident $y:ident $hx:ident $vy:ident $px:ident $tint:ident $font:ident $content:ident $talign:ident $z:ident) => {{
        $z = ($v) as i16;
    }};
    ($other:ident $($anything:tt)* ) => {
        compile_error!(concat!("act!(text): unknown command: ", stringify!($other)));
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ui_textalign_from_ident {
    (left)   => { $crate::ui::actors::TextAlign::Left };
    (center) => { $crate::ui::actors::TextAlign::Center };
    (right)  => { $crate::ui::actors::TextAlign::Right };
    ($other:ident) => {
        compile_error!(concat!("act!(text): talign expects left|center|right, got: ", stringify!($other)));
    };
}

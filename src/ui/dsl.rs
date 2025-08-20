// src/ui/dsl.rs
use crate::core::gfx::types::BlendMode;
use crate::ui::actors::{Actor, Anchor, SizeSpec, SpriteSource};

#[inline(always)]
fn snap_align(v: f32) -> f32 {
    if v <= 0.25 { 0.0 } else if v >= 0.75 { 1.0 } else { 0.5 }
}

#[inline(always)]
fn anchor_from_factors(hx: f32, vy: f32) -> Anchor {
    match (snap_align(hx), snap_align(vy)) {
        (0.0, 0.0) => Anchor::TopLeft,
        (0.5, 0.0) => Anchor::TopCenter,
        (1.0, 0.0) => Anchor::TopRight,
        (0.0, 0.5) => Anchor::CenterLeft,
        (0.5, 0.5) => Anchor::Center,
        (1.0, 0.5) => Anchor::CenterRight,
        (0.0, 1.0) => Anchor::BottomLeft,
        (0.5, 1.0) => Anchor::BottomCenter,
        _          => Anchor::BottomRight,
    }
}

#[inline(always)]
pub fn finish_sprite(
    texture: &'static str,
    x: f32, y: f32, w: f32, h: f32,
    hx: f32, vy: f32,
    tint: [f32; 4],
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    uv_rect: Option<[f32; 4]>,
    visible: bool, flip_x: bool, flip_y: bool,
    cropleft: f32, cropright: f32, croptop: f32, cropbottom: f32,
    blend: BlendMode,
) -> Actor {
    Actor::Sprite {
        anchor: anchor_from_factors(hx, vy),
        offset: [x, y],
        size:   [SizeSpec::Px(w), SizeSpec::Px(h)],
        source: SpriteSource::Texture(texture),
        tint,
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
    }
}

#[inline(always)]
pub fn finish_quad(
    x: f32, y: f32, w: f32, h: f32,
    hx: f32, vy: f32,
    tint: [f32; 4],
    visible: bool, flip_x: bool, flip_y: bool,
    cropleft: f32, cropright: f32, croptop: f32, cropbottom: f32,
    blend: BlendMode,
) -> Actor {
    Actor::Sprite {
        anchor: anchor_from_factors(hx, vy),
        offset: [x, y],
        size:   [SizeSpec::Px(w), SizeSpec::Px(h)],
        source: SpriteSource::Solid,
        tint,
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
    }
}

/// Public macro: `act!(sprite("tex.png"): align(...): xy(...): zoomto(...): diffuse(...))`
///                `act!(quad: align(...): xy(...): zoomto(...): diffuse(...))`
#[macro_export]
macro_rules! act {
    (sprite($tex:expr): $($tail:tt)+) => {{
        use $crate::core::gfx::types::BlendMode;
        let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let (mut hx, mut vy) = (0.5f32, 0.5f32);
        let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
        let mut cell: Option<(u32,u32)> = None;
        let grid: Option<(u32,u32)> = None;
        let uv_rect: Option<[f32;4]> = None;
        let visible: bool = true;
        let flip_x: bool = false;
        let flip_y: bool = false;
        let cropleft: f32 = 0.0;
        let cropright: f32 = 0.0;
        let croptop: f32 = 0.0;
        let cropbottom: f32 = 0.0;
        let blend: BlendMode = BlendMode::Alpha;

        $crate::__ui_act_apply!( ($($tail)+)
            x y w h hx vy tint cell grid uv_rect visible flip_x flip_y
            cropleft cropright croptop cropbottom blend
        );

        $crate::ui::dsl::finish_sprite(
            $tex, x,y,w,h,hx,vy,tint,cell,grid,uv_rect,visible,flip_x,flip_y,
            cropleft,cropright,croptop,cropbottom,blend
        )
    }};
    (quad: $($tail:tt)+) => {{
        use $crate::core::gfx::types::BlendMode;
        let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let (mut hx, mut vy) = (0.5f32, 0.5f32);
        let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
        let visible: bool = true;
        let flip_x: bool = false;
        let flip_y: bool = false;
        let cropleft: f32 = 0.0;
        let cropright: f32 = 0.0;
        let croptop: f32 = 0.0;
        let cropbottom: f32 = 0.0;
        let blend: BlendMode = BlendMode::Alpha;

        // For quads we ignore cell/grid/uv_rect, but the parser still expects placeholders.
        $crate::__ui_act_apply!( ($($tail)+)
            x y w h hx vy tint __skip_cell __skip_grid __skip_uv_rect visible flip_x flip_y
            cropleft cropright croptop cropbottom blend
        );

        $crate::ui::dsl::finish_quad(
            x,y,w,h,hx,vy,tint,visible,flip_x,flip_y,cropleft,cropright,croptop,cropbottom,blend
        )
    }};
}

/// Internal: command list muncher (`cmd(args): cmd2(...): ...`)
#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply {
    // end of list — return an expression so it's valid in expression position
    ( () $($vars:ident)+ ) => { () };

    // consume one `cmd(args):` then recurse for more
    ( ($cmd:ident ( $($args:expr),* ) : $($rest:tt)* )
      $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
      $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
      $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $crate::__ui_act_apply_one!{
            $cmd ( $($args),* )
            $x $y $w $h $hx $vy $tint $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend
        }
        // ← make the recursive call a statement
        $crate::__ui_act_apply!( ($($rest)*) $x $y $w $h $hx $vy $tint $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend );
    }};

    // final `cmd(args)` with NO trailing colon
    ( ($cmd:ident ( $($args:expr),* ) )
      $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
      $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
      $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $crate::__ui_act_apply_one!{
            $cmd ( $($args),* )
            $x $y $w $h $hx $vy $tint $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend
        }
        // ← same here
        $crate::__ui_act_apply!( () $x $y $w $h $hx $vy $tint $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend );
    }};
}

/// Internal: single-command handlers.
#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply_one {
    (xy ($xv:expr, $yv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $x = ($xv) as f32; $y = ($yv) as f32;
    }};
    (align ($hv:expr, $vv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $hx = ($hv) as f32; $vy = ($vv) as f32;
    }};
    (zoomto ($nw:expr, $nh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $w = ($nw) as f32; $h = ($nh) as f32;
    }};
    (diffuse ($r:expr, $g:expr, $b:expr, $a:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $tint = [($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32];
    }};
    (cell ($c:expr, $r:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $cell = Some((($c) as u32, ($r) as u32));
    }};
    (visible ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $visible = ($v) as bool;
    }};
    (blend (alpha)
        $($rest:tt)*
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Alpha;
    }};
    (blend (add)
        $($rest:tt)*
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Add;
    }};
    (blend (multiply)
        $($rest:tt)*
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Multiply;
    }};
    // Friendly error for unknown commands
    ($other:ident ( $($args:expr),* )
        $($rest:tt)*
    ) => {
        compile_error!(concat!("act!: unknown command: ", stringify!($other)));
    };
}

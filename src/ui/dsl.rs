use crate::core::gfx::types::BlendMode;
use crate::ui::actors::SizeSpec;
use crate::ui::actors::SpriteSource;
use crate::ui::actors::Actor;
use crate::ui::actors::TextAlign;

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

/// Public macro: `act!(sprite("tex.png"): align(...): xy(...): zoomto(...): diffuse(...))`
///                `act!(quad: align(...): xy(...): zoomto(...): diffuse(...))`

#[macro_export]
macro_rules! act {
    (sprite($tex:expr): $($tail:tt)+) => {{
        #[allow(unused_assignments)]
        {
            use $crate::core::gfx::types::BlendMode;
            let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            let (mut hx, mut vy) = (0.5f32, 0.5f32);
            let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
            let z: i16 = 0;
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
                x y w h hx vy tint z cell grid uv_rect visible flip_x flip_y
                cropleft cropright croptop cropbottom blend
            );

            $crate::ui::dsl::finish_sprite(
                $tex, x,y,w,h,hx,vy,tint,z,cell,grid,uv_rect,visible,flip_x,flip_y,
                cropleft,cropright,croptop,cropbottom,blend
            )
        }
    }};
    (quad: $($tail:tt)+) => {{
        #[allow(unused_assignments)]
        {
            use $crate::core::gfx::types::BlendMode;
            let (mut x, mut y, mut w, mut h) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            let (mut hx, mut vy) = (0.5f32, 0.5f32);
            let mut tint: [f32;4] = [1.0, 1.0, 1.0, 1.0];
            let z: i16 = 0;
            let visible: bool = true;
            let flip_x: bool = false;
            let flip_y: bool = false;
            let cropleft: f32 = 0.0;
            let cropright: f32 = 0.0;
            let croptop: f32 = 0.0;
            let cropbottom: f32 = 0.0;
            let blend: BlendMode = BlendMode::Alpha;

            $crate::__ui_act_apply!( ($($tail)+)
                x y w h hx vy tint z __skip_cell __skip_grid __skip_uv_rect visible flip_x flip_y
                cropleft cropright croptop cropbottom blend
            );

            $crate::ui::dsl::finish_quad(
                x,y,w,h,hx,vy,tint,z,visible,flip_x,flip_y,cropleft,cropright,croptop,cropbottom,blend
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
            let z: i16 = 0;

            $crate::__ui_act_apply_text!( ($($tail)+)
                x y hx vy px tint font content talign z
            );

            $crate::ui::dsl::finish_text(content, x, y, hx, vy, px, tint, font, talign, z)
        }
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
      $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
      $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $crate::__ui_act_apply_one!{
            $cmd ( $($args),* )
            $x $y $w $h $hx $vy $tint $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend
        }
        // ← make the recursive call a statement
        $crate::__ui_act_apply!( ($($rest)*) $x $y $w $h $hx $vy $tint 
            $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend );
    }};

    // final `cmd(args)` with NO trailing colon
    ( ($cmd:ident ( $($args:expr),* ) )
      $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
      $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
      $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $crate::__ui_act_apply_one!{
            $cmd ( $($args),* )
            $x $y $w $h $hx $vy $tint $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend
        }
        // ← same here
        $crate::__ui_act_apply!( () $x $y $w $h $hx $vy $tint $z $cell $grid $uv_rect $visible $flip_x $flip_y
            $cropleft $cropright $croptop $cropbottom $blend );
    }};
}

/// Internal: single-command handlers.
#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply_one {
    (xy ($xv:expr, $yv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $x = ($xv) as f32; $y = ($yv) as f32;
    }};
    (x ($xv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $x = ($xv) as f32;
    }};
    (y ($yv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $y = ($yv) as f32;
    }};
    (align ($hv:expr, $vv:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $hx = ($hv) as f32; $vy = ($vv) as f32;
    }};
    (zoomto ($nw:expr, $nh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $w = ($nw) as f32; $h = ($nh) as f32;
    }};
    // alias for SM-like naming (absolute size)
    (setsize ($nw:expr, $nh:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $w = ($nw) as f32; $h = ($nh) as f32;
    }};

    (diffuse ($r:expr, $g:expr, $b:expr, $a:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $tint = [($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32];
    }};
    (diffusealpha ($a:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $tint[3] = ($a) as f32;
    }};

    (z ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $z = ($v) as i16;
    }};

    (cell ($c:expr, $r:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $cell = Some((($c) as u32, ($r) as u32));
    }};
    // linear index -> (col,row) is resolved later in layout using filename/grid
    (setstate ($i:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $cell = Some((($i) as u32, u32::MAX));
    }};
    (grid ($c:expr, $r:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $grid = Some((($c) as u32, ($r) as u32));
    }};

    (visible ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $visible = ($v) as bool;
    }};

    // cropping (fractions 0..1)
    (cropleft   ($v:expr) $($rest:tt)* ) => {{ $($rest)*; }};
    (cropleft   ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{ $cropleft = ($v) as f32; }};
    (cropright  ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{ $cropright = ($v) as f32; }};
    (croptop    ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{ $croptop = ($v) as f32; }};
    (cropbottom ($v:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{ $cropbottom = ($v) as f32; }};

    // explicit UV rect (normalized)
    (texrect ($u0:expr, $v0:expr, $u1:expr, $v1:expr)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $uv_rect = Some([($u0) as f32, ($v0) as f32, ($u1) as f32, ($v1) as f32]);
    }};

    (blend (alpha)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Alpha;
    }};
    (blend (normal)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Alpha;
    }};
    (blend (add)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Add;
    }};
    (blend (additive)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
    ) => {{
        $blend = $crate::core::gfx::types::BlendMode::Add;
    }};
    (blend (multiply)
        $x:ident $y:ident $w:ident $h:ident $hx:ident $vy:ident
        $tint:ident $z:ident $cell:ident $grid:ident $uv_rect:ident $visible:ident
        $flip_x:ident $flip_y:ident $cropleft:ident $cropright:ident $croptop:ident $cropbottom:ident $blend:ident
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

/// Internal muncher for `act!(text: …)` commands
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

/// Single-command handlers for `act!(text: …)`
#[doc(hidden)]
#[macro_export]
macro_rules! __ui_act_apply_one_text {
    (xy ($xv:expr, $yv:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $x = ($xv) as f32; $y = ($yv) as f32;
    }};
    (x ($xv:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $x = ($xv) as f32;
    }};
    (y ($yv:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $y = ($yv) as f32;
    }};
    (align ($hv:expr, $vv:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $hx = ($hv) as f32; $vy = ($vv) as f32;
    }};
    (px ($s:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $px = ($s) as f32;
    }};
    (diffuse ($r:expr, $g:expr, $b:expr, $a:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $tint = [($r) as f32, ($g) as f32, ($b) as f32, ($a) as f32];
    }};
    (diffusealpha ($a:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $tint[3] = ($a) as f32;
    }};
    (font ($name:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $font = $name;
    }};
    (text ($txt:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $content = ($txt).into();
    }};
    (talign ($dir:ident)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $align_mode:ident $z:ident
    ) => {{
        $align_mode = $crate::__ui_textalign_from_ident!($dir);
    }};
    (z ($v:expr)
        $x:ident $y:ident $hx:ident $vy:ident $px:ident
        $tint:ident $font:ident $content:ident $talign:ident $z:ident
    ) => {{
        $z = ($v) as i16;
    }};
    // Friendly error for unknown commands
    ($other:ident $($anything:tt)* ) => {
        compile_error!(concat!("act!(text): unknown command: ", stringify!($other)));
    };
}

/// tiny helper so `talign(center)` etc. match as an ident and map to the enum
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
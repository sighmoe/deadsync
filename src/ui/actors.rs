// src/ui/actors.rs

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Background {
    Color([f32; 4]),
    Texture(&'static str),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum SizeSpec {
    Px(f32),
    Fill,
}

#[derive(Clone, Debug)]
pub enum Actor {
    Quad { anchor: Anchor, offset: [f32; 2], size: [SizeSpec; 2], color: [f32; 4] },
    Sprite { anchor: Anchor, offset: [f32; 2], size: [SizeSpec; 2], texture: &'static str },
    Text {
        anchor: Anchor,
        offset: [f32; 2],
        px: f32,
        color: [f32; 4],
        font: &'static str,
        content: String,
        align: TextAlign,
    },
    Frame {
        anchor: Anchor,
        offset: [f32; 2],
        size: [SizeSpec; 2],
        children: Vec<Actor>,
        background: Option<Background>,
    },
}

// ---- DSL MACROS ----

#[macro_export]
macro_rules! quad {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor: Option<$crate::ui::actors::Anchor>     = None;
        let mut offset: Option<[f32; 2]>                       = None;
        let mut size:   Option<[f32; 2]>                       = None;
        let mut width:  Option<$crate::ui::actors::SizeSpec>   = None;
        let mut height: Option<$crate::ui::actors::SizeSpec>   = None;
        let mut color:  Option<[f32; 4]>                       = None;

        $( $crate::__assign_quad_kv!([anchor, offset, size, width, height, color], $k : $v); )*

        let anchor = anchor.unwrap_or($crate::ui::actors::Anchor::TopLeft);
        let offset = offset.unwrap_or([0.0_f32, 0.0_f32]);
        let w = width.unwrap_or_else(|| size.map(|s| $crate::ui::actors::SizeSpec::Px(s[0])).unwrap_or($crate::ui::actors::SizeSpec::Px(0.0)));
        let h = height.unwrap_or_else(|| size.map(|s| $crate::ui::actors::SizeSpec::Px(s[1])).unwrap_or($crate::ui::actors::SizeSpec::Px(0.0)));
        let color  = color.unwrap_or([1.0_f32, 1.0, 1.0, 1.0]);
        $crate::ui::actors::Actor::Quad { anchor, offset, size: [w, h], color }
    }};
}

#[macro_export]
macro_rules! sprite {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor:  Option<$crate::ui::actors::Anchor>     = None;
        let mut offset:  Option<[f32; 2]>                       = None;
        let mut size:    Option<[f32; 2]>                       = None;
        let mut width:   Option<$crate::ui::actors::SizeSpec>   = None;
        let mut height:  Option<$crate::ui::actors::SizeSpec>   = None;
        let mut texture: Option<&'static str>                   = None;

        $( $crate::__assign_sprite_kv!([anchor, offset, size, width, height, texture], $k : $v); )*

        let anchor  = anchor.unwrap_or($crate::ui::actors::Anchor::TopLeft);
        let offset  = offset.unwrap_or([0.0_f32, 0.0_f32]);
        let w = width.unwrap_or_else(|| size.map(|s| $crate::ui::actors::SizeSpec::Px(s[0])).unwrap_or($crate::ui::actors::SizeSpec::Px(0.0)));
        let h = height.unwrap_or_else(|| size.map(|s| $crate::ui::actors::SizeSpec::Px(s[1])).unwrap_or($crate::ui::actors::SizeSpec::Px(0.0)));
        let texture = texture.unwrap_or("");
        $crate::ui::actors::Actor::Sprite { anchor, offset, size: [w, h], texture }
    }};
}

#[macro_export]
macro_rules! text {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor:  Option<$crate::ui::actors::Anchor>     = None;
        let mut offset:  Option<[f32; 2]>                       = None;
        let mut px:      Option<f32>                            = None;
        let mut color:   Option<[f32; 4]>                       = None;
        let mut font:    Option<&'static str>                   = None;
        let mut content: Option<String>                         = None;
        let mut align:   Option<$crate::ui::actors::TextAlign>  = None;

        $( $crate::__assign_text_kv!([anchor, offset, px, color, font, content, align], $k : $v); )*

        let anchor  = anchor.unwrap_or($crate::ui::actors::Anchor::TopLeft);
        let offset  = offset.unwrap_or([0.0_f32, 0.0_f32]);
        let px      = px.unwrap_or(32.0);
        let color   = color.unwrap_or([1.0, 1.0, 1.0, 1.0]);
        let font    = font.unwrap_or("wendy");
        let content = content.unwrap_or_else(String::new);
        let align   = align.unwrap_or_default();

        $crate::ui::actors::Actor::Text { anchor, offset, px, color, font, content, align }
    }};
}

#[macro_export]
macro_rules! frame {
    ( $( $k:ident : $v:tt ),* $(,)? ) => {{
        let mut anchor:     Option<$crate::ui::actors::Anchor>      = None;
        let mut offset:     Option<[f32; 2]>                        = None;
        let mut size:       Option<[f32; 2]>                        = None;
        let mut width:      Option<$crate::ui::actors::SizeSpec>    = None;
        let mut height:     Option<$crate::ui::actors::SizeSpec>    = None;
        let mut children:   Option<::std::vec::Vec<$crate::ui::actors::Actor>> = None;
        let mut background: Option<$crate::ui::actors::Background>  = None;

        $( $crate::__assign_frame_kv!([anchor, offset, size, width, height, children, background], $k : $v ); )*

        let anchor     = anchor.unwrap_or($crate::ui::actors::Anchor::TopLeft);
        let offset     = offset.unwrap_or([0.0_f32, 0.0_f32]);
        let w = width.unwrap_or_else(|| size.map(|s| $crate::ui::actors::SizeSpec::Px(s[0])).unwrap_or($crate::ui::actors::SizeSpec::Px(0.0)));
        let h = height.unwrap_or_else(|| size.map(|s| $crate::ui::actors::SizeSpec::Px(s[1])).unwrap_or($crate::ui::actors::SizeSpec::Px(0.0)));
        let children   = children.unwrap_or_else(::std::vec::Vec::new);
        let background = background;
        $crate::ui::actors::Actor::Frame { anchor, offset, size: [w, h], children, background }
    }};
}

// ---- Macro Helpers ----

#[macro_export]
macro_rules! __assign_quad_kv {
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], anchor: $v:ident) => {
        $a = Some($crate::ui::actors::Anchor::$v)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], offset: [$x:expr, $y:expr]) => {
        $o = Some([$x as f32, $y as f32])
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], size: [$vw:expr, $vh:expr]) => {
        $s = Some([$vw as f32, $vh as f32])
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], width: Fill) => {
        $w = Some($crate::ui::actors::SizeSpec::Fill)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], width: Px($v:expr)) => {
        $w = Some($crate::ui::actors::SizeSpec::Px($v as f32))
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], height: Fill) => {
        $h = Some($crate::ui::actors::SizeSpec::Fill)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], height: Px($v:expr)) => {
        $h = Some($crate::ui::actors::SizeSpec::Px($v as f32))
    };
    // array literal color
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], color: [$r:expr, $g:expr, $b:expr, $a4:expr]) => {
        $c = Some([$r as f32, $g as f32, $b as f32, $a4 as f32])
    };
    // general expression color (e.g., a variable)
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $c:ident], color: $val:expr) => {
        $c = Some($val)
    };
}

#[macro_export]
macro_rules! __assign_sprite_kv {
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], anchor: $v:ident) => {
        $a = Some($crate::ui::actors::Anchor::$v)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], offset: [$x:expr, $y:expr]) => {
        $o = Some([$x as f32, $y as f32])
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], size: [$vw:expr, $vh:expr]) => {
        $s = Some([$vw as f32, $vh as f32])
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], width: Fill) => {
        $w = Some($crate::ui::actors::SizeSpec::Fill)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], width: Px($v:expr)) => {
        $w = Some($crate::ui::actors::SizeSpec::Px($v as f32))
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], height: Fill) => {
        $h = Some($crate::ui::actors::SizeSpec::Fill)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], height: Px($v:expr)) => {
        $h = Some($crate::ui::actors::SizeSpec::Px($v as f32))
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $t:ident], texture: $v:expr) => {
        $t = Some($v)
    };
}

#[macro_export]
macro_rules! __assign_frame_kv {
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], anchor: $v:ident) => {
        $a = Some($crate::ui::actors::Anchor::$v)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], offset: [$x:expr, $y:expr]) => {
        $o = Some([$x as f32, $y as f32])
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], size: [$vw:expr, $vh:expr]) => {
        $s = Some([$vw as f32, $vh as f32])
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], width: Fill) => {
        $w = Some($crate::ui::actors::SizeSpec::Fill)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], width: Px($v:expr)) => {
        $w = Some($crate::ui::actors::SizeSpec::Px($v as f32))
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], height: Fill) => {
        $h = Some($crate::ui::actors::SizeSpec::Fill)
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], height: Px($v:tt)) => { $h = Some($crate::ui::actors::SizeSpec::Px(($v) as f32)) };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], children: [$($child:expr),* $(,)?]) => {
        $ch = Some(vec![$($child),*])
    };
    // bg_color: array literal
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], bg_color: [$r:expr, $g:expr, $b_val:expr, $a_val:expr]) => {
        $b = Some($crate::ui::actors::Background::Color([$r as f32, $g as f32, $b_val as f32, $a_val as f32]))
    };
    // bg_color: general expression
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], bg_color: $val:expr) => {
        $b = Some($crate::ui::actors::Background::Color($val))
    };
    ([$a:ident, $o:ident, $s:ident, $w:ident, $h:ident, $ch:ident, $b:ident], bg_texture: $v:expr) => {
        $b = Some($crate::ui::actors::Background::Texture($v))
    };
}

#[macro_export]
macro_rules! __assign_text_kv {
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], anchor: $v:ident) => {
        $a = Some($crate::ui::actors::Anchor::$v)
    };
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], offset: [$x:expr, $y:expr]) => {
        $o = Some([$x as f32, $y as f32])
    };
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], px: $v:expr) => {
        $px = Some($v as f32)
    };
    // color array
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], color: [$r:expr, $g:expr, $b:expr, $a4:expr]) => {
        $c = Some([$r as f32, $g as f32, $b as f32, $a4 as f32])
    };
    // color expression (e.g., variable)
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], color: $val:expr) => {
        $c = Some($val)
    };
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], font: $v:expr) => {
        $f = Some($v)
    };
    // accept any tokens that form an expression and stringify them
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], text: $v:tt) => {
        $t = Some(($v).to_string())
    };
    ([$a:ident, $o:ident, $px:ident, $c:ident, $f:ident, $t:ident, $al:ident], align: $v:ident) => {
        $al = Some($crate::ui::actors::TextAlign::$v)
    };
}

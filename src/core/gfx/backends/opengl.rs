// src/core/gfx/backends/opengl.rs
use crate::core::gfx as renderer;
use crate::core::gfx::{ObjectType, Screen};
use crate::core::space::ortho_for_window;
use cgmath::Matrix4;
use glow::{HasContext, PixelUnpackData, UniformLocation};
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, PossiblyCurrentContext},
    display::{Display, DisplayApiPreference},
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, WindowSurface},
};
use image::RgbaImage;
use log::{info, warn};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{collections::HashMap, error::Error, ffi::CStr, mem, num::NonZeroU32, sync::Arc};
use winit::window::Window;

// A handle to an OpenGL texture on the GPU.
#[derive(Debug, Clone, Copy)]
pub struct Texture(pub glow::Texture);

pub struct State {
    pub gl: glow::Context,
    gl_surface: Surface<WindowSurface>,
    gl_context: PossiblyCurrentContext,
    program: glow::Program,
    mvp_location: UniformLocation,
    color_location: UniformLocation,
    use_texture_location: UniformLocation,
    texture_location: UniformLocation,
    projection: Matrix4<f32>,
    window_size: (u32, u32),
    // Replaced `gl_objects` with a single, shared set of buffers.
    shared_vao: glow::VertexArray,
    _shared_vbo: glow::Buffer,
    _shared_ibo: glow::Buffer,
    index_count: i32,
    uv_scale_location: UniformLocation,
    uv_offset_location: UniformLocation,
    is_msdf_location: UniformLocation,
    px_range_location: UniformLocation,

}

pub fn init(window: Arc<Window>, _screen: &Screen, vsync_enabled: bool) -> Result<State, Box<dyn Error>> {
    info!("Initializing OpenGL backend...");

    let (gl_surface, gl_context, gl) = create_opengl_context(&window, vsync_enabled)?;
    let (
        program,
        mvp_location,
        color_location,
        use_texture_location,
        texture_location,
        uv_scale_location,
        uv_offset_location,
        is_msdf_location,
        px_range_location,
    ) = create_graphics_program(&gl)?;

    // Create one shared VAO/VBO/IBO for a unit quad, to be reused for all objects.
    let (shared_vao, _shared_vbo, _shared_ibo, index_count) = unsafe {
        const UNIT_QUAD_VERTICES: [[f32; 4]; 4] = [
            [-0.5, -0.5, 0.0, 1.0],
            [ 0.5, -0.5, 1.0, 1.0],
            [ 0.5,  0.5, 1.0, 0.0],
            [-0.5,  0.5, 0.0, 0.0],
        ];
        const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

        let vao = gl.create_vertex_array()?;
        let vbo = gl.create_buffer()?;
        let ibo = gl.create_buffer()?;

        gl.bind_vertex_array(Some(vao));

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&UNIT_QUAD_VERTICES),
            glow::STATIC_DRAW,
        );

        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
        gl.buffer_data_u8_slice(
            glow::ELEMENT_ARRAY_BUFFER,
            bytemuck::cast_slice(&QUAD_INDICES),
            glow::STATIC_DRAW,
        );

        let stride = (4 * mem::size_of::<f32>()) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, (2 * mem::size_of::<f32>()) as i32);

        gl.bind_vertex_array(None);

        (vao, vbo, ibo, QUAD_INDICES.len() as i32)
    };

    let initial_size = window.inner_size();
    let projection = ortho_for_window(initial_size.width, initial_size.height);

    // Set a valid viewport immediately so the very first frame renders correctly.
    unsafe {
        gl.viewport(0, 0, initial_size.width as i32, initial_size.height as i32);
    }

    // Set constant program state once
    unsafe {
        gl.use_program(Some(program));
        gl.active_texture(glow::TEXTURE0);
        gl.uniform_1_i32(Some(&texture_location), 0);

        // default UVs and MSDF off
        gl.uniform_2_f32(Some(&uv_scale_location), 1.0, 1.0);
        gl.uniform_2_f32(Some(&uv_offset_location), 0.0, 0.0);
        gl.uniform_1_i32(Some(&is_msdf_location), 0);
        gl.uniform_1_f32(Some(&px_range_location), 4.0);

        gl.use_program(None);
    }

    let state = State {
        gl,
        gl_surface,
        gl_context,
        program,
        mvp_location,
        color_location,
        use_texture_location,
        texture_location,
        projection,
        window_size: (initial_size.width, initial_size.height),
        shared_vao,
        _shared_vbo,
        _shared_ibo,
        index_count,
        uv_scale_location,
        uv_offset_location,
        is_msdf_location,
        px_range_location,
    };

    info!("OpenGL backend initialized successfully.");
    Ok(state)
}

pub fn create_texture(gl: &glow::Context, image: &RgbaImage, srgb: bool) -> Result<Texture, String> {
    unsafe {
        let t = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(t));

        // Ensure pixel-store state is well-defined for tightly-packed RGBA8 uploads.
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);

        // Clamp and linear sample (no mips) to mirror Vulkan setup & your UI needs.
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);

        // Explicitly pin to a single mip level (no accidental sampling beyond level 0).
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_BASE_LEVEL, 0);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAX_LEVEL, 0);

        // Choose internal format based on desired color space.
        let internal = if srgb { glow::SRGB8_ALPHA8 } else { glow::RGBA8 };

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            internal as i32,
            image.width() as i32,
            image.height() as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(Some(image.as_raw().as_slice())),
        );

        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(Texture(t))
    }
}

// This function is now a no-op because the geometry buffers are static and shared.
// It's kept to maintain a consistent interface with the Vulkan backend.
pub fn load_screen(_state: &mut State, _screen: &Screen) -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn draw(
    state: &mut State,
    screen: &Screen,
    textures: &HashMap<&'static str, renderer::Texture>,
) -> Result<(), Box<dyn Error>> {
    let (width, height) = state.window_size;
    if width == 0 || height == 0 {
        return Ok(());
    }

    #[inline(always)]
    fn apply_blend(
        gl: &glow::Context,
        want: crate::core::gfx::types::BlendMode,
        last: &mut Option<crate::core::gfx::types::BlendMode>,
    ) {
        if *last == Some(want) {
            return;
        }
        unsafe {
            gl.enable(glow::BLEND);
            match want {
                crate::core::gfx::types::BlendMode::Alpha => {
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                }
                crate::core::gfx::types::BlendMode::Add => {
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::ONE, glow::ONE);
                }
                crate::core::gfx::types::BlendMode::Multiply => {
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::DST_COLOR, glow::ZERO);
                }
            }
        }
        *last = Some(want);
    }

    unsafe {
        // Clear once
        let c = screen.clear_color;
        state.gl.clear_color(c[0], c[1], c[2], c[3]);
        state.gl.clear(glow::COLOR_BUFFER_BIT);

        // Program + fixed state once
        state.gl.use_program(Some(state.program));
        state.gl.enable(glow::BLEND);
        state.gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA); // default
        state.gl.blend_equation(glow::FUNC_ADD);

        // Texture unit 0 for all textured draws
        state.gl.active_texture(glow::TEXTURE0);
        state.gl.uniform_1_i32(Some(&state.texture_location), 0);

        // Shared geometry
        state.gl.bind_vertex_array(Some(state.shared_vao));

        // Track to avoid redundant GL calls
        let mut last_bound_tex: Option<glow::Texture> = None;
        let mut last_use_texture: Option<bool> = None;
        let mut last_color: Option<[f32; 4]> = None;
        let mut last_blend: Option<crate::core::gfx::types::BlendMode> = None;

        for object in &screen.objects {
            // Per-object blend
            apply_blend(&state.gl, object.blend, &mut last_blend);

            // Per-object transform
            let mvp_array: [[f32; 4]; 4] = (state.projection * object.transform).into();
            let mvp_slice: &[f32] = bytemuck::cast_slice(&mvp_array);
            state
                .gl
                .uniform_matrix_4_f32_slice(Some(&state.mvp_location), false, mvp_slice);

            match &object.object_type {
                ObjectType::SolidColor { color } => {
                    if last_use_texture != Some(false) {
                        state.gl.uniform_1_i32(Some(&state.use_texture_location), 0);
                        last_use_texture = Some(false);
                    }
                    state.gl.uniform_1_i32(Some(&state.is_msdf_location), 0);
                    if last_bound_tex.is_some() {
                        state.gl.bind_texture(glow::TEXTURE_2D, None);
                        last_bound_tex = None;
                    }
                    if last_color.map_or(true, |c| c != *color) {
                        state.gl.uniform_4_f32_slice(Some(&state.color_location), color);
                        last_color = Some(*color);
                    }
                }
                ObjectType::Textured { texture_id } => {
                    if bind_texture_for_object(state, textures, texture_id, &mut last_bound_tex, &mut last_use_texture) {
                        state.gl.uniform_4_f32_slice(Some(&state.color_location), &[1.0, 1.0, 1.0, 1.0]);
                        state.gl.uniform_1_i32(Some(&state.is_msdf_location), 0);
                        state.gl.uniform_2_f32(Some(&state.uv_scale_location), 1.0, 1.0);
                        state.gl.uniform_2_f32(Some(&state.uv_offset_location), 0.0, 0.0);
                    } else {
                        let magenta = [1.0, 0.0, 1.0, 1.0];
                        if last_color.map_or(true, |c| c != magenta) {
                            state.gl.uniform_4_f32_slice(Some(&state.color_location), &magenta);
                            last_color = Some(magenta);
                        }
                    }
                }
                ObjectType::Sprite { texture_id, tint, uv_scale, uv_offset, } => {
                    if bind_texture_for_object(state, textures, texture_id, &mut last_bound_tex, &mut last_use_texture) {
                        state.gl.uniform_1_i32(Some(&state.is_msdf_location), 0);
                        state.gl.uniform_2_f32(Some(&state.uv_scale_location), uv_scale[0], uv_scale[1]);
                        state.gl.uniform_2_f32(Some(&state.uv_offset_location), uv_offset[0], uv_offset[1]);
                        state.gl.uniform_4_f32_slice(Some(&state.color_location), tint);
                    }
                }
                ObjectType::MsdfGlyph { texture_id, uv_scale, uv_offset, color, px_range, } => {
                    if bind_texture_for_object(state, textures, texture_id, &mut last_bound_tex, &mut last_use_texture) {
                        state.gl.uniform_1_i32(Some(&state.is_msdf_location), 1);
                        state.gl.uniform_2_f32(Some(&state.uv_scale_location), uv_scale[0], uv_scale[1]);
                        state.gl.uniform_2_f32(Some(&state.uv_offset_location), uv_offset[0], uv_offset[1]);
                        state.gl.uniform_4_f32_slice(Some(&state.color_location), color);
                        state.gl.uniform_1_f32(Some(&state.px_range_location), *px_range);
                    }
                }
            }

            state
                .gl
                .draw_elements(glow::TRIANGLES, state.index_count, glow::UNSIGNED_SHORT, 0);
        }

        state.gl.bind_vertex_array(None);
    }

    state.gl_surface.swap_buffers(&state.gl_context)?;
    Ok(())
}

pub fn resize(state: &mut State, width: u32, height: u32) {
    if width > 0 && height > 0 {
        if let (Some(width_nz), Some(height_nz)) = (NonZeroU32::new(width), NonZeroU32::new(height))
        {
            state
                .gl_surface
                .resize(&state.gl_context, width_nz, height_nz);
            unsafe {
                state.gl.viewport(0, 0, width as i32, height as i32);
            }
            state.projection = ortho_for_window(width, height);
            state.window_size = (width, height);
        }
    } else {
        warn!("Ignoring resize to zero dimensions.");
    }
}

pub fn cleanup(state: &mut State) {
    info!("Cleaning up OpenGL resources...");
    unsafe {
        // Note: Textures are cleaned up from the main `App` struct,
        // as the backend `State` doesn't own them.
        state.gl.delete_program(state.program);
        
        // Delete the shared VAO and its buffers.
        state.gl.delete_vertex_array(state.shared_vao);
        state.gl.delete_buffer(state._shared_vbo);
        state.gl.delete_buffer(state._shared_ibo);
    }
    info!("OpenGL resources cleaned up.");
}

fn create_opengl_context(
    window: &Window,
    vsync_enabled: bool,
) -> Result<(Surface<WindowSurface>, PossiblyCurrentContext, glow::Context), Box<dyn Error>> {
    let display_handle = window.display_handle()?.as_raw();

    info!("Using WGL display for OpenGL context.");
    let preference_wgl = DisplayApiPreference::Wgl(None);
    let display = unsafe { Display::new(display_handle, preference_wgl)? };

    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_stencil_size(8)
        .build();

    let config = unsafe { display.find_configs(template)?.next() }
        .ok_or("Failed to find a suitable GL config")?;

    let (width, height): (u32, u32) = window.inner_size().into();
    let raw_window_handle = window.window_handle()?.as_raw();
    let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );
    let surface = unsafe { display.create_window_surface(&config, &surface_attributes)? };

    let context_attributes =
        ContextAttributesBuilder::new().build(Some(raw_window_handle));
    let context = unsafe { display.create_context(&config, &context_attributes)? }
        .make_current(&surface)?;

    // --- VSYNC CHANGE ---
    // The standard `set_swap_interval` call fails on this driver, but the manual WGL call works.
    // We will skip the failing call and use the reliable manual method directly.
    info!("Attempting to set VSync via wglSwapIntervalEXT...");
    type SwapIntervalFn = extern "system" fn(i32) -> i32;
    let proc_name = CStr::from_bytes_with_nul(b"wglSwapIntervalEXT\0").unwrap();
    let proc = display.get_proc_address(proc_name);
    if !proc.is_null() {
        let f: SwapIntervalFn = unsafe { std::mem::transmute(proc) };
        let interval = if vsync_enabled { 1 } else { 0 };
        if f(interval) != 0 {
            info!("Successfully set VSync to: {}", if vsync_enabled { "on" } else { "off" });
        } else {
            warn!("wglSwapIntervalEXT call failed. VSync state may not be as requested.");
        }
    } else {
        warn!("wglSwapIntervalEXT function not found. Cannot control VSync.");
    }

    unsafe {
        let gl = glow::Context::from_loader_function_cstr(|s: &CStr| display.get_proc_address(s));
        gl.enable(glow::FRAMEBUFFER_SRGB);
        info!("FRAMEBUFFER_SRGB enabled.");
        Ok((surface, context, gl))
    }
}

fn create_graphics_program(
    gl: &glow::Context,
) -> Result<
    (
        glow::Program,
        UniformLocation, // u_model_view_proj
        UniformLocation, // u_color
        UniformLocation, // u_use_texture
        UniformLocation, // u_texture
        UniformLocation, // u_uv_scale
        UniformLocation, // u_uv_offset
        UniformLocation, // u_is_msdf
        UniformLocation, // u_px_range
    ),
    String,
> {
    unsafe {
        let program = gl.create_program()?;
        let shader_sources = [
            (glow::VERTEX_SHADER,   include_str!("../shaders/opengl_shader.vert")),
            (glow::FRAGMENT_SHADER, include_str!("../shaders/opengl_shader.frag")),
        ];

        let mut shaders = Vec::with_capacity(shader_sources.len());
        for (shader_type, shader_source) in shader_sources.iter() {
            let shader = gl.create_shader(*shader_type)?;
            gl.shader_source(shader, shader_source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        let mvp_location        = gl.get_uniform_location(program, "u_model_view_proj").ok_or("u_model_view_proj")?;
        let color_location      = gl.get_uniform_location(program, "u_color").ok_or("u_color")?;
        let use_texture_location= gl.get_uniform_location(program, "u_use_texture").ok_or("u_use_texture")?;
        let texture_location    = gl.get_uniform_location(program, "u_texture").ok_or("u_texture")?;
        let uv_scale_location   = gl.get_uniform_location(program, "u_uv_scale").ok_or("u_uv_scale")?;
        let uv_offset_location  = gl.get_uniform_location(program, "u_uv_offset").ok_or("u_uv_offset")?;
        let is_msdf_location    = gl.get_uniform_location(program, "u_is_msdf").ok_or("u_is_msdf")?;
        let px_range_location   = gl.get_uniform_location(program, "u_px_range").ok_or("u_px_range")?;

        Ok((
            program,
            mvp_location,
            color_location,
            use_texture_location,
            texture_location,
            uv_scale_location,
            uv_offset_location,
            is_msdf_location,
            px_range_location,
        ))
    }
}

/// Helper to bind a texture if needed, managing state changes and fallbacks.
/// Returns true if a valid texture was bound, false otherwise.
unsafe fn bind_texture_for_object(
    state: &State,
    textures: &HashMap<&'static str, renderer::Texture>,
    texture_id: &str,
    last_bound_tex: &mut Option<glow::Texture>,
    last_use_texture: &mut Option<bool>,
) -> bool {
    // This block is necessary because glow calls are unsafe.
    unsafe {
        if *last_use_texture != Some(true) {
            state.gl.uniform_1_i32(Some(&state.use_texture_location), 1);
            *last_use_texture = Some(true);
        }

        if let Some(renderer::Texture::OpenGL(gl_texture)) = textures.get(texture_id) {
            if *last_bound_tex != Some(gl_texture.0) {
                state.gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture.0));
                *last_bound_tex = Some(gl_texture.0);
            }
            true
        } else {
            // Fallback to no texture if the ID is invalid
            if *last_use_texture != Some(false) {
                state.gl.uniform_1_i32(Some(&state.use_texture_location), 0);
                *last_use_texture = Some(false);
            }
            if last_bound_tex.is_some() {
                state.gl.bind_texture(glow::TEXTURE_2D, None);
                *last_bound_tex = None;
            }
            false
        }
    }
}

mod bytemuck {
    // Safer cast: uses align_to to ensure alignment is correct.
    // For our use (f32 -> u8), this is always safe; the asserts keep us honest.
    #[inline(always)]
    pub fn cast_slice<T, U>(slice: &[T]) -> &[U] {
        // FIX: The call to `align_to` must be in an `unsafe` block.
        // We are confident this is safe because we only cast from f32 to u8,
        // and any type's alignment is a multiple of u8's alignment (which is 1).
        let (prefix, mid, suffix) = unsafe { slice.align_to::<U>() };
        debug_assert!(
            prefix.is_empty() && suffix.is_empty(),
            "cast_slice: misaligned cast"
        );
        mid
    }
}

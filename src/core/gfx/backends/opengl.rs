use crate::core::gfx::{BlendMode, ObjectType, RenderList, Texture as RendererTexture};
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
    instance_vbo: glow::Buffer,
    instanced_location: UniformLocation,
    edge_fade_location: UniformLocation,
}

pub fn init(window: Arc<Window>, vsync_enabled: bool) -> Result<State, Box<dyn Error>> {
    info!("Initializing OpenGL backend...");

    let (gl_surface, gl_context, gl) = create_opengl_context(&window, vsync_enabled)?;
    let (
        program,
        mvp_location,
        color_location,
        texture_location,
        uv_scale_location,
        uv_offset_location,
        is_msdf_location,
        px_range_location,
        instanced_location,
        edge_fade_location,
    ) = create_graphics_program(&gl)?;

    // Create shared static unit quad + index + the instance VBO (and wire attributes to it)
    let (shared_vao, _shared_vbo, _shared_ibo, index_count, instance_vbo) = unsafe {
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

        // Per-vertex: a_pos (0), a_tex_coord (1)
        let stride = (4 * mem::size_of::<f32>()) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, (2 * mem::size_of::<f32>()) as i32);

        // NEW: per-instance attributes buffer (locations 2..5)
        let instance_vbo = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_vbo));
        let i_stride = (8 * mem::size_of::<f32>()) as i32; // center(2), size(2), uv_scale(2), uv_offset(2)

        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, i_stride, 0);
        gl.vertex_attrib_divisor(2, 1);

        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_f32(3, 2, glow::FLOAT, false, i_stride, (2 * mem::size_of::<f32>()) as i32);
        gl.vertex_attrib_divisor(3, 1);

        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_f32(4, 2, glow::FLOAT, false, i_stride, (4 * mem::size_of::<f32>()) as i32);
        gl.vertex_attrib_divisor(4, 1);

        gl.enable_vertex_attrib_array(5);
        gl.vertex_attrib_pointer_f32(5, 2, glow::FLOAT, false, i_stride, (6 * mem::size_of::<f32>()) as i32);
        gl.vertex_attrib_divisor(5, 1);

        gl.bind_vertex_array(None);

        (vao, vbo, ibo, QUAD_INDICES.len() as i32, instance_vbo)
    };

    let initial_size = window.inner_size();
    let projection = ortho_for_window(initial_size.width, initial_size.height);

    unsafe {
        gl.viewport(0, 0, initial_size.width as i32, initial_size.height as i32);
        gl.use_program(Some(program));
        gl.active_texture(glow::TEXTURE0);
        gl.uniform_1_i32(Some(&texture_location), 0);

        // defaults
        gl.uniform_2_f32(Some(&uv_scale_location), 1.0, 1.0);
        gl.uniform_2_f32(Some(&uv_offset_location), 0.0, 0.0);
        gl.uniform_1_i32(Some(&is_msdf_location), 0);
        gl.uniform_1_f32(Some(&px_range_location), 4.0);
        gl.uniform_1_i32(Some(&instanced_location), 0);
        gl.uniform_4_f32(Some(&edge_fade_location), 0.0, 0.0, 0.0, 0.0);
        gl.use_program(None);
    }

    let state = State {
        gl,
        gl_surface,
        gl_context,
        program,
        mvp_location,
        color_location,
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
        instance_vbo,
        instanced_location,
        edge_fade_location,
    };

    info!("OpenGL backend initialized successfully.");
    Ok(state)
}

pub fn create_texture(gl: &glow::Context, image: &RgbaImage, srgb: bool) -> Result<Texture, String> {
    unsafe {
        let t = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(t));

        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
        gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);

        // CHANGED: Use REPEAT for texcoordvelocity to work as expected
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_BASE_LEVEL, 0);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAX_LEVEL, 0);

        let internal = if srgb { glow::SRGB8_ALPHA8 } else { glow::RGBA8 };
        let w = image.width() as i32;
        let h = image.height() as i32;
        let raw = image.as_raw();

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            internal as i32,
            w,
            h,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(Some(raw)),
        );

        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(Texture(t))
    }
}

pub fn draw(
    state: &mut State,
    render_list: &RenderList,
    textures: &HashMap<&'static str, RendererTexture>,
) -> Result<u32, Box<dyn Error>> {
    use cgmath::{Matrix4, Vector4};

    #[inline(always)]
    fn extract_center_size(t: Matrix4<f32>) -> ([f32;2], [f32;2]) {
        let c = t * Vector4::new(0.0, 0.0, 0.0, 1.0);
        let dx = t * Vector4::new(0.5, 0.0, 0.0, 0.0);
        let dy = t * Vector4::new(0.0, 0.5, 0.0, 0.0);
        let sx = 2.0 * (dx.x*dx.x + dx.y*dx.y).sqrt();
        let sy = 2.0 * (dy.x*dy.x + dy.y*dy.y).sqrt();
        ([c.x, c.y], [sx, sy])
    }

    let (width, height) = state.window_size;
    if width == 0 || height == 0 {
        return Ok(0);
    }

    #[inline(always)]
    fn apply_blend(gl: &glow::Context, want: BlendMode, last: &mut Option<BlendMode>) {
        if *last == Some(want) { return; }
        unsafe {
            gl.enable(glow::BLEND);
            match want {
                BlendMode::Alpha => {
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                }
                BlendMode::Add => {
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::SRC_ALPHA, glow::ONE);
                }
                BlendMode::Multiply => {
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::DST_COLOR, glow::ZERO);
                }
                BlendMode::Subtract => {
                    // Result = D - S (clamped)
                    gl.blend_equation(glow::FUNC_REVERSE_SUBTRACT);
                    gl.blend_func(glow::ONE, glow::ONE);
                }
            }
        }
        *last = Some(want);
    }

    let mut vertices: u32 = 0;

    unsafe {
        let gl = &state.gl;

        let c = render_list.clear_color;
        gl.clear_color(c[0], c[1], c[2], c[3]);
        gl.clear(glow::COLOR_BUFFER_BIT);

        gl.use_program(Some(state.program));
        gl.bind_vertex_array(Some(state.shared_vao));

        gl.enable(glow::BLEND);
        gl.blend_equation(glow::FUNC_ADD);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

        gl.active_texture(glow::TEXTURE0);
        gl.uniform_1_i32(Some(&state.texture_location), 0);

        let mut last_bound_tex: Option<glow::Texture> = None;
        let mut last_blend = Some(BlendMode::Alpha);
        let mut last_is_msdf: Option<bool> = None;
        let mut last_uv_scale: Option<[f32; 2]> = None;
        let mut last_uv_offset: Option<[f32; 2]> = None;
        let mut last_px_range: Option<f32> = None;
        let mut last_color: Option<[f32; 4]> = None;
        let mut last_edge_fade: Option<[f32; 4]> = None;
        let mut instanced_on = false;

        let proj: [[f32;4];4] = state.projection.into();
        let proj_slice: &[f32] = bytemuck::cast_slice(&proj);

        let mut i = 0;
        while i < render_list.objects.len() {
            let obj = &render_list.objects[i];

            if let ObjectType::MsdfGlyph { texture_id, color, px_range, .. } = obj.object_type {
                let mut run_instances: Vec<f32> = Vec::new();
                run_instances.reserve(8 * 64);

                let Some(RendererTexture::OpenGL(gl_tex)) = textures.get(texture_id) else {
                    i += 1; continue;
                };

                let mut j = i;
                while j < render_list.objects.len() {
                    match &render_list.objects[j].object_type {
                        ObjectType::MsdfGlyph { texture_id: tid2, uv_scale: s2, uv_offset: o2, color: c2, px_range: pr2 }
                            if tid2 == &texture_id && c2 == &color && pr2 == &px_range =>
                        {
                            let (center, size) = extract_center_size(render_list.objects[j].transform);
                            run_instances.extend_from_slice(&[ center[0], center[1], size[0], size[1], s2[0], s2[1], o2[0], o2[1] ]);
                            j += 1;
                        }
                        _ => break,
                    }
                }

                if last_bound_tex != Some(gl_tex.0) {
                    gl.bind_texture(glow::TEXTURE_2D, Some(gl_tex.0));
                    last_bound_tex = Some(gl_tex.0);
                }

                if last_is_msdf != Some(true) { gl.uniform_1_i32(Some(&state.is_msdf_location), 1); last_is_msdf = Some(true); }
                if !instanced_on { gl.uniform_1_i32(Some(&state.instanced_location), 1); instanced_on = true; }

                gl.uniform_matrix_4_f32_slice(Some(&state.mvp_location), false, proj_slice);

                if last_px_range != Some(px_range) {
                    gl.uniform_1_f32(Some(&state.px_range_location), px_range);
                    last_px_range = Some(px_range);
                }
                if last_color != Some(color) {
                    gl.uniform_4_f32_slice(Some(&state.color_location), &color);
                    last_color = Some(color);
                }

                gl.bind_buffer(glow::ARRAY_BUFFER, Some(state.instance_vbo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&run_instances), glow::STREAM_DRAW);

                apply_blend(gl, obj.blend, &mut last_blend);
                let inst_count = (run_instances.len() / 8) as i32;
                gl.draw_elements_instanced(glow::TRIANGLES, state.index_count, glow::UNSIGNED_SHORT, 0, inst_count);

                vertices += 4 * (inst_count as u32);
                i = j;
                continue;
            }

            if instanced_on { gl.uniform_1_i32(Some(&state.instanced_location), 0); instanced_on = false; }

            apply_blend(gl, obj.blend, &mut last_blend);

            let mvp_array: [[f32; 4]; 4] = (state.projection * obj.transform).into();
            gl.uniform_matrix_4_f32_slice(Some(&state.mvp_location), false, bytemuck::cast_slice(&mvp_array));

            match &obj.object_type {
                ObjectType::Sprite { texture_id, tint, uv_scale, uv_offset, edge_fade } => {
                    if let Some(RendererTexture::OpenGL(gl_tex)) = textures.get(texture_id) {
                        if last_bound_tex != Some(gl_tex.0) {
                            gl.bind_texture(glow::TEXTURE_2D, Some(gl_tex.0));
                            last_bound_tex = Some(gl_tex.0);
                        }
                        if last_is_msdf != Some(false) {
                            gl.uniform_1_i32(Some(&state.is_msdf_location), 0);
                            last_is_msdf = Some(false);
                        }
                        if last_uv_scale != Some(*uv_scale) {
                            gl.uniform_2_f32(Some(&state.uv_scale_location), uv_scale[0], uv_scale[1]);
                            last_uv_scale = Some(*uv_scale);
                        }
                        if last_uv_offset != Some(*uv_offset) {
                            gl.uniform_2_f32(Some(&state.uv_offset_location), uv_offset[0], uv_offset[1]);
                            last_uv_offset = Some(*uv_offset);
                        }
                        if last_color != Some(*tint) {
                            gl.uniform_4_f32_slice(Some(&state.color_location), tint);
                            last_color = Some(*tint);
                        }
                        if last_edge_fade != Some(*edge_fade) {
                            gl.uniform_4_f32_slice(Some(&state.edge_fade_location), edge_fade);
                            last_edge_fade = Some(*edge_fade);
                        }
                        gl.draw_elements(glow::TRIANGLES, state.index_count, glow::UNSIGNED_SHORT, 0);
                        vertices += 4;
                    }
                }
                // We handle MsdfGlyph in the instanced path above
                ObjectType::MsdfGlyph { .. } => unreachable!("handled above"),
            }
            i += 1;
        }
        if instanced_on { gl.uniform_1_i32(Some(&state.instanced_location), 0); }
        gl.bind_vertex_array(None);
    }

    state.gl_surface.swap_buffers(&state.gl_context)?;
    Ok(vertices)
}

pub fn resize(state: &mut State, width: u32, height: u32) {
    if width == 0 || height == 0 {
        warn!("Ignoring resize to zero dimensions.");
        return;
    }
    let w = NonZeroU32::new(width).unwrap();
    let h = NonZeroU32::new(height).unwrap();

    state.gl_surface.resize(&state.gl_context, w, h);
    unsafe {
        state.gl.viewport(0, 0, width as i32, height as i32);
    }
    state.projection = ortho_for_window(width, height);
    state.window_size = (width, height);
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
        UniformLocation, // mvp
        UniformLocation, // color
        UniformLocation, // texture
        UniformLocation, // uv_scale
        UniformLocation, // uv_offset
        UniformLocation, // is_msdf
        UniformLocation, // px_range
        UniformLocation, // instanced
        UniformLocation, // edge_fade
    ),
    String,
> {
    unsafe {
        let program = gl.create_program()?;

        let compile = |ty, src: &str| -> Result<glow::Shader, String> {
            let sh = gl.create_shader(ty)?;
            gl.shader_source(sh, src);
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                let log = gl.get_shader_info_log(sh);
                gl.delete_shader(sh);
                return Err(log);
            }
            Ok(sh)
        };

        let vert = compile(glow::VERTEX_SHADER, include_str!("../shaders/opengl_shader.vert"))?;
        let frag = compile(glow::FRAGMENT_SHADER, include_str!("../shaders/opengl_shader.frag"))?;

        gl.attach_shader(program, vert);
        gl.attach_shader(program, frag);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.detach_shader(program, vert);
            gl.detach_shader(program, frag);
            gl.delete_shader(vert);
            gl.delete_shader(frag);
            gl.delete_program(program);
            return Err(log);
        }
        gl.detach_shader(program, vert);
        gl.detach_shader(program, frag);
        gl.delete_shader(vert);
        gl.delete_shader(frag);

        let get = |name: &str| gl.get_uniform_location(program, name).ok_or_else(|| name.to_string());

        let mvp_location        = get("u_model_view_proj")?;
        let color_location      = get("u_color")?;
        let texture_location    = get("u_texture")?;
        let uv_scale_location   = get("u_uv_scale")?;
        let uv_offset_location  = get("u_uv_offset")?;
        let is_msdf_location    = get("u_is_msdf")?;
        let px_range_location   = get("u_px_range")?;
        let instanced_location  = get("u_instanced")?;
        let edge_fade_location  = get("u_edge_fade")?;

        Ok((
            program,
            mvp_location,
            color_location,
            texture_location,
            uv_scale_location,
            uv_offset_location,
            is_msdf_location,
            px_range_location,
            instanced_location,
            edge_fade_location,
        ))
    }
}

mod bytemuck {
    // Safer cast: uses align_to to ensure alignment is correct.
    // For our use (f32 -> u8), this is always safe; the asserts keep us honest.
    #[inline(always)]
    pub fn cast_slice<T, U>(slice: &[T]) -> &[U] {
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

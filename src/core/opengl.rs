use crate::{
    renderer,
    screen::{ObjectType, Screen, ScreenObject},
};
use crate::math::ortho_for_window;
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

// This struct is no longer needed, as its resources are now shared in State.
/*
struct OpenGLObject {
    vao: glow::VertexArray,
    _vbo: glow::Buffer,
    _ibo: glow::Buffer,
    index_count: i32,
}
*/

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
}

pub fn init(window: Arc<Window>, _screen: &Screen, vsync_enabled: bool) -> Result<State, Box<dyn Error>> {
    info!("Initializing OpenGL backend...");

    let (gl_surface, gl_context, gl) = create_opengl_context(&window, vsync_enabled)?;
    let (program, mvp_location, color_location, use_texture_location, texture_location) =
        create_graphics_program(&gl)?;

    // Create one shared VAO/VBO/IBO for a unit quad, to be reused for all objects.
    let (shared_vao, _shared_vbo, _shared_ibo, index_count) = unsafe {
        // These constants are now defined once during initialization.
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
        // Position attribute (location 0)
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        // UV attribute (location 1)
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, (2 * mem::size_of::<f32>()) as i32);

        gl.bind_vertex_array(None);

        (vao, vbo, ibo, QUAD_INDICES.len() as i32)
    };

    let initial_size = window.inner_size();
    let projection = ortho_for_window(initial_size.width, initial_size.height);

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
    };

    // `load_screen` is no longer needed at init time because resources are static.
    info!("OpenGL backend initialized successfully.");
    Ok(state)
}

pub fn create_texture(gl: &glow::Context, image: &RgbaImage) -> Result<Texture, String> {
    unsafe {
        let texture = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));

        // Set texture parameters (unchanged)
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );

        // Updated to match glow 0.16.0's PixelUnpackData::Slice(Option<&[u8]>)
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::SRGB_ALPHA as i32,
            image.width() as i32,
            image.height() as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(Some(image.as_raw().as_slice())),  // Add Some() inside Slice
        );

        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(Texture(texture))
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
    textures: &HashMap<&'static str, renderer::Texture>, // Changed from String
) -> Result<(), Box<dyn Error>> {
    let (width, height) = state.window_size;
    if width == 0 || height == 0 {
        return Ok(());
    }

    unsafe {
        let c = screen.clear_color;
        state.gl.clear_color(c[0], c[1], c[2], c[3]);
        state.gl.clear(glow::COLOR_BUFFER_BIT);
        state.gl.use_program(Some(state.program));

        // Enable blending for transparency
        state.gl.enable(glow::BLEND);
        state.gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

        // Activate texture unit 0 once
        state.gl.active_texture(glow::TEXTURE0);
        state.gl.uniform_1_i32(Some(&state.texture_location), 0);
        
        // Bind the single, shared VAO once for all draw calls.
        state.gl.bind_vertex_array(Some(state.shared_vao));

        for object in screen.objects.iter() {
            let mvp_array: [[f32; 4]; 4] = (state.projection * object.transform).into();
            let mvp_slice: &[f32] = bytemuck::cast_slice(&mvp_array); // safe

            state.gl.uniform_matrix_4_f32_slice(
                Some(&state.mvp_location),
                false,
                mvp_slice,
            );

            match &object.object_type {
                ObjectType::SolidColor { color } => {
                    state.gl.uniform_1_i32(Some(&state.use_texture_location), 0);
                    state
                        .gl
                        .uniform_4_f32_slice(Some(&state.color_location), color);
                    // Unbind any previous texture to avoid surprises
                    state.gl.bind_texture(glow::TEXTURE_2D, None);
                }
                ObjectType::Textured { texture_id } => {
                    // This `get` call will now work correctly with a &String key.
                    if let Some(renderer::Texture::OpenGL(gl_texture)) = textures.get(texture_id) {
                        state.gl.uniform_1_i32(Some(&state.use_texture_location), 1);
                        state.gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture.0));
                    } else {
                        warn!("Texture ID '{}' not found in texture manager!", texture_id);
                        // Fallback: draw with a bright magenta color (debugging-friendly)
                        let magenta = [1.0, 0.0, 1.0, 1.0];
                        state.gl.uniform_1_i32(Some(&state.use_texture_location), 0);
                        state
                            .gl
                            .uniform_4_f32_slice(Some(&state.color_location), &magenta);
                        state.gl.bind_texture(glow::TEXTURE_2D, None);
                    }
                }
            }

            // The VAO is already bound, so we just issue the draw call.
            state.gl.draw_elements(
                glow::TRIANGLES,
                state.index_count,
                glow::UNSIGNED_SHORT,
                0,
            );
        }
        // Unbind VAO after all objects are drawn.
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
        UniformLocation,
        UniformLocation,
        UniformLocation,
        UniformLocation,
    ),
    String,
> {
    unsafe {
        let program = gl.create_program()?;
        let shader_sources = [
            (
                glow::VERTEX_SHADER,
                include_str!("../shaders/opengl_shader.vert"),
            ),
            (
                glow::FRAGMENT_SHADER,
                include_str!("../shaders/opengl_shader.frag"),
            ),
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

        let mvp_location = gl
            .get_uniform_location(program, "u_model_view_proj")
            .ok_or("Could not find 'u_model_view_proj' uniform")?;
        let color_location = gl
            .get_uniform_location(program, "u_color")
            .ok_or("Could not find 'u_color' uniform")?;
        let use_texture_location = gl
            .get_uniform_location(program, "u_use_texture")
            .ok_or("Could not find 'u_use_texture' uniform")?;
        let texture_location = gl
            .get_uniform_location(program, "u_texture")
            .ok_or("Could not find 'u_texture' uniform")?;

        Ok((
            program,
            mvp_location,
            color_location,
            use_texture_location,
            texture_location,
        ))
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
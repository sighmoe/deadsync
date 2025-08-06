// ===== FILE: /mnt/c/Users/perfe/Code/vulkan-opengl-mix/src/opengl_renderer.rs =====
use crate::screen::{Screen, ScreenObject};
use cgmath::Matrix4;
use glow::{HasContext, UniformLocation};
use glutin::{
    context::{ContextAttributesBuilder, PossiblyCurrentContext},
    config::{ConfigTemplateBuilder},
    display::{Display, DisplayApiPreference},
    prelude::*,
    surface::{Surface, SurfaceAttributesBuilder, WindowSurface},
};
use log::{info, warn};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{error::Error, ffi::CStr, mem, num::NonZeroU32, sync::Arc};
use winit::window::Window;

struct OpenGLObject {
    vao: glow::VertexArray,
    _vbo: glow::Buffer,
    _ibo: glow::Buffer,
    index_count: i32,
}

pub struct State {
    gl: glow::Context,
    gl_surface: Surface<WindowSurface>,
    gl_context: PossiblyCurrentContext,
    program: glow::Program,
    mvp_location: UniformLocation,
    color_location: UniformLocation,
    projection: Matrix4<f32>,
    window_size: (u32, u32),
    gl_objects: Vec<OpenGLObject>,
}

pub fn init(window: Arc<Window>, screen: &Screen) -> Result<State, Box<dyn Error>> {
    info!("Initializing OpenGL backend...");

    let (gl_surface, gl_context, gl) = create_opengl_context(&window)?;
    let (program, mvp_location, color_location) = create_graphics_program(&gl)?;

    let mut gl_objects = Vec::new();
    for object in &screen.objects {
        gl_objects.push(create_object_resources(&gl, object)?);
    }

    let initial_size = window.inner_size();
    let projection = create_projection_matrix(initial_size.width, initial_size.height);

    info!("OpenGL backend initialized successfully.");
    Ok(State {
        gl,
        gl_surface,
        gl_context,
        program,
        mvp_location,
        color_location,
        projection,
        window_size: (initial_size.width, initial_size.height),
        gl_objects,
    })
}

pub fn draw(state: &mut State, screen: &Screen) -> Result<(), Box<dyn Error>> {
    let (width, height) = state.window_size;
    if width == 0 || height == 0 {
        return Ok(());
    }

    unsafe {
        let c = screen.clear_color;
        state.gl.clear_color(c[0], c[1], c[2], c[3]);
        state.gl.clear(glow::COLOR_BUFFER_BIT);
        state.gl.use_program(Some(state.program));

        for (i, object) in screen.objects.iter().enumerate() {
            let gl_object = &state.gl_objects[i];
            let mvp = state.projection * object.transform;
            let mvp_array: [[f32; 4]; 4] = mvp.into();

            state
                .gl
                .uniform_matrix_4_f32_slice(Some(&state.mvp_location), false, &mvp_array.concat());
            state
                .gl
                .uniform_4_f32_slice(Some(&state.color_location), &object.color);

            state.gl.bind_vertex_array(Some(gl_object.vao));
            state
                .gl
                .draw_elements(glow::TRIANGLES, gl_object.index_count, glow::UNSIGNED_SHORT, 0);
        }
    }
    state.gl_surface.swap_buffers(&state.gl_context)?;
    Ok(())
}

pub fn resize(state: &mut State, width: u32, height: u32) {
    if width > 0 && height > 0 {
        if let (Some(width_nz), Some(height_nz)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            state.gl_surface.resize(&state.gl_context, width_nz, height_nz);
            unsafe {
                state.gl.viewport(0, 0, width as i32, height as i32);
            }
            state.projection = create_projection_matrix(width, height);
            state.window_size = (width, height);
        }
    } else {
        warn!("Ignoring resize to zero dimensions.");
    }
}

pub fn cleanup(state: &mut State) {
    info!("Cleaning up OpenGL resources...");
    unsafe {
        state.gl.delete_program(state.program);
        for object in state.gl_objects.iter() {
            state.gl.delete_vertex_array(object.vao);
            state.gl.delete_buffer(object._vbo);
            state.gl.delete_buffer(object._ibo);
        }
    }
    info!("OpenGL resources cleaned up.");
}

fn create_opengl_context(
    window: &Window,
) -> Result<(Surface<WindowSurface>, PossiblyCurrentContext, glow::Context), Box<dyn Error>> {
    let preference = DisplayApiPreference::Wgl(None);
    let display = unsafe { Display::new(window.display_handle()?.into(), preference)? };

    // FIX: Revert to the original, more robust config search.
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_stencil_size(8)
        .build();

    let config = unsafe { display.find_configs(template)?.next() }
        .ok_or("Failed to find a suitable GL config")?;

    let (width, height): (u32, u32) = window.inner_size().into();
    let raw_window_handle = window.window_handle()?;
    let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle.into(),
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );
    let surface = unsafe { display.create_window_surface(&config, &surface_attributes)? };

    let context_attributes =
        ContextAttributesBuilder::new().build(Some(raw_window_handle.into()));
    let context = unsafe { display.create_context(&config, &context_attributes)? }
        .make_current(&surface)?;

    // This is the most important part for correct sRGB color.
    unsafe {
        let gl = glow::Context::from_loader_function_cstr(|s: &CStr| display.get_proc_address(s));
        gl.enable(glow::FRAMEBUFFER_SRGB);
        info!("FRAMEBUFFER_SRGB enabled.");
        Ok((surface, context, gl))
    }
}

fn create_graphics_program(
    gl: &glow::Context,
) -> Result<(glow::Program, UniformLocation, UniformLocation), String> {
    unsafe {
        let program = gl.create_program()?;
        let shader_sources = [
            (glow::VERTEX_SHADER, include_str!("../shaders/opengl_shader.vert")),
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

        let mvp_location = gl.get_uniform_location(program, "u_model_view_proj")
            .ok_or("Could not find 'u_model_view_proj' uniform")?;
        let color_location = gl.get_uniform_location(program, "u_color")
            .ok_or("Could not find 'u_color' uniform")?;

        Ok((program, mvp_location, color_location))
    }
}

fn create_object_resources(
    gl: &glow::Context,
    object: &ScreenObject,
) -> Result<OpenGLObject, String> {
    unsafe {
        let vbo = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(&object.vertices),
            glow::STATIC_DRAW,
        );

        let ibo = gl.create_buffer()?;
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
        gl.buffer_data_u8_slice(
            glow::ELEMENT_ARRAY_BUFFER,
            bytemuck::cast_slice(&object.indices),
            glow::STATIC_DRAW,
        );

        let vao = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ibo));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 2 * mem::size_of::<f32>() as i32, 0);

        Ok(OpenGLObject {
            vao,
            _vbo: vbo,
            _ibo: ibo,
            index_count: object.indices.len() as i32,
        })
    }
}

fn create_projection_matrix(width: u32, height: u32) -> Matrix4<f32> {
    let aspect_ratio = width as f32 / height as f32;
    let (ortho_width, ortho_height) = if aspect_ratio >= 1.0 {
        (400.0 * aspect_ratio, 400.0)
    } else {
        (400.0, 400.0 / aspect_ratio)
    };
    cgmath::ortho(-ortho_width, ortho_width, -ortho_height, ortho_height, -1.0, 1.0)
}

mod bytemuck {
    pub unsafe fn cast_slice<T, U>(slice: &[T]) -> &[U] {
        unsafe {
            std::slice::from_raw_parts(
                slice.as_ptr() as *const U,
                (slice.len() * std::mem::size_of::<T>()) / std::mem::size_of::<U>(),
            )
        }
    }
}
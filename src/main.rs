mod texture;
mod utils;
mod vulkan_base;

use ash::vk;
use cgmath::{ortho, Matrix4, Rad, Vector3};
use log::{debug, error, info, trace, LevelFilter};
use rand::prelude::*;
use rodio::{Decoder, OutputStream, Sink};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::f32::consts::PI;
use std::ffi::CString;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::mem;
use std::path::Path;
use std::time::{Duration, Instant};

use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::WindowBuilder,
};

use texture::load_texture;
use utils::fps::FPSCounter;
use vulkan_base::{UniformBufferObject, Vertex, VulkanBase};

// --- Constants and Structs ---
const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;
const TARGET_Y_POS: f32 = 150.0;
const TARGET_SIZE: f32 = 120.0;
const ARROW_SIZE: f32 = 120.0;
const ARROW_SPEED: f32 = 600.0;
const SONG_BPM: f32 = 174.0;
const SONG_FOLDER_PATH: &str = "Songs/Liquidity/About Tonight"; // Temporary static test song
const SONG_AUDIO_FILENAME: &str = "about_tonight.ogg"; // Temporary static test song
const AUDIO_SYNC_OFFSET_MS: i64 = 60;
const SPAWN_LOOKAHEAD_BEATS: f32 = 10.0;
const DIFFICULTY: u32 = 2;
const W1_WINDOW_MS: f32 = 22.5;
const W2_WINDOW_MS: f32 = 45.0;
const W3_WINDOW_MS: f32 = 90.0;
const W4_WINDOW_MS: f32 = 135.0;
const MAX_HIT_WINDOW_MS: f32 = 180.0;
const MISS_WINDOW_MS: f32 = 200.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteType {
    Quarter,
    Eighth,
    Sixteenth,
}
const TARGET_TINT: [f32; 4] = [0.7, 0.7, 0.7, 0.5];
const ARROW_TINT_QUARTER: [f32; 4] = [1.0, 0.6, 0.6, 1.0];
const ARROW_TINT_EIGHTH: [f32; 4] = [0.6, 0.6, 1.0, 1.0];
const ARROW_TINT_SIXTEENTH: [f32; 4] = [0.6, 1.0, 0.6, 1.0];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judgment {
    W1,
    W2,
    W3,
    W4,
    Miss,
}
const FLASH_COLOR_W1: [f32; 4] = [0.2, 0.7, 1.0, 0.9];
const FLASH_COLOR_W2: [f32; 4] = [1.0, 0.8, 0.2, 0.9];
const FLASH_COLOR_W3: [f32; 4] = [0.2, 1.0, 0.2, 0.9];
const FLASH_COLOR_W4: [f32; 4] = [0.8, 0.4, 1.0, 0.9];
const FLASH_DURATION: Duration = Duration::from_millis(120);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ArrowDirection {
    Left,
    Down,
    Up,
    Right,
}
const ARROW_DIRECTIONS: [ArrowDirection; 4] = [
    ArrowDirection::Left,
    ArrowDirection::Down,
    ArrowDirection::Up,
    ArrowDirection::Right,
];

#[derive(Debug, Clone)]
struct TargetInfo {
    x: f32,
    y: f32,
    direction: ArrowDirection,
}
#[derive(Debug, Clone)]
struct Arrow {
    x: f32,
    y: f32,
    direction: ArrowDirection,
    note_type: NoteType,
    target_beat: f32,
}
#[derive(Debug, Clone, Copy)]
struct FlashState {
    color: [f32; 4],
    end_time: Instant,
}

struct GameState {
    targets: Vec<TargetInfo>,
    arrows: HashMap<ArrowDirection, Vec<Arrow>>,
    pressed_keys: HashSet<VirtualKeyCode>,
    last_spawned_16th_index: i32,
    last_spawned_direction: Option<ArrowDirection>,
    current_beat: f32,
    window_size: (f32, f32),
    flash_states: HashMap<ArrowDirection, FlashState>,
    modifiers: ModifiersState,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct PushConstantData {
    model: Matrix4<f32>,
    color: [f32; 4],
    uv_offset: [f32; 2],
    uv_scale: [f32; 2],
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum VirtualKeyCode {
    Left,
    Down,
    Up,
    Right,
    Escape,
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Trace)
        .init();

    // --- Audio Setup ---
    info!("Initializing audio stream...");
    let (_stream, stream_handle) = OutputStream::try_default()?;
    info!("Audio stream handle obtained.");
    let audio_path_str = format!("{}/{}", SONG_FOLDER_PATH, SONG_AUDIO_FILENAME);
    let audio_path = Path::new(&audio_path_str);
    info!("Attempting to load audio file: {:?}", audio_path);
    let file = File::open(audio_path)
        .map_err(|e| format!("Failed to open audio file {:?}: {}", audio_path, e))?;
    info!("Audio file opened: {:?}", audio_path);
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio file {:?}: {}", audio_path, e))?;
    info!("Audio file decoded successfully.");
    let sink = Sink::try_new(&stream_handle)?;
    info!("Audio sink created.");
    sink.append(source);
    sink.play();
    sink.detach();
    info!("Audio playback started for {:?}.", audio_path);

    // --- Winit Setup ---
    info!("Initializing Winit...");
    let mut event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Ash RITG - Vulkan")
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(WINDOW_WIDTH),
            f64::from(WINDOW_HEIGHT),
        ))
        .build(&event_loop)?;
    let initial_window_size = window.inner_size();

    // --- VulkanBase Setup ---
    info!("Initializing VulkanBase...");
    let mut base = VulkanBase::new(window)?;
    info!("VulkanBase Initialized for window: {:?}", base.window.id());
    info!("GPU: {}", base.get_gpu_name());

    // --- Game State & RNG ---
    let mut game_state = initialize_game_state(
        initial_window_size.width as f32,
        initial_window_size.height as f32,
    );
    let mut fps_counter = FPSCounter::new();
    let mut last_frame_time = Instant::now();
    let mut rng = rand::rng();

    // --- Vulkan Resource Creation ---
    let quad_vertices: [Vertex; 4] = [
        Vertex {
            pos: [-0.5, -0.5],
            tex_coord: [0.0, 0.0],
        },
        Vertex {
            pos: [0.5, -0.5],
            tex_coord: [1.0, 0.0],
        },
        Vertex {
            pos: [0.5, 0.5],
            tex_coord: [1.0, 1.0],
        },
        Vertex {
            pos: [-0.5, 0.5],
            tex_coord: [0.0, 1.0],
        },
    ];
    let vertex_buffer_size = (quad_vertices.len() * mem::size_of::<Vertex>()) as vk::DeviceSize;
    let mut vertex_buffer = base.create_buffer(
        vertex_buffer_size,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    base.update_buffer(&vertex_buffer, &quad_vertices)?;
    let quad_indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
    let index_buffer_size = (quad_indices.len() * mem::size_of::<u32>()) as vk::DeviceSize;
    let mut index_buffer = base.create_buffer(
        index_buffer_size,
        vk::BufferUsageFlags::INDEX_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    base.update_buffer(&index_buffer, &quad_indices)?;
    let ubo_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
    let mut projection_ubo = base.create_buffer(
        ubo_size,
        vk::BufferUsageFlags::UNIFORM_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    // Check if the texture file exists and load it
    let arrow_texture_path = Path::new("assets/down_arrow_atlas.png"); // Must be executed from the code folder ./target/release/deadsync
    if !arrow_texture_path.exists() {
        eprintln!("Error: Texture file not found at {:?}", arrow_texture_path);
        std::process::exit(1);
    }
    let mut arrow_texture = load_texture(&base, arrow_texture_path)?;
    info!("Texture loaded successfully: {:?}", arrow_texture_path);

    // --- Descriptors, Pipeline Layout, Pipeline ---
    let dsl_bindings = [
        // Binding 0: Uniform Buffer (Vertex Shader)
        vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::VERTEX,
            p_immutable_samplers: std::ptr::null(),
            ..Default::default()
        },
        // Binding 1: Combined Image Sampler (Fragment Shader)
        vk::DescriptorSetLayoutBinding {
            binding: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            p_immutable_samplers: std::ptr::null(),
            ..Default::default()
        },
    ];
    let dsl_create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&dsl_bindings);
    let descriptor_set_layout = unsafe {
        base.device
            .create_descriptor_set_layout(&dsl_create_info, None)?
    };
    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        },
    ];
    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&pool_sizes)
        .max_sets(1);
    let descriptor_pool = unsafe {
        base.device
            .create_descriptor_pool(&pool_create_info, None)?
    };
    let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(std::slice::from_ref(&descriptor_set_layout));
    let descriptor_set = unsafe { base.device.allocate_descriptor_sets(&desc_alloc_info)?[0] };
    let ubo_buffer_info = vk::DescriptorBufferInfo::default()
        .buffer(projection_ubo.buffer)
        .offset(0)
        .range(vk::WHOLE_SIZE);
    let write_ubo = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .buffer_info(std::slice::from_ref(&ubo_buffer_info));
    let image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(arrow_texture.view)
        .sampler(arrow_texture.sampler);
    let write_sampler = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&image_info));
    unsafe {
        base.device
            .update_descriptor_sets(&[write_ubo, write_sampler], &[])
    };
    let push_constant_ranges = [vk::PushConstantRange {
        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
        offset: 0,
        size: mem::size_of::<PushConstantData>() as u32,
    }];
    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(std::slice::from_ref(&descriptor_set_layout))
        .push_constant_ranges(&push_constant_ranges);
    let pipeline_layout = unsafe {
        base.device
            .create_pipeline_layout(&pipeline_layout_create_info, None)?
    };
    let mut vert_shader_file = Cursor::new(&include_bytes!("../shaders/vert.spv")[..]);
    let mut frag_shader_file = Cursor::new(&include_bytes!("../shaders/frag.spv")[..]);
    let vert_code = ash::util::read_spv(&mut vert_shader_file)?;
    let frag_code = ash::util::read_spv(&mut frag_shader_file)?;
    let vert_module_info = vk::ShaderModuleCreateInfo::default().code(&vert_code);
    let frag_module_info = vk::ShaderModuleCreateInfo::default().code(&frag_code);
    let vert_shader_module = unsafe { base.device.create_shader_module(&vert_module_info, None)? };
    let frag_shader_module = unsafe { base.device.create_shader_module(&frag_module_info, None)? };
    let shader_entry_name = CString::new("main").unwrap();
    let shader_stage_create_infos = [
        vk::PipelineShaderStageCreateInfo {
            module: vert_shader_module,
            p_name: shader_entry_name.as_ptr(),
            stage: vk::ShaderStageFlags::VERTEX,
            ..Default::default()
        },
        vk::PipelineShaderStageCreateInfo {
            module: frag_shader_module,
            p_name: shader_entry_name.as_ptr(),
            stage: vk::ShaderStageFlags::FRAGMENT,
            ..Default::default()
        },
    ];
    let binding_descriptions = [vk::VertexInputBindingDescription {
        binding: 0,
        stride: std::mem::size_of::<Vertex>() as u32,
        input_rate: vk::VertexInputRate::VERTEX,
    }];

    let attribute_descriptions = [
        vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: offset_of!(Vertex, pos) as u32,
        },
        vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: offset_of!(Vertex, tex_coord) as u32,
        },
    ];
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&binding_descriptions)
        .vertex_attribute_descriptions(&attribute_descriptions);
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE);
    let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD);
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .attachments(std::slice::from_ref(&color_blend_attachment));
    let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(false)
        .depth_write_enable(false);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state_info =
        vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stage_create_infos)
        .vertex_input_state(&vertex_input_state)
        .input_assembly_state(&input_assembly_state)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization_state)
        .multisample_state(&multisample_state)
        .color_blend_state(&color_blend_state)
        .depth_stencil_state(&depth_stencil_state)
        .layout(pipeline_layout)
        .render_pass(base.render_pass)
        .subpass(0)
        .dynamic_state(&dynamic_state_info);
    let graphics_pipeline = unsafe {
        base.device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .map_err(|(_, err)| err)?[0]
    };
    unsafe {
        base.device.destroy_shader_module(vert_shader_module, None);
        base.device.destroy_shader_module(frag_shader_module, None);
    }
    info!("Graphics Pipeline Created.");

    // --- Initial Projection Matrix Update ---
    let projection_matrix = ortho(
        0.0,
        game_state.window_size.0,
        0.0,
        game_state.window_size.1,
        1.0,
        -1.0,
    );
    let ubo = UniformBufferObject {
        projection: projection_matrix,
    };
    base.update_buffer(&projection_ubo, &[ubo])?;

    // --- Event Loop ---
    info!("Starting Event Loop...");
    let mut resize_needed = false;
    event_loop.run_on_demand(|event, elwp| {
        elwp.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, window_id } if window_id == base.window.id() => {
                handle_input(&event, &mut game_state, elwp);

                if let WindowEvent::Resized(new_size) = event {
                    log::info!("Window resized event: {:?}", new_size);
                    if new_size.width > 0 && new_size.height > 0 {
                        game_state.window_size = (new_size.width as f32, new_size.height as f32);
                        resize_needed = true;
                    }
                }
            }
            Event::AboutToWait => {
                if resize_needed {
                    log::warn!("Resize detected - Vulkan swapchain recreation NOT IMPLEMENTED!");
                    let projection_matrix = ortho(
                        0.0,
                        game_state.window_size.0,
                        0.0,
                        game_state.window_size.1,
                        -1.0,
                        1.0,
                    );
                    let ubo = UniformBufferObject {
                        projection: projection_matrix,
                    };
                    if let Err(e) = base.update_buffer(&projection_ubo, &[ubo]) {
                        error!("Failed to update projection UBO after resize: {}", e);
                    } else {
                        info!(
                            "Projection matrix updated for new size: {:?}",
                            game_state.window_size
                        );
                    }
                    resize_needed = false;
                }

                let now = Instant::now();
                let dt = (now - last_frame_time).as_secs_f32().max(0.0).min(0.1);
                last_frame_time = now;
                update_game_state(&mut game_state, dt, &mut rng);

                if let Some(fps) = fps_counter.update() {
                    base.window
                        .set_title(&format!("Ash RITG | BPM: {} | FPS: {}", SONG_BPM, fps));
                }

                let current_surface_extent = base.surface_resolution;
                let current_beat = game_state.current_beat;
                let arrows_clone = game_state.arrows.clone();
                let targets_clone = game_state.targets.clone();
                let flash_states_clone = game_state.flash_states.clone();

                match base.draw_frame(|device, cmd_buf| unsafe {
                    device.cmd_bind_pipeline(
                        cmd_buf,
                        vk::PipelineBindPoint::GRAPHICS,
                        graphics_pipeline,
                    );
                    device.cmd_bind_vertex_buffers(cmd_buf, 0, &[vertex_buffer.buffer], &[0]);
                    device.cmd_bind_index_buffer(
                        cmd_buf,
                        index_buffer.buffer,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_bind_descriptor_sets(
                        cmd_buf,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_layout,
                        0,
                        &[descriptor_set],
                        &[],
                    );
                    let viewport = vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: current_surface_extent.width as f32,
                        height: current_surface_extent.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    };
                    let scissor = vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: current_surface_extent,
                    };
                    device.cmd_set_viewport(cmd_buf, 0, &[viewport]);
                    device.cmd_set_scissor(cmd_buf, 0, &[scissor]);

                    let frame_index = ((current_beat + 0.0001) % 4.0) as usize;
                    let uv_width = 1.0 / 4.0;
                    let uv_x_start = frame_index as f32 * uv_width;
                    let uv_offset = [uv_x_start, 0.0];
                    let uv_scale = [uv_width, 1.0];

                    let now_for_flash = Instant::now();
                    for target in &targets_clone {
                        let current_tint = flash_states_clone
                            .get(&target.direction)
                            .filter(|flash| now_for_flash < flash.end_time)
                            .map_or(TARGET_TINT, |flash| flash.color);
                        let rotation_angle = match target.direction {
                            ArrowDirection::Down => Rad(0.0),
                            ArrowDirection::Left => Rad(PI / 2.0),
                            ArrowDirection::Up => Rad(PI),
                            ArrowDirection::Right => Rad(-PI / 2.0),
                        };
                        let model_matrix =
                            Matrix4::from_translation(Vector3::new(target.x, target.y, 0.0))
                                * Matrix4::from_angle_z(rotation_angle)
                                * Matrix4::from_nonuniform_scale(TARGET_SIZE, TARGET_SIZE, 1.0);
                        let push_data = PushConstantData {
                            model: model_matrix,
                            color: current_tint,
                            uv_offset,
                            uv_scale,
                        };
                        let push_data_bytes = std::slice::from_raw_parts(
                            &push_data as *const _ as *const u8,
                            mem::size_of::<PushConstantData>(),
                        );
                        device.cmd_push_constants(
                            cmd_buf,
                            pipeline_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0,
                            push_data_bytes,
                        );
                        device.cmd_draw_indexed(cmd_buf, quad_indices.len() as u32, 1, 0, 0, 0);
                    }
                    for column in arrows_clone.values() {
                        for arrow in column {
                            let arrow_tint = match arrow.note_type {
                                NoteType::Quarter => ARROW_TINT_QUARTER,
                                NoteType::Eighth => ARROW_TINT_EIGHTH,
                                NoteType::Sixteenth => ARROW_TINT_SIXTEENTH,
                            };
                            let rotation_angle = match arrow.direction {
                                ArrowDirection::Down => Rad(0.0),
                                ArrowDirection::Left => Rad(PI / 2.0),
                                ArrowDirection::Up => Rad(PI),
                                ArrowDirection::Right => Rad(-PI / 2.0),
                            };
                            let model_matrix =
                                Matrix4::from_translation(Vector3::new(arrow.x, arrow.y, 0.0))
                                    * Matrix4::from_angle_z(rotation_angle)
                                    * Matrix4::from_nonuniform_scale(ARROW_SIZE, ARROW_SIZE, 1.0);
                            let push_data = PushConstantData {
                                model: model_matrix,
                                color: arrow_tint,
                                uv_offset,
                                uv_scale,
                            };
                            let push_data_bytes = std::slice::from_raw_parts(
                                &push_data as *const _ as *const u8,
                                mem::size_of::<PushConstantData>(),
                            );
                            device.cmd_push_constants(
                                cmd_buf,
                                pipeline_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                0,
                                push_data_bytes,
                            );
                            device.cmd_draw_indexed(cmd_buf, quad_indices.len() as u32, 1, 0, 0, 0);
                        }
                    }
                }) {
                    Ok(needs_resize_result) => {
                        if needs_resize_result {
                            resize_needed = true;
                        }
                    }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        resize_needed = true;
                    }
                    Err(e) => {
                        error!("Failed to draw frame: {:?}", e);
                        elwp.exit();
                    }
                }
                base.window.request_redraw();
            }
            _ => (),
        }
    })?;

    // --- Cleanup ---
    info!("Waiting for GPU idle...");
    unsafe { base.device.device_wait_idle()? };
    info!("GPU idle.");
    info!("Cleaning up main Vulkan resources...");
    unsafe {
        base.device.destroy_pipeline(graphics_pipeline, None);
        base.device.destroy_pipeline_layout(pipeline_layout, None);
        base.device.destroy_descriptor_pool(descriptor_pool, None);
        base.device
            .destroy_descriptor_set_layout(descriptor_set_layout, None);
        vertex_buffer.destroy(&base.device);
        index_buffer.destroy(&base.device);
        projection_ubo.destroy(&base.device);
        arrow_texture.destroy(&base.device);
    }
    info!("Main Vulkan resources cleaned up.");
    info!("Exiting application.");
    Ok(())
}

// --- Game Logic Functions ---
fn initialize_game_state(win_w: f32, win_h: f32) -> GameState {
    info!(
        "Initializing game state for window size: {}x{}",
        win_w, win_h
    );
    let center_x = win_w / 2.0;
    let target_spacing = ARROW_SIZE * 1.2;
    let total_width = ARROW_DIRECTIONS.len() as f32 * target_spacing - target_spacing * 0.2;
    let start_x_slot_center = center_x - total_width / 2.0 + target_spacing / 2.0;
    let targets = ARROW_DIRECTIONS
        .iter()
        .enumerate()
        .map(|(i, &dir)| TargetInfo {
            x: start_x_slot_center + i as f32 * target_spacing,
            y: TARGET_Y_POS,
            direction: dir,
        })
        .collect();
    let mut arrows = HashMap::new();
    for dir in ARROW_DIRECTIONS.iter() {
        arrows.insert(*dir, Vec::new());
    }
    let offset_seconds = AUDIO_SYNC_OFFSET_MS as f32 / 1000.0;
    let beat_offset = offset_seconds * (SONG_BPM / 60.0);
    let initial_beat = -beat_offset;
    info!(
        "Audio Sync Offset: {} ms -> Initial Beat: {:.4}",
        AUDIO_SYNC_OFFSET_MS, initial_beat
    );
    let initial_last_spawned_16th_index = (initial_beat * 4.0 - 1.0).floor() as i32;
    info!(
        "Initial last spawned 16th index: {}",
        initial_last_spawned_16th_index
    );
    // Initialize with default ModifiersState
    GameState {
        targets,
        arrows,
        pressed_keys: HashSet::new(),
        last_spawned_16th_index: initial_last_spawned_16th_index,
        last_spawned_direction: None,
        current_beat: initial_beat,
        window_size: (win_w, win_h),
        flash_states: HashMap::new(),
        modifiers: ModifiersState::default(),
    }
}

fn handle_input(event: &WindowEvent, state: &mut GameState, elwp: &EventLoopWindowTarget<()>) {
    match event {
        WindowEvent::CloseRequested => {
            info!("Close requested, exiting.");
            elwp.exit();
        }
        // The event provides winit::event::Modifiers
        WindowEvent::ModifiersChanged(modifiers) => {
            trace!("Modifiers changed event: {:?}", modifiers);
            // Call .state() to get the ModifiersState and assign it
            state.modifiers = modifiers.state();
            trace!("GameState modifiers updated to: {:?}", state.modifiers);
        }
        WindowEvent::KeyboardInput {
            event: key_event, ..
        } => {
            if let winit::keyboard::Key::Named(named_key) = key_event.logical_key {
                let virtual_keycode = key_to_virtual_keycode(named_key);
                if let Some(keycode) = virtual_keycode {
                    match key_event.state {
                        ElementState::Pressed => {
                            if state.pressed_keys.insert(keycode) {
                                check_hits_on_press(state, keycode);
                            }
                        }
                        ElementState::Released => {
                            state.pressed_keys.remove(&keycode);
                        }
                    }
                } else if key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                    && key_event.state == ElementState::Pressed
                {
                    elwp.exit();
                }
            } else if key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                && key_event.state == ElementState::Pressed
            {
                elwp.exit();
            }
        }
        _ => {}
    }
}

fn key_to_virtual_keycode(key: winit::keyboard::NamedKey) -> Option<VirtualKeyCode> {
    use winit::keyboard::NamedKey::*;
    match key {
        ArrowLeft => Some(VirtualKeyCode::Left),
        ArrowDown => Some(VirtualKeyCode::Down),
        ArrowUp => Some(VirtualKeyCode::Up),
        ArrowRight => Some(VirtualKeyCode::Right),
        Escape => Some(VirtualKeyCode::Escape),
        _ => None,
    }
}

fn check_hits_on_press(state: &mut GameState, keycode: VirtualKeyCode) {
    let direction = match keycode {
        VirtualKeyCode::Left => Some(ArrowDirection::Left),
        VirtualKeyCode::Down => Some(ArrowDirection::Down),
        VirtualKeyCode::Up => Some(ArrowDirection::Up),
        VirtualKeyCode::Right => Some(ArrowDirection::Right),
        _ => None,
    };
    if let Some(dir) = direction {
        if let Some(column_arrows) = state.arrows.get_mut(&dir) {
            let current_beat = state.current_beat;
            let seconds_per_beat = 60.0 / SONG_BPM;
            let mut best_hit_idx: Option<usize> = None;
            let mut min_time_diff_ms = f32::INFINITY;
            for (idx, arrow) in column_arrows.iter().enumerate() {
                let beat_diff = current_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();
                if abs_time_diff_ms <= MAX_HIT_WINDOW_MS && abs_time_diff_ms < min_time_diff_ms {
                    min_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }
            if let Some(idx) = best_hit_idx {
                let judgment = if min_time_diff_ms <= W1_WINDOW_MS {
                    Judgment::W1
                } else if min_time_diff_ms <= W2_WINDOW_MS {
                    Judgment::W2
                } else if min_time_diff_ms <= W3_WINDOW_MS {
                    Judgment::W3
                } else {
                    Judgment::W4
                };
                let hit_arrow = &column_arrows[idx];
                let time_diff_for_log =
                    (current_beat - hit_arrow.target_beat) * seconds_per_beat * 1000.0;
                info!(
                    "HIT! {:?} {:?}. Time Diff: {:.1}ms -> {:?}",
                    dir, hit_arrow.note_type, time_diff_for_log, judgment
                );
                let flash_color = match judgment {
                    Judgment::W1 => FLASH_COLOR_W1,
                    Judgment::W2 => FLASH_COLOR_W2,
                    Judgment::W3 => FLASH_COLOR_W3,
                    Judgment::W4 => FLASH_COLOR_W4,
                    Judgment::Miss => unreachable!(),
                };
                let flash_end_time = Instant::now() + FLASH_DURATION;
                state.flash_states.insert(
                    dir,
                    FlashState {
                        color: flash_color,
                        end_time: flash_end_time,
                    },
                );
                column_arrows.remove(idx);
            } else {
                debug!(
                    "Input {:?} registered, but no arrow within hit window.",
                    keycode
                );
            }
        }
    }
}

fn update_game_state(state: &mut GameState, dt: f32, rng: &mut impl Rng) {
    let beat_delta = dt * (SONG_BPM / 60.0);
    state.current_beat += beat_delta;
    let seconds_per_beat = 60.0 / SONG_BPM;
    let target_16th_index = ((state.current_beat + SPAWN_LOOKAHEAD_BEATS) * 4.0).floor() as i32;

    if target_16th_index > state.last_spawned_16th_index {
        for i in (state.last_spawned_16th_index + 1)..=target_16th_index {
            let target_beat = i as f32 / 4.0;
            let note_type = match i % 4 {
                0 => NoteType::Quarter,
                2 => NoteType::Eighth,
                _ => NoteType::Sixteenth,
            };
            let should_spawn = match DIFFICULTY {
                0 => note_type == NoteType::Quarter,
                1 => {
                    note_type == NoteType::Quarter
                        || (note_type == NoteType::Eighth && rng.random::<bool>())
                }
                2 => note_type == NoteType::Quarter || note_type == NoteType::Eighth,
                _ => true,
            };
            if !should_spawn {
                continue;
            }
            let beats_remaining = target_beat - state.current_beat;
            if beats_remaining <= 0.0 {
                continue;
            }
            let time_to_target_s = beats_remaining * seconds_per_beat;
            let distance_to_travel = ARROW_SPEED * time_to_target_s;
            let spawn_y = TARGET_Y_POS + distance_to_travel;
            if spawn_y <= TARGET_Y_POS + (ARROW_SIZE * 0.5) {
                continue;
            }

            let dir: ArrowDirection;
            if DIFFICULTY >= 3 && state.last_spawned_direction.is_some() {
                let mut available_dirs: Vec<ArrowDirection> = ARROW_DIRECTIONS
                    .iter()
                    .copied()
                    .filter(|&d| Some(d) != state.last_spawned_direction)
                    .collect();
                if available_dirs.is_empty() {
                    available_dirs = ARROW_DIRECTIONS.to_vec();
                }
                dir = *available_dirs.choose(rng).unwrap_or(&ARROW_DIRECTIONS[0]);
            } else {
                dir = ARROW_DIRECTIONS[rng.random_range(0..ARROW_DIRECTIONS.len())];
            }
            let target_x = state
                .targets
                .iter()
                .find(|t| t.direction == dir)
                .map(|t| t.x)
                .unwrap_or(state.window_size.0 / 2.0);
            if let Some(column_arrows) = state.arrows.get_mut(&dir) {
                column_arrows.push(Arrow {
                    x: target_x,
                    y: spawn_y,
                    direction: dir,
                    note_type,
                    target_beat,
                });
                state.last_spawned_direction = Some(dir);
            }
        }
        state.last_spawned_16th_index = target_16th_index;
    }

    for column_arrows in state.arrows.values_mut() {
        for arrow in column_arrows.iter_mut() {
            arrow.y -= ARROW_SPEED * dt;
        }
    }

    let miss_window_beats = (MISS_WINDOW_MS / 1000.0) / seconds_per_beat;
    for (_dir, column_arrows) in state.arrows.iter_mut() {
        column_arrows.retain(|arrow| {
            if state.current_beat > arrow.target_beat + miss_window_beats {
                info!(
                    "MISSED! {:?} {:?} (Target Beat: {:.2}, Current: {:.2}, Diff: {:.1}ms)",
                    arrow.direction,
                    arrow.note_type,
                    arrow.target_beat,
                    state.current_beat,
                    (state.current_beat - arrow.target_beat - miss_window_beats)
                        * seconds_per_beat
                        * 1000.0
                );
                false
            } else {
                true
            }
        });
    }

    let now_for_cleanup = Instant::now();
    state
        .flash_states
        .retain(|_dir, flash| now_for_cleanup < flash.end_time);
}

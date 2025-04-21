// main.rs
use ash::vk;
use cgmath::{ortho, Matrix4, Rad, Vector3};
use log::{debug, error, info, trace, warn, LevelFilter}; // Added warn
use rand::distr::{Bernoulli, Distribution}; // Added Bernoulli + Distribution
use rand::prelude::IndexedRandom;
use rand::Rng;
use rodio::{Decoder, OutputStream, Sink}; // Removed unused source::Source
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
    event::{ElementState, Event, KeyEvent, WindowEvent}, // Added KeyEvent
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey}, // Added Key, NamedKey
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::WindowBuilder,
};

use memoffset::offset_of; // Import offset_of macro

mod texture;
mod utils;
mod vulkan_base;

use texture::{load_texture, TextureResource}; // Make TextureResource public if needed, or keep using load_texture's return type directly
use utils::fps::FPSCounter;
use vulkan_base::{BufferResource, UniformBufferObject, Vertex, VulkanBase}; // Added BufferResource

// --- Constants ---
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
const DIFFICULTY: u32 = 2; // 0: Quarters, 1: Q+Random Eighths, 2: Q+E, 3: Q+E+S+AvoidLast
const W1_WINDOW_MS: f32 = 22.5;
const W2_WINDOW_MS: f32 = 45.0;
const W3_WINDOW_MS: f32 = 90.0;
const W4_WINDOW_MS: f32 = 135.0;
const MAX_HIT_WINDOW_MS: f32 = 180.0;
const MISS_WINDOW_MS: f32 = 200.0; // Time after target beat arrow is considered missed

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteType {
    Quarter,
    Eighth,
    Sixteenth,
}

const TARGET_TINT: [f32; 4] = [0.7, 0.7, 0.7, 0.5];
const ARROW_TINT_QUARTER: [f32; 4] = [1.0, 0.6, 0.6, 1.0]; // Red-ish
const ARROW_TINT_EIGHTH: [f32; 4] = [0.6, 0.6, 1.0, 1.0]; // Blue-ish
const ARROW_TINT_SIXTEENTH: [f32; 4] = [0.6, 1.0, 0.6, 1.0]; // Green-ish

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judgment {
    W1, // Marvelous
    W2, // Perfect
    W3, // Great
    W4, // Good
    Miss,
}

const FLASH_COLOR_W1: [f32; 4] = [0.2, 0.7, 1.0, 0.9]; // Bright Blue
const FLASH_COLOR_W2: [f32; 4] = [1.0, 0.8, 0.2, 0.9]; // Bright Yellow
const FLASH_COLOR_W3: [f32; 4] = [0.2, 1.0, 0.2, 0.9]; // Bright Green
const FLASH_COLOR_W4: [f32; 4] = [0.8, 0.4, 1.0, 0.9]; // Purple
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

// --- Structs ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Menu,
    Gameplay,
}

#[derive(Debug, Clone)] // Added Clone
struct MenuState {
    options: Vec<String>,
    selected_index: usize,
}

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
    pressed_keys: HashSet<VirtualKeyCode>, // Keep using VirtualKeyCode for gameplay hits
    last_spawned_16th_index: i32,
    last_spawned_direction: Option<ArrowDirection>,
    current_beat: f32,
    window_size: (f32, f32),
    flash_states: HashMap<ArrowDirection, FlashState>,
    // --- Added for Menu Logic ---
    audio_sink: Option<Sink>,          // To control audio playback
    audio_start_time: Option<Instant>, // Track when audio actually started playing
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

// --- Main Function ---
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info) // Use Info level, Trace is very verbose
        .init();

    // --- Winit Setup ---
    info!("Initializing Winit...");
    let mut event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("DeadSync") // Initial title
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(WINDOW_WIDTH),
            f64::from(WINDOW_HEIGHT),
        ))
        .build(&event_loop)?;
    let initial_window_size = window.inner_size();

    // --- VulkanBase Setup ---
    info!("Initializing VulkanBase...");
    let mut base = VulkanBase::new(window)?; // Window is moved into base
    info!("VulkanBase Initialized for window: {:?}", base.window.id());
    info!("GPU: {}", base.get_gpu_name());

    // --- Application State ---
    let mut current_app_state = AppState::Menu;
    let mut menu_state = MenuState {
        options: vec!["Play!".to_string(), "Exit".to_string()],
        selected_index: 0,
    };
    let mut game_state: Option<GameState> = None; // Initialize gameplay state later
    let mut game_init_pending = false; // Flag to trigger game state init

    // --- Audio Setup (Preparation) ---
    info!("Preparing audio stream...");
    // We need the handle to create sinks later
    let (_stream, stream_handle) = OutputStream::try_default()?;
    info!("Audio stream handle obtained.");
    // Keep the path string for re-opening the file
    let audio_path_str = format!("{}/{}", SONG_FOLDER_PATH, SONG_AUDIO_FILENAME);
    let audio_path = Path::new(&audio_path_str);
    // Check existence early
    if !audio_path.exists() {
        return Err(format!("Audio file not found: {:?}", audio_path).into());
    }
    info!("Audio file path confirmed: {:?}", audio_path);
    // We will open/decode when entering gameplay state

    // --- RNG ---
    let mut rng = rand::rng();

    // --- Common Game Variables ---
    let mut fps_counter = FPSCounter::new();
    let mut last_frame_time = Instant::now();

    // --- Vulkan Resource Creation (Common for both states) ---
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
        // Use let, not mut, if not rebound
        vertex_buffer_size,
        vk::BufferUsageFlags::VERTEX_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    base.update_buffer(&vertex_buffer, &quad_vertices)?;

    let quad_indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
    let index_buffer_size = (quad_indices.len() * mem::size_of::<u32>()) as vk::DeviceSize;
    let mut index_buffer = base.create_buffer(
        // Use let, not mut
        index_buffer_size,
        vk::BufferUsageFlags::INDEX_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    base.update_buffer(&index_buffer, &quad_indices)?;

    let ubo_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
    let mut projection_ubo = base.create_buffer(
        // Use let, not mut
        ubo_size,
        vk::BufferUsageFlags::UNIFORM_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    // Check if the texture file exists and load it
    let arrow_texture_path = Path::new("assets/down_arrow_atlas.png"); //Must be executed from the code folder ./target/release/deadsync
    if !arrow_texture_path.exists() {
        error!(
            "Error: Texture file not found at {:?}",
            arrow_texture_path
                .canonicalize()
                .unwrap_or_else(|_| arrow_texture_path.to_path_buf())
        );
        // Attempt to print canonical path if possible
        return Err("Texture file not found.".into());
    }
    // Assuming TextureResource has a destroy method implemented
    let mut arrow_texture: TextureResource = load_texture(&base, arrow_texture_path)?;
    info!("Texture loaded successfully: {:?}", arrow_texture_path);

    // --- Descriptors, Pipeline Layout, Pipeline ---
    let dsl_bindings = [
        // Binding 0: Uniform Buffer (Vertex Shader)
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX),
        // Binding 1: Combined Image Sampler (Fragment Shader)
        vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT),
    ];
    let dsl_create_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&dsl_bindings);
    let descriptor_set_layout = unsafe {
        base.device
            .create_descriptor_set_layout(&dsl_create_info, None)?
    };

    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1, // Only need 1 UBO descriptor
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1, // Only need 1 texture sampler descriptor
        },
    ];
    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&pool_sizes)
        .max_sets(1); // Only need 1 descriptor set
    let descriptor_pool = unsafe {
        base.device
            .create_descriptor_pool(&pool_create_info, None)?
    };

    let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(std::slice::from_ref(&descriptor_set_layout));
    let descriptor_set = unsafe { base.device.allocate_descriptor_sets(&desc_alloc_info)?[0] };

    // Update descriptor set
    let ubo_buffer_info = vk::DescriptorBufferInfo::default()
        .buffer(projection_ubo.buffer)
        .offset(0)
        .range(vk::WHOLE_SIZE); // Use WHOLE_SIZE for clarity
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

    // Pipeline Layout
    let push_constant_ranges = [vk::PushConstantRange {
        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, // Used by both
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

    // Shaders
    let vert_shader_module = {
        // Scope to drop file handles
        let mut vert_shader_file = Cursor::new(&include_bytes!("../shaders/vert.spv")[..]);
        let vert_code = ash::util::read_spv(&mut vert_shader_file)?;
        let vert_module_info = vk::ShaderModuleCreateInfo::default().code(&vert_code);
        unsafe { base.device.create_shader_module(&vert_module_info, None)? }
    };
    let frag_shader_module = {
        // Scope to drop file handles
        let mut frag_shader_file = Cursor::new(&include_bytes!("../shaders/frag.spv")[..]);
        let frag_code = ash::util::read_spv(&mut frag_shader_file)?;
        let frag_module_info = vk::ShaderModuleCreateInfo::default().code(&frag_code);
        unsafe { base.device.create_shader_module(&frag_module_info, None)? }
    };

    let shader_entry_name = CString::new("main").unwrap(); // Ensure it ends with \0

    let shader_stage_create_infos = [
        vk::PipelineShaderStageCreateInfo::default()
            .module(vert_shader_module)
            .name(&shader_entry_name) // Use reference
            .stage(vk::ShaderStageFlags::VERTEX),
        vk::PipelineShaderStageCreateInfo::default()
            .module(frag_shader_module)
            .name(&shader_entry_name) // Use reference
            .stage(vk::ShaderStageFlags::FRAGMENT),
    ];

    // Pipeline Configuration
    let binding_descriptions = [vk::VertexInputBindingDescription {
        binding: 0,
        stride: mem::size_of::<Vertex>() as u32,
        input_rate: vk::VertexInputRate::VERTEX,
    }];

    let attribute_descriptions = [
        // Position
        vk::VertexInputAttributeDescription {
            location: 0, // Corresponds to layout(location=0) in vertex shader
            binding: 0,
            format: vk::Format::R32G32_SFLOAT, // vec2
            offset: offset_of!(Vertex, pos) as u32,
        },
        // Texture Coordinate
        vk::VertexInputAttributeDescription {
            location: 1, // Corresponds to layout(location=1) in vertex shader
            binding: 0,
            format: vk::Format::R32G32_SFLOAT, // vec2
            offset: offset_of!(Vertex, tex_coord) as u32,
        },
    ];

    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&binding_descriptions)
        .vertex_attribute_descriptions(&attribute_descriptions);

    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1) // We'll set these dynamically
        .scissor_count(1);

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE) // No culling for 2D sprites
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE); // Match quad winding

    let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1); // No MSAA

    // Enable alpha blending
    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO) // Blend alpha channel based on source
        .alpha_blend_op(vk::BlendOp::ADD);

    let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false) // No logic op needed
        .attachments(std::slice::from_ref(&color_blend_attachment));

    // No depth testing needed for this simple 2D setup
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
        .depth_stencil_state(&depth_stencil_state) // Add depth state
        .layout(pipeline_layout)
        .render_pass(base.render_pass) // Use the base render pass
        .subpass(0)
        .dynamic_state(&dynamic_state_info); // Enable dynamic states

    let graphics_pipeline = unsafe {
        base.device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &[pipeline_info], // Pass as slice
            None,
        )
        .map_err(|(_, err)| err)? // Handle pipeline creation error
        [0] // Get the first pipeline from the result vector
    };

    // Clean up shader modules after pipeline creation
    unsafe {
        base.device.destroy_shader_module(vert_shader_module, None);
        base.device.destroy_shader_module(frag_shader_module, None);
    }
    info!("Graphics Pipeline Created.");

    // --- Initial Projection Matrix Update ---
    let mut current_window_size = (
        initial_window_size.width as f32,
        initial_window_size.height as f32,
    );
    // Pass projection_ubo immutably
    update_projection_matrix(&mut base, &projection_ubo, current_window_size)?;

    // --- Event Loop ---
    info!("Starting Event Loop...");
    let mut resize_needed = false;
    let mut modifiers_state = ModifiersState::default(); // Track modifiers globally
                                                         // Moved outside the closure
    let mut next_app_state: Option<AppState> = None;

    // Use into_run_on_demand if available and preferred for WGPU/Web, but run_on_demand works too
    event_loop.run_on_demand(|event, elwp| {
        elwp.set_control_flow(ControlFlow::Poll); // Poll for continuous updates

        match event {
            Event::WindowEvent { event, window_id } if window_id == base.window.id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        info!("Close requested, exiting.");
                        elwp.exit();
                    }
                    WindowEvent::Resized(new_size) => {
                        log::info!("Window resized event: {:?}", new_size);
                        if new_size.width > 0 && new_size.height > 0 {
                            current_window_size = (new_size.width as f32, new_size.height as f32);
                            // Update game state window size if it exists
                            if let Some(ref mut gs) = game_state {
                                gs.window_size = current_window_size;
                            }
                            resize_needed = true; // Signal Vulkan needs swapchain recreation
                        }
                    }
                    WindowEvent::ModifiersChanged(modifiers) => {
                        modifiers_state = modifiers.state();
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        // Route input based on state
                        match current_app_state {
                            AppState::Menu => {
                                // Assign return value to the persistent next_app_state
                                if let Some(requested_state) = handle_menu_input(key_event, &mut menu_state, elwp) {
                                     debug!("Menu input requested state change to: {:?}", requested_state);
                                     next_app_state = Some(requested_state);
                                 }
                            }
                            AppState::Gameplay => {
                                if let Some(ref mut gs) = game_state {
                                     // Assign return value to the persistent next_app_state
                                     if let Some(requested_state) = handle_gameplay_input(key_event, gs, modifiers_state, elwp) {
                                          debug!("Gameplay input requested state change to: {:?}", requested_state);
                                          next_app_state = Some(requested_state);
                                      }
                                }
                            }
                        }
                    }
                     _ => {} // Ignore other window events like mouse input for now
                }
            }
            Event::AboutToWait => { // Best place for updates and drawing
                // --- State Transition Logic ---
                // Now checks the persistent next_app_state
                if let Some(new_state) = next_app_state.take() { // Use take() to consume the request
                    debug!("AboutToWait: Processing requested state transition to {:?}", new_state);
                    if new_state != current_app_state {
                        debug!("AboutToWait: new_state ({:?}) != current_app_state ({:?}). Proceeding.", new_state, current_app_state);
                        match (current_app_state, new_state) {
                            (AppState::Menu, AppState::Gameplay) => {
                                info!("Transitioning Menu -> Gameplay");
                                game_init_pending = true;
                                if let Some(ref mut gs) = game_state {
                                    if let Some(sink) = gs.audio_sink.take() {
                                        sink.stop();
                                    }
                                }
                                game_state = None;
                            }
                            (AppState::Gameplay, AppState::Menu) => {
                                info!("Transitioning Gameplay -> Menu");
                                if let Some(ref mut gs) = game_state {
                                    if let Some(sink) = gs.audio_sink.take() {
                                        info!("Stopping gameplay audio.");
                                        sink.stop();
                                        // sink is dropped here
                                    }
                                }
                                game_state = None; // Clear gameplay state
                                base.window.set_title("DeadSync");
                                menu_state.selected_index = 0; // Reset menu selection
                            }
                             _ => { warn!("Unexpected state transition requested from {:?} to {:?}", current_app_state, new_state); }
                        }
                        debug!("AboutToWait: Setting current_app_state = {:?}", new_state);
                        current_app_state = new_state; // Update current state
                    } else {
                         info!("AboutToWait: State transition requested, but new_state ({:?}) == current_app_state ({:?}). Ignoring.", new_state, current_app_state);
                    }
                } // End: if let Some(new_state)

                // --- Initialize Game State if Pending ---
                 debug!("AboutToWait: Checking game_init_pending ({}) && current_app_state ({:?}) == Gameplay", game_init_pending, current_app_state);
                if game_init_pending && current_app_state == AppState::Gameplay {
                    info!("Initializing Game State...");
                    // --- Start Audio for Gameplay ---
                    let file = match File::open(&audio_path) {
                         Ok(f) => f,
                         Err(e) => { error!("Failed to open audio: {}",e); game_init_pending=false; current_app_state=AppState::Menu; base.window.request_redraw(); return; }
                    };
                    let source = match Decoder::new(BufReader::new(file)) {
                         Ok(s) => s,
                         Err(e) => { error!("Failed to decode audio: {}",e); game_init_pending=false; current_app_state=AppState::Menu; base.window.request_redraw(); return; }
                    };
                    let sink = match Sink::try_new(&stream_handle) {
                         Ok(s) => s,
                         Err(e) => { error!("Failed to create sink: {}",e); game_init_pending=false; current_app_state=AppState::Menu; base.window.request_redraw(); return; }
                    };

                    sink.append(source);
                    sink.play();
                    let audio_start_time = Instant::now();
                    // Do NOT detach sink

                    game_state = Some(initialize_game_state(
                        current_window_size.0,
                        current_window_size.1,
                        Some(sink), // Pass the controllable sink
                        Some(audio_start_time),
                    ));
                    info!("Gameplay initialized and audio started at {:?}", audio_start_time);
                    game_init_pending = false; // Reset flag
                }

                 // --- Handle Resize ---
                 if resize_needed {
                    // IMPORTANT: Proper resize handling requires recreating the swapchain, etc.
                    log::warn!("Resize detected - Vulkan swapchain recreation NOT IMPLEMENTED! Graphics may be distorted.");
                    // Pass projection_ubo immutably
                    if let Err(e) = update_projection_matrix(&mut base, &projection_ubo, current_window_size) {
                         error!("Failed to update projection UBO after resize: {}", e);
                     } else {
                         info!("Projection matrix updated for new size: {:?}", current_window_size);
                     }
                    // Ideally, call base.recreate_swapchain() here.
                    resize_needed = false; // Reset flag after handling
                 }

                // --- Update Logic ---
                let now = Instant::now();
                // Calculate delta time, ensuring it's not negative and capping max value
                let dt = (now - last_frame_time).as_secs_f32().max(0.0).min(0.1);
                last_frame_time = now;

                // Update based on the current state
                match current_app_state {
                    AppState::Menu => {
                        // No menu state updates needed for this simple version
                        if let Some(fps) = fps_counter.update() {
                            base.window.set_title(&format!("DeadSync | FPS: {}", fps));
                        }
                    }
                    AppState::Gameplay => {
                        if let Some(ref mut gs) = game_state {
                            update_game_state(gs, dt, &mut rng);
                            if let Some(fps) = fps_counter.update() {
                                 base.window.set_title(&format!("DeadSync | BPM: {} | FPS: {}", SONG_BPM, fps));
                             }
                        } else {
                             // This case should ideally not happen if init logic is correct
                             error!("In Gameplay state but game_state is None!");
                        }
                    }
                }


                // --- Drawing ---
                let current_surface_extent = base.surface_resolution; // Needed for viewport/scissor

                // Clone necessary data for the drawing closure *before* calling draw_frame
                let menu_state_clone = if current_app_state == AppState::Menu { Some(menu_state.clone()) } else { None };
                let game_state_data_for_draw = if current_app_state == AppState::Gameplay {
                    game_state.as_ref().map(|gs| {
                        // Only clone data needed for drawing
                        (gs.current_beat, gs.arrows.clone(), gs.targets.clone(), gs.flash_states.clone())
                    })
                 } else {
                     None
                 };
                let app_state_for_draw = current_app_state; // Copy the enum state

                // The draw_frame closure captures the environment
                match base.draw_frame(|device, cmd_buf| {
                    // Common setup for both states
                    unsafe {
                        device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, graphics_pipeline);
                        device.cmd_bind_vertex_buffers(cmd_buf, 0, &[vertex_buffer.buffer], &[0]);
                        device.cmd_bind_index_buffer(cmd_buf, index_buffer.buffer, 0, vk::IndexType::UINT32);
                        device.cmd_bind_descriptor_sets(
                            cmd_buf,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline_layout,
                            0, // set number
                            &[descriptor_set],
                            &[], // no dynamic offsets
                        );

                        // Set viewport and scissor dynamically
                        let viewport = vk::Viewport {
                            x: 0.0,
                            y: 0.0, // Flip Y if needed based on projection, but ortho handles it here
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
                    }

                    // State-specific drawing logic using the cloned data
                    match app_state_for_draw {
                        AppState::Menu => {
                            if let Some(ms) = menu_state_clone { // Use cloned menu state
                                draw_menu(device, cmd_buf, pipeline_layout, &ms, current_window_size, quad_indices.len() as u32);
                            }
                        }
                        AppState::Gameplay => {
                             if let Some((current_beat, arrows, targets, flash_states)) = game_state_data_for_draw { // Use cloned game data
                                 draw_gameplay(device, cmd_buf, pipeline_layout, current_beat, &arrows, &targets, &flash_states, quad_indices.len() as u32);
                             }
                        }
                    }
                }) {
                    Ok(needs_resize_result) => {
                        // Flag resize based on presentation result (suboptimal or out of date)
                        if needs_resize_result { resize_needed = true; }
                    }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        // Explicitly handle out of date error
                        resize_needed = true;
                    }
                    Err(e) => {
                        // Handle other drawing errors
                        error!("Failed to draw frame: {:?}", e);
                        elwp.exit(); // Exit on critical draw error
                    }
                }
                base.window.request_redraw(); // Request redraw continuously for animation/updates
            }
            _ => (), // Ignore other events like DeviceEvent
        }
    })?; // Propagate errors from event loop run

    // --- Cleanup ---
    info!("Waiting for GPU idle before cleanup...");
    unsafe { base.device.device_wait_idle()? }; // Wait for all GPU commands to finish
    info!("GPU idle.");
    info!("Cleaning up application resources...");

    // Explicitly stop audio if game state exists and has a sink
    if let Some(mut gs) = game_state.take() {
        // Take ownership to drop sink
        if let Some(sink) = gs.audio_sink.take() {
            info!("Stopping game audio sink.");
            sink.stop();
            // Sink is dropped here
        }
    }

    // Clean up Vulkan resources created in main
    unsafe {
        info!("Destroying graphics pipeline...");
        base.device.destroy_pipeline(graphics_pipeline, None);
        info!("Destroying pipeline layout...");
        base.device.destroy_pipeline_layout(pipeline_layout, None);
        info!("Destroying descriptor pool...");
        base.device.destroy_descriptor_pool(descriptor_pool, None); // Destroys sets allocated from it
        info!("Destroying descriptor set layout...");
        base.device
            .destroy_descriptor_set_layout(descriptor_set_layout, None);
        info!("Destroying vertex buffer...");
        // Call destroy method if BufferResource implements Drop or has one
        // Assuming BufferResource needs explicit destruction:
        vertex_buffer.destroy(&base.device);
        info!("Destroying index buffer...");
        index_buffer.destroy(&base.device);
        info!("Destroying projection UBO...");
        projection_ubo.destroy(&base.device);
        info!("Destroying arrow texture...");
        arrow_texture.destroy(&base.device); // Assuming TextureResource has destroy
    }

    // VulkanBase's Drop implementation handles the rest (device, instance, swapchain, etc.)
    info!("Main Vulkan resources cleaned up.");
    info!("Exiting application.");
    Ok(()) // Return Ok if everything finished cleanly
}

// --- Rest of the functions (update_projection_matrix, etc.) remain the same ---
// ... make sure all the other functions like initialize_game_state, update_game_state,
// check_hits_on_press, draw_menu, draw_gameplay, key_to_virtual_keycode,
// update_projection_matrix are present below this line ...

// --- Helper to update projection matrix ---
fn update_projection_matrix(
    base: &mut VulkanBase,       // Needs access to device for update_buffer
    ubo_buffer: &BufferResource, // Changed to non-mutable borrow, update happens inside base
    window_size: (f32, f32),
) -> Result<(), Box<dyn Error>> {
    // Orthographic projection: maps world coords (0,0) top-left to (width, height) bottom-right
    // to Vulkan normalized device coords (-1,-1) bottom-left to (1,1) top-right.
    // Vulkan's Y-axis points down in NDC, but ortho setup maps (0,0) to top-left, (0, height) to bottom-left.
    let proj = ortho(
        0.0,           // left
        window_size.0, // right
        0.0,           // bottom (world Y=0)
        window_size.1, // top    (world Y=height)
        -1.0,          // near (closer to viewer in Z) - Choose appropriate range
        1.0,           // far (further from viewer in Z)
    );
    let ubo = UniformBufferObject { projection: proj };
    base.update_buffer(ubo_buffer, &[ubo])?; // update_buffer needs mutable self? Check impl. If not, base can be &
    Ok(())
}

// --- Menu Input Handler ---
fn handle_menu_input(
    key_event: KeyEvent, // Use winit KeyEvent directly
    menu_state: &mut MenuState,
    elwp: &EventLoopWindowTarget<()>, // To exit the app
) -> Option<AppState> {
    // Return Option<AppState> to signal state change request
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        // Handle press, ignore repeats
        match key_event.logical_key {
            Key::Named(NamedKey::ArrowUp) => {
                menu_state.selected_index = if menu_state.selected_index == 0 {
                    menu_state.options.len() - 1
                } else {
                    menu_state.selected_index - 1
                };
                debug!("Menu Up: Selected index {}", menu_state.selected_index);
            }
            Key::Named(NamedKey::ArrowDown) => {
                menu_state.selected_index =
                    (menu_state.selected_index + 1) % menu_state.options.len();
                debug!("Menu Down: Selected index {}", menu_state.selected_index);
            }
            Key::Named(NamedKey::Enter) => {
                debug!("Menu Enter: Selected index {}", menu_state.selected_index);
                match menu_state.selected_index {
                    0 => return Some(AppState::Gameplay), // Request transition to Gameplay
                    1 => elwp.exit(),                     // Exit the application directly
                    _ => {}                               // Should not happen with 2 options
                }
            }
            Key::Named(NamedKey::Escape) => {
                // Escape in menu also exits the application
                debug!("Menu Escape: Exiting");
                elwp.exit();
            }
            _ => {} // Ignore other keys in the menu
        }
    }
    None // No state change requested by default
}

// --- Gameplay Input Handler (Modified handle_input) ---
fn handle_gameplay_input(
    key_event: KeyEvent, // Use winit KeyEvent
    state: &mut GameState,
    _modifiers: ModifiersState, // Passed in, but not used in hit checking logic currently
    _elwp: &EventLoopWindowTarget<()>, // Keep for potential future use (e.g., pause menu)
) -> Option<AppState> {
    // Return Option<AppState> to signal state change request

    // Check for Escape first to transition back to menu
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if key_event.logical_key == Key::Named(NamedKey::Escape)
            || key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
        {
            info!("Escape pressed in gameplay, returning to menu.");
            return Some(AppState::Menu); // Request transition back to Menu
        }
    }

    // Handle gameplay key presses/releases for hits
    if let Key::Named(named_key) = key_event.logical_key {
        if let Some(virtual_keycode) = key_to_virtual_keycode(named_key) {
            match key_event.state {
                ElementState::Pressed => {
                    // Use HashSet::insert's return value to check if it was newly inserted
                    if state.pressed_keys.insert(virtual_keycode) {
                        trace!(
                            "Gameplay Key Pressed: {:?}, checking hits.",
                            virtual_keycode
                        );
                        check_hits_on_press(state, virtual_keycode);
                    } else {
                        trace!("Gameplay Key Repeat/Held: {:?}", virtual_keycode);
                    }
                }
                ElementState::Released => {
                    if state.pressed_keys.remove(&virtual_keycode) {
                        trace!("Gameplay Key Released: {:?}", virtual_keycode);
                    }
                }
            }
        }
    } // Ignore character input or other keys for gameplay actions

    None // No state change requested by default from gameplay actions
}

// --- Game Initialization (Modified) ---
fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_sink: Option<Sink>,          // Accept the sink
    audio_start_time: Option<Instant>, // Accept the start time
) -> GameState {
    info!(
        "Initializing game state for window size: {}x{}",
        win_w, win_h
    );
    let center_x = win_w / 2.0;
    let target_spacing = TARGET_SIZE * 1.2; // Use TARGET_SIZE for spacing too
    let total_width = (ARROW_DIRECTIONS.len() as f32 * target_spacing) - (target_spacing * 0.2); // Slightly reduce spacing
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

    // Initial beat calculation depends on audio start time, but we need a value now.
    // Calculate based on the offset relative to the expected audio start.
    // update_game_state will refine this based on actual elapsed audio time.
    let initial_beat = -(AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) * (SONG_BPM / 60.0);

    info!(
        "Audio Sync Offset: {} ms -> Estimated Initial Beat: {:.4}",
        AUDIO_SYNC_OFFSET_MS, initial_beat
    );

    // Calculate the index of the last 16th note *before* the initial beat.
    let initial_last_spawned_16th_index = (initial_beat * 4.0 - 1.0).floor() as i32;
    info!(
        "Initial last spawned 16th index: {}",
        initial_last_spawned_16th_index
    );

    GameState {
        targets,
        arrows,
        pressed_keys: HashSet::new(),
        last_spawned_16th_index: initial_last_spawned_16th_index,
        last_spawned_direction: None,
        current_beat: initial_beat, // Start relative to audio sync offset
        window_size: (win_w, win_h),
        flash_states: HashMap::new(),
        audio_sink,       // Store the sink
        audio_start_time, // Store the start time
    }
}

// --- Game State Update (Modified) ---
fn update_game_state(state: &mut GameState, dt: f32, rng: &mut impl Rng) {
    // Calculate current beat based on precise audio time if available
    if let Some(start_time) = state.audio_start_time {
        let elapsed_since_audio_start = Instant::now().duration_since(start_time).as_secs_f32();
        // Current beat = (time since audio start * beats per second) - initial offset beats
        let beat_offset = (AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) * (SONG_BPM / 60.0);
        state.current_beat = elapsed_since_audio_start * (SONG_BPM / 60.0) - beat_offset;
        // trace!("Audio time: {:.3}s, Beat: {:.3}", elapsed_since_audio_start, state.current_beat); // Can be verbose
    } else {
        // Fallback: If audio hasn't started or we lost track, update based on frame dt. Less accurate.
        let beat_delta = dt * (SONG_BPM / 60.0);
        state.current_beat += beat_delta;
        warn!("Updating beat based on dt, audio start time not available or audio not playing.");
    }

    let seconds_per_beat = 60.0 / SONG_BPM;

    // --- Arrow Spawning Logic ---
    // Determine the target 16th note index based on lookahead
    let target_16th_index = ((state.current_beat + SPAWN_LOOKAHEAD_BEATS) * 4.0).floor() as i32;

    // Spawn arrows for any 16th notes between the last spawned and the target
    if target_16th_index > state.last_spawned_16th_index {
        let bernoulli_half = Bernoulli::new(0.5).unwrap(); // Pre-create distribution

        for i in (state.last_spawned_16th_index + 1)..=target_16th_index {
            let target_beat = i as f32 / 4.0;

            // Determine note type based on the 16th index
            let note_type = match i % 4 {
                0 => NoteType::Quarter,   // On the beat
                2 => NoteType::Eighth,    // Off-beat eighth
                _ => NoteType::Sixteenth, // 1st and 3rd sixteenths
            };

            // Determine if the note should spawn based on difficulty
            let should_spawn = match DIFFICULTY {
                0 => note_type == NoteType::Quarter, // Only quarter notes
                // Use Bernoulli distribution sample method
                1 => {
                    note_type == NoteType::Quarter
                        || (note_type == NoteType::Eighth && bernoulli_half.sample(rng))
                }
                2 => note_type == NoteType::Quarter || note_type == NoteType::Eighth, // Quarters and Eighths
                3 | _ => true, // All note types (difficulty 3+), also default
            };

            if !should_spawn {
                continue; // Skip this note if difficulty prevents it
            }

            // --- Calculate Spawn Position ---
            let beats_remaining = target_beat - state.current_beat;
            // Don't spawn arrows that should have already passed the target
            if beats_remaining <= 0.0 {
                trace!(
                    "Skipping spawn for past beat {:.2} (current: {:.2})",
                    target_beat,
                    state.current_beat
                );
                continue;
            }

            let time_to_target_s = beats_remaining * seconds_per_beat;
            let distance_to_travel = ARROW_SPEED * time_to_target_s;
            let spawn_y = TARGET_Y_POS + distance_to_travel;

            // Optional: Add a small buffer to avoid spawning exactly on the target line if timing is tight
            if spawn_y <= TARGET_Y_POS + (ARROW_SIZE * 0.1) {
                trace!(
                    "Skipping spawn for arrow too close to target (y: {:.1})",
                    spawn_y
                );
                continue;
            }

            // --- Determine Arrow Direction ---
            let dir: ArrowDirection;
            // Difficulty 3+: Try not to repeat the same direction immediately
            if DIFFICULTY >= 3 && state.last_spawned_direction.is_some() {
                // Collect all directions *except* the last spawned one
                let mut available_dirs: Vec<ArrowDirection> = ARROW_DIRECTIONS
                    .iter()
                    .copied()
                    .filter(|&d| Some(d) != state.last_spawned_direction)
                    .collect();
                // If filtering left no options (only happens if ARROW_DIRECTIONS has 1 element), use all directions
                if available_dirs.is_empty() {
                    available_dirs = ARROW_DIRECTIONS.to_vec();
                }
                // Choose randomly from the available directions
                dir = *available_dirs.choose(rng).unwrap_or(&ARROW_DIRECTIONS[0]);
            // Default if choose fails (shouldn't)
            } else {
                // Lower difficulties or no previous arrow: choose completely randomly
                dir = ARROW_DIRECTIONS[rng.random_range(0..ARROW_DIRECTIONS.len())];
            }

            // Get the target X position for the chosen direction
            let target_x = state
                .targets
                .iter()
                .find(|t| t.direction == dir)
                .map(|t| t.x)
                .unwrap_or(state.window_size.0 / 2.0); // Fallback to center X

            // Add the new arrow to the corresponding column
            if let Some(column_arrows) = state.arrows.get_mut(&dir) {
                column_arrows.push(Arrow {
                    x: target_x,
                    y: spawn_y,
                    direction: dir,
                    note_type,
                    target_beat,
                });
                trace!(
                    "Spawned {:?} {:?} at y={:.1}, target_beat={:.2}",
                    dir,
                    note_type,
                    spawn_y,
                    target_beat
                );
                // Store the direction if needed for the next spawn decision
                if DIFFICULTY >= 3 {
                    state.last_spawned_direction = Some(dir);
                }
            }
        }
        // Update the index of the last spawned 16th note
        state.last_spawned_16th_index = target_16th_index;
    }

    // --- Arrow Movement ---
    for column_arrows in state.arrows.values_mut() {
        for arrow in column_arrows.iter_mut() {
            arrow.y -= ARROW_SPEED * dt;
        }
    }

    // --- Miss Logic ---
    // Check for arrows that have gone past the miss window
    let miss_window_beats = (MISS_WINDOW_MS / 1000.0) / seconds_per_beat;
    for (_dir, column_arrows) in state.arrows.iter_mut() {
        // Use _dir as it's not needed
        // Use retain to efficiently remove missed arrows
        column_arrows.retain(|arrow| {
            // If the current beat is beyond the arrow's target beat + miss window tolerance
            if state.current_beat > arrow.target_beat + miss_window_beats {
                info!(
                    "MISSED! {:?} {:?} (Target Beat: {:.2}, Current: {:.2}, Time Since Target: {:.1}ms)",
                    arrow.direction,
                    arrow.note_type,
                    arrow.target_beat,
                    state.current_beat,
                    (state.current_beat - arrow.target_beat) * seconds_per_beat * 1000.0
                );
                false // Remove the arrow
            } else {
                true // Keep the arrow
            }
        });
    }

    // --- Flash State Cleanup ---
    // Remove flash states that have expired
    let now_for_cleanup = Instant::now();
    state
        .flash_states
        .retain(|_dir, flash| now_for_cleanup < flash.end_time);
}

// --- check_hits_on_press (Checks for hits when a gameplay key is pressed) ---
fn check_hits_on_press(state: &mut GameState, keycode: VirtualKeyCode) {
    // Map the VirtualKeyCode to an ArrowDirection
    let direction = match keycode {
        VirtualKeyCode::Left => Some(ArrowDirection::Left),
        VirtualKeyCode::Down => Some(ArrowDirection::Down),
        VirtualKeyCode::Up => Some(ArrowDirection::Up),
        VirtualKeyCode::Right => Some(ArrowDirection::Right),
        _ => None, // Ignore other keys like Escape here
    };

    if let Some(dir) = direction {
        // Get the column of arrows for the pressed direction
        if let Some(column_arrows) = state.arrows.get_mut(&dir) {
            let current_beat = state.current_beat; // Use the accurately calculated current beat
            let seconds_per_beat = 60.0 / SONG_BPM;

            let mut best_hit_idx: Option<usize> = None;
            let mut min_time_diff_ms = f32::INFINITY;

            // Iterate through arrows in this column to find the closest one within the hit window
            for (idx, arrow) in column_arrows.iter().enumerate() {
                let beat_diff = current_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat * 1000.0; // Time difference in ms
                let abs_time_diff_ms = time_diff_ms.abs();

                // Check if the arrow is within the widest possible hit window (W4 or Max Hit)
                // and if it's closer than the current best hit found
                if abs_time_diff_ms <= MAX_HIT_WINDOW_MS && abs_time_diff_ms < min_time_diff_ms {
                    min_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            // If a suitable arrow was found
            if let Some(idx) = best_hit_idx {
                // Determine the judgment based on the timing difference
                let judgment = if min_time_diff_ms <= W1_WINDOW_MS {
                    Judgment::W1
                } else if min_time_diff_ms <= W2_WINDOW_MS {
                    Judgment::W2
                } else if min_time_diff_ms <= W3_WINDOW_MS {
                    Judgment::W3
                } else if min_time_diff_ms <= W4_WINDOW_MS {
                    Judgment::W4
                } else {
                    // This case should technically not be reached if MAX_HIT_WINDOW_MS >= W4_WINDOW_MS
                    // But handle it defensively. Could treat as W4 or a separate category.
                    warn!(
                        "Hit registered outside W4 window but within MAX_HIT: {:.1}ms",
                        min_time_diff_ms
                    );
                    Judgment::W4 // Or potentially Judgment::Miss if MAX_HIT is for late hits only
                };

                // --- Process the Hit ---
                // Borrow arrow data *before* removing it
                let hit_arrow = &column_arrows[idx]; // Immutable borrow first
                let time_diff_for_log =
                    (current_beat - hit_arrow.target_beat) * seconds_per_beat * 1000.0; // Recalculate signed diff for log
                let note_type_for_log = hit_arrow.note_type; // Copy data needed after remove

                info!(
                    "HIT! {:?} {:?}. Time Diff: {:.1}ms -> {:?}",
                    dir, note_type_for_log, time_diff_for_log, judgment
                );

                // Trigger visual flash feedback
                let flash_color = match judgment {
                    Judgment::W1 => FLASH_COLOR_W1,
                    Judgment::W2 => FLASH_COLOR_W2,
                    Judgment::W3 => FLASH_COLOR_W3,
                    Judgment::W4 => FLASH_COLOR_W4,
                    Judgment::Miss => unreachable!(), // Miss is handled by time passing, not by key press
                };
                let flash_end_time = Instant::now() + FLASH_DURATION;
                state.flash_states.insert(
                    dir,
                    FlashState {
                        color: flash_color,
                        end_time: flash_end_time,
                    },
                );

                // Remove the hit arrow from the game state
                column_arrows.remove(idx); // Now we can remove it
            } else {
                // Key press occurred, but no arrow was within the hit window for that direction
                debug!(
                    "Input {:?} registered, but no arrow within hit window (Current Beat: {:.2}).",
                    keycode, current_beat
                );
            }
        }
    }
}

// --- NEW: Drawing Function for Menu ---
fn draw_menu(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    menu_state: &MenuState,
    window_size: (f32, f32),
    index_count: u32, // Number of indices for the quad (should be 6)
) {
    // Basic menu layout
    let center_x = window_size.0 / 2.0;
    let start_y = window_size.1 / 2.0 + 50.0; // Position menu items vertically
    let spacing_y = 80.0; // Spacing between items
    let item_width = 250.0; // Width of the placeholder box
    let item_height = 60.0; // Height of the placeholder box

    // Use the whole texture for the placeholder box (no specific UVs needed)
    // But set them anyway to avoid potential issues if shader expects valid values
    let uv_offset = [0.0, 0.0];
    let uv_scale = [1.0, 1.0]; // Use full texture area

    // Draw a placeholder rectangle for each menu option
    for (index, _option_text) in menu_state.options.iter().enumerate() {
        // Text is not drawn
        let y_pos = start_y + index as f32 * spacing_y; // Calculate Y position

        // Set color: Highlight selected item, different color for others
        let color = if index == menu_state.selected_index {
            [0.9, 0.9, 0.2, 0.9] // Highlight color (Yellowish, semi-opaque)
        } else {
            [0.6, 0.6, 0.6, 0.7] // Normal color (Grayish, more transparent)
        };

        // Create model matrix: Translate to position, scale to size
        let model_matrix = Matrix4::from_translation(Vector3::new(center_x, y_pos, 0.0))
            * Matrix4::from_nonuniform_scale(item_width, item_height, 1.0); // Z scale doesn't matter

        // Prepare push constants
        let push_data = PushConstantData {
            model: model_matrix,
            color,
            uv_offset,
            uv_scale,
        };

        // Push constants and draw the quad
        unsafe {
            // Convert push constant data to bytes
            let push_data_bytes = std::slice::from_raw_parts(
                &push_data as *const _ as *const u8,
                mem::size_of::<PushConstantData>(),
            );
            device.cmd_push_constants(
                cmd_buf,
                pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, // Accessible by both
                0,                                                             // Offset
                push_data_bytes,
            );
            // Draw the indexed quad
            device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);
        }
    }
}

// --- NEW: Drawing Function for Gameplay ---
fn draw_gameplay(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    current_beat: f32,
    arrows: &HashMap<ArrowDirection, Vec<Arrow>>,
    targets: &[TargetInfo],
    flash_states: &HashMap<ArrowDirection, FlashState>,
    index_count: u32, // Number of indices for the quad (should be 6)
) {
    unsafe {
        // Unsafe needed for Vulkan calls

        // --- Calculate UVs for Animated Target/Arrow ---
        // Use modulo arithmetic for smooth looping animation based on beat
        let frame_index = ((current_beat * 2.0) % 4.0).floor() as usize; // Adjust multiplier for speed
        let uv_width = 1.0 / 4.0; // Assuming 4 frames horizontally in the atlas
        let uv_x_start = frame_index as f32 * uv_width;
        let uv_offset = [uv_x_start, 0.0]; // Offset in U direction
        let uv_scale = [uv_width, 1.0]; // Scale to select one frame

        // --- Draw Targets ---
        let now_for_flash = Instant::now(); // Get time once for flash checks
        for target in targets {
            // Determine tint: Use flash color if active, otherwise default target tint
            let current_tint = flash_states
                .get(&target.direction)
                .filter(|flash| now_for_flash < flash.end_time) // Check if flash is still active
                .map_or(TARGET_TINT, |flash| flash.color); // Use flash color or default

            // Calculate rotation based on direction
            let rotation_angle = match target.direction {
                ArrowDirection::Down => Rad(0.0), // No rotation needed for down arrow texture
                ArrowDirection::Left => Rad(PI / 2.0), // Rotate 90 degrees clockwise
                ArrowDirection::Up => Rad(PI),    // Rotate 180 degrees
                ArrowDirection::Right => Rad(-PI / 2.0), // Rotate 90 degrees counter-clockwise
            };

            // Create model matrix: Translate, rotate, scale
            let model_matrix = Matrix4::from_translation(Vector3::new(target.x, target.y, 0.0))
                * Matrix4::from_angle_z(rotation_angle)
                * Matrix4::from_nonuniform_scale(TARGET_SIZE, TARGET_SIZE, 1.0);

            // Prepare push constants
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

            // Push constants and draw
            device.cmd_push_constants(
                cmd_buf,
                pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                push_data_bytes,
            );
            device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);
        }

        // --- Draw Arrows ---
        // Iterate through all arrow columns
        for column_arrows in arrows.values() {
            // Iterate through arrows in the current column
            for arrow in column_arrows {
                // Skip drawing arrows that are way off-screen (optimization)
                // Accessing window_size here is tricky. Use constants for now.
                if arrow.y > WINDOW_HEIGHT as f32 + 200.0 || arrow.y < -200.0 {
                    continue;
                }

                // Determine tint based on note type
                let arrow_tint = match arrow.note_type {
                    NoteType::Quarter => ARROW_TINT_QUARTER,
                    NoteType::Eighth => ARROW_TINT_EIGHTH,
                    NoteType::Sixteenth => ARROW_TINT_SIXTEENTH,
                };

                // Calculate rotation based on direction (same as targets)
                let rotation_angle = match arrow.direction {
                    ArrowDirection::Down => Rad(0.0),
                    ArrowDirection::Left => Rad(PI / 2.0),
                    ArrowDirection::Up => Rad(PI),
                    ArrowDirection::Right => Rad(-PI / 2.0),
                };

                // Create model matrix: Translate, rotate, scale
                let model_matrix = Matrix4::from_translation(Vector3::new(arrow.x, arrow.y, 0.0))
                    * Matrix4::from_angle_z(rotation_angle)
                    * Matrix4::from_nonuniform_scale(ARROW_SIZE, ARROW_SIZE, 1.0);

                // Prepare push constants (using the same animated UVs as targets)
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

                // Push constants and draw
                device.cmd_push_constants(
                    cmd_buf,
                    pipeline_layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    push_data_bytes,
                );
                device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);
            }
        }
    }
}

// --- key_to_virtual_keycode (Maps specific NamedKeys to Gameplay Actions) ---
fn key_to_virtual_keycode(key: winit::keyboard::NamedKey) -> Option<VirtualKeyCode> {
    match key {
        NamedKey::ArrowLeft => Some(VirtualKeyCode::Left),
        NamedKey::ArrowDown => Some(VirtualKeyCode::Down),
        NamedKey::ArrowUp => Some(VirtualKeyCode::Up),
        NamedKey::ArrowRight => Some(VirtualKeyCode::Right),
        // NamedKey::Escape => Some(VirtualKeyCode::Escape), // Optional
        _ => None, // Ignore other keys for this mapping
    }
}

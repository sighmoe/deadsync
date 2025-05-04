// main.rs
use ash::vk;
use cgmath::{ortho, Matrix4, Rad, Vector3};
use log::{debug, error, info, trace, warn, LevelFilter};
use rand::distr::{Bernoulli, Distribution};
use rand::prelude::IndexedRandom;
use rand::Rng; // Use Rng trait
use rodio::{Decoder, OutputStream, Sink};
use rodio::OutputStreamHandle;
use rodio::Source;
use rodio::source::Buffered;
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
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::WindowBuilder,
};

use memoffset::offset_of;

mod font;
mod texture;
mod utils;
mod vulkan_base;

use font::{draw_text, load_font, Font};
use texture::{load_texture, TextureResource};
use utils::fps::FPSCounter;
use vulkan_base::{BufferResource, UniformBufferObject, Vertex, VulkanBase};

// --- Constants ---
const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;
const TARGET_Y_POS: f32 = 150.0;
const TARGET_SIZE: f32 = 120.0;
const ARROW_SIZE: f32 = 120.0;
const ARROW_SPEED: f32 = 600.0;
const SONG_BPM: f32 = 174.0;
const SONG_FOLDER_PATH: &str = "songs/Pack/About Tonight";
const SONG_AUDIO_FILENAME: &str = "about_tonight.ogg";
const AUDIO_SYNC_OFFSET_MS: i64 = 60;
const SPAWN_LOOKAHEAD_BEATS: f32 = 10.0;
const DIFFICULTY: u32 = 2;
const W1_WINDOW_MS: f32 = 22.5;
const W2_WINDOW_MS: f32 = 45.0;
const W3_WINDOW_MS: f32 = 90.0;
const W4_WINDOW_MS: f32 = 135.0;
const MAX_HIT_WINDOW_MS: f32 = 180.0;
const MISS_WINDOW_MS: f32 = 200.0;
const FONT_INI_PATH: &str = "assets/fonts/miso/font.ini";
const FONT_TEXTURE_PATH: &str = "assets/fonts/miso/_miso light 15x15 (res 360x360).png";
const LOGO_TEXTURE_PATH: &str = "assets/graphics/logo.png";
const DANCE_TEXTURE_PATH: &str = "assets/graphics/dance.png"; // ADDED
const SFX_CHANGE_PATH: &str = "assets/sounds/change.ogg"; // ADDED
const SFX_START_PATH: &str = "assets/sounds/start.ogg";   // ADDED
const LOGO_DISPLAY_WIDTH: f32 = 500.0;
const LOGO_Y_POS: f32 = WINDOW_HEIGHT as f32 - 700.0;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Menu,
    Gameplay,
}

#[derive(Debug, Clone)]
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
    pressed_keys: HashSet<VirtualKeyCode>,
    last_spawned_16th_index: i32,
    last_spawned_direction: Option<ArrowDirection>,
    current_beat: f32,
    window_size: (f32, f32),
    flash_states: HashMap<ArrowDirection, FlashState>,
    audio_sink: Option<Sink>,
    audio_start_time: Option<Instant>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PushConstantData {
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

// In src/main.rs
fn load_sound_effect(path: &Path) -> Result<Buffered<Decoder<BufReader<File>>>, Box<dyn Error>> {
    let file = File::open(path).map_err(|e| format!("Failed to open SFX {:?}: {}", path, e))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode SFX {:?}: {}", path, e))?;
    let buffered = source.buffered();
    info!("Loaded SFX: {:?}, Buffered", path);
    Ok(buffered)
}

// --- Main Function ---
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info)
        .init();

    // --- Winit Setup ---
    info!("Initializing Winit...");
    let mut event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("DeadSync")
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(WINDOW_WIDTH),
            f64::from(WINDOW_HEIGHT),
        ))
        .build(&event_loop)?;
    let initial_window_size = window.inner_size();

    // --- VulkanBase Setup ---
    info!("Initializing VulkanBase...");
    let mut base = VulkanBase::new(window)?;
    info!("VulkanBase Initialized. GPU: {}", base.get_gpu_name());

    // --- Application State ---
    let mut current_app_state = AppState::Menu;
    let mut menu_state = MenuState {
        options: vec!["Play!".to_string(), "Exit".to_string()],
        selected_index: 0,
    };
    let mut game_state: Option<GameState> = None;
    let mut game_init_pending = false;

    // --- Audio Setup ---
    info!("Preparing audio stream...");
    let (_stream, stream_handle) = OutputStream::try_default()?; // Keep stream_handle for SFX
    let audio_path_str = format!("{}/{}", SONG_FOLDER_PATH, SONG_AUDIO_FILENAME);
    let audio_path = Path::new(&audio_path_str);
    if !audio_path.exists() { return Err(format!("Audio file not found: {:?}", audio_path).into()); }
    info!("Audio stream handle obtained. Music path: {:?}", audio_path);

    // --- Load Sound Effects --- // ADDED
    let change_sfx_path = Path::new(SFX_CHANGE_PATH);
    let start_sfx_path = Path::new(SFX_START_PATH);
    if !change_sfx_path.exists() {
        return Err(format!("SFX not found: {:?}", change_sfx_path).into());
    }
    if !start_sfx_path.exists() {
        return Err(format!("SFX not found: {:?}", start_sfx_path).into());
    }

    let change_sfx = load_sound_effect(change_sfx_path)?;
    let start_sfx = load_sound_effect(start_sfx_path)?;
    info!("Menu sound effects loaded.");
    // --- END ADDED ---

    // --- RNG ---
    let mut rng = rand::rng();

    // --- Common Game Variables ---
    let mut fps_counter = FPSCounter::new();
    let mut last_frame_time = Instant::now();

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
    let quad_index_count = quad_indices.len() as u32;

    let ubo_size = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
    let mut projection_ubo = base.create_buffer(
        ubo_size,
        vk::BufferUsageFlags::UNIFORM_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    // Load Textures
    let arrow_texture_path = Path::new("assets/down_arrow_atlas.png");
    if !arrow_texture_path.exists() {
        return Err("Arrow texture file not found.".into());
    }
    let mut arrow_texture: TextureResource = load_texture(&base, arrow_texture_path)?;
    info!("Arrow texture loaded: {:?}", arrow_texture_path);

    let font_ini_path = Path::new(FONT_INI_PATH);
    let font_texture_path = Path::new(FONT_TEXTURE_PATH);
    if !font_ini_path.exists() {
        return Err("Font INI file not found.".into());
    }
    if !font_texture_path.exists() {
        return Err("Font texture file not found.".into());
    }
    let mut font: Font = load_font(&base, font_ini_path, font_texture_path)?;
    info!("Font loaded: {:?}", font_ini_path);

    let logo_texture_path = Path::new(LOGO_TEXTURE_PATH);
    if !logo_texture_path.exists() {
        return Err(format!("Logo texture file not found: {:?}", logo_texture_path).into());
    }
    let mut logo_texture: TextureResource = load_texture(&base, logo_texture_path)?;
    info!("Logo texture loaded: {:?}", logo_texture_path);

    // --- Load Dance Texture --- // ADDED
    let dance_texture_path = Path::new(DANCE_TEXTURE_PATH);
    if !dance_texture_path.exists() {
        return Err(format!("Dance texture file not found: {:?}", dance_texture_path).into());
    }
    let mut dance_texture: TextureResource = load_texture(&base, dance_texture_path)?;
    info!("Dance texture loaded: {:?}", dance_texture_path);
    // --- END ADDED ---

    // --- Descriptors, Pipeline Layout, Pipeline ---
    // Descriptor Set Layout (defines bindings 0 and 1)
    let dsl_bindings = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX),
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

    // Descriptor Pool (Allocate space for FOUR sets now)
    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 4,
        }, // One UBO per set
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 4,
        }, // One Sampler per set
    ];
    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&pool_sizes)
        .max_sets(4); // Need FOUR sets
    let descriptor_pool = unsafe {
        base.device
            .create_descriptor_pool(&pool_create_info, None)?
    };

    // Allocate FOUR Descriptor Sets
    let set_layouts = [
        descriptor_set_layout,
        descriptor_set_layout,
        descriptor_set_layout,
        descriptor_set_layout,
    ];
    let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let descriptor_sets = unsafe { base.device.allocate_descriptor_sets(&desc_alloc_info)? };
    let descriptor_set_font = descriptor_sets[0];
    let descriptor_set_logo = descriptor_sets[1];
    let descriptor_set_dancer = descriptor_sets[2]; // NEW
    let descriptor_set_gameplay = descriptor_sets[3]; // Was sprite before

    // --- Initial Descriptor Set Updates ---
    let ubo_buffer_info = vk::DescriptorBufferInfo::default()
        .buffer(projection_ubo.buffer)
        .offset(0)
        .range(vk::WHOLE_SIZE);

    // Update Font Set
    let font_image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(font.texture.view)
        .sampler(font.texture.sampler);
    let write_ubo_font = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_font)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .buffer_info(std::slice::from_ref(&ubo_buffer_info));
    let write_sampler_font = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_font)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&font_image_info));

    // Update Logo Set
    let logo_image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(logo_texture.view)
        .sampler(logo_texture.sampler);
    let write_ubo_logo = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_logo)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .buffer_info(std::slice::from_ref(&ubo_buffer_info));
    let write_sampler_logo = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_logo)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&logo_image_info));

    // Update Dancer Set (NEW)
    let dancer_image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(dance_texture.view)
        .sampler(dance_texture.sampler);
    let write_ubo_dancer = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_dancer)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .buffer_info(std::slice::from_ref(&ubo_buffer_info));
    let write_sampler_dancer = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_dancer)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&dancer_image_info));

    // Update Gameplay Set (initially with arrows, though it doesn't matter until Gameplay state)
    let arrow_image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(arrow_texture.view)
        .sampler(arrow_texture.sampler);
    let write_ubo_gameplay = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_gameplay)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .buffer_info(std::slice::from_ref(&ubo_buffer_info));
    let write_sampler_gameplay = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set_gameplay)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&arrow_image_info));

    unsafe {
        base.device.update_descriptor_sets(
            &[
                write_ubo_font,
                write_sampler_font,
                write_ubo_logo,
                write_sampler_logo,
                write_ubo_dancer,
                write_sampler_dancer, // ADDED
                write_ubo_gameplay,
                write_sampler_gameplay,
            ],
            &[],
        );
    }

    // Pipeline Layout (uses the single descriptor_set_layout)
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

    // --- Pipeline (uses the single pipeline layout) ---
    let vert_shader_module = {
        let mut vert_shader_file = Cursor::new(&include_bytes!("../shaders/vert.spv")[..]);
        let vert_code = ash::util::read_spv(&mut vert_shader_file)?;
        let vert_module_info = vk::ShaderModuleCreateInfo::default().code(&vert_code);
        unsafe { base.device.create_shader_module(&vert_module_info, None)? }
    };
    let frag_shader_module = {
        let mut frag_shader_file = Cursor::new(&include_bytes!("../shaders/frag.spv")[..]);
        let frag_code = ash::util::read_spv(&mut frag_shader_file)?;
        let frag_module_info = vk::ShaderModuleCreateInfo::default().code(&frag_code);
        unsafe { base.device.create_shader_module(&frag_module_info, None)? }
    };
    let shader_entry_name = CString::new("main").unwrap();
    let shader_stage_create_infos = [
        vk::PipelineShaderStageCreateInfo::default()
            .module(vert_shader_module)
            .name(&shader_entry_name)
            .stage(vk::ShaderStageFlags::VERTEX),
        vk::PipelineShaderStageCreateInfo::default()
            .module(frag_shader_module)
            .name(&shader_entry_name)
            .stage(vk::ShaderStageFlags::FRAGMENT),
    ];
    let binding_descriptions = [vk::VertexInputBindingDescription {
        binding: 0,
        stride: mem::size_of::<Vertex>() as u32,
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
    let mut current_window_size = (
        initial_window_size.width as f32,
        initial_window_size.height as f32,
    );
    update_projection_matrix(&mut base, &projection_ubo, current_window_size)?;

    // --- Event Loop ---
    info!("Starting Event Loop...");
    let mut resize_needed = false;
    let mut modifiers_state = ModifiersState::default();
    let mut next_app_state: Option<AppState> = None;

    // Clone Arc<SamplesBuffer> for the closure
    let change_sfx_clone = change_sfx.clone();
    let start_sfx_clone = start_sfx.clone();
    // Clone stream handle for closure
    let stream_handle_clone = stream_handle.clone();

    event_loop.run_on_demand(|event, elwp| {
        elwp.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, window_id } if window_id == base.window.id() => {
                 match event {
                    WindowEvent::CloseRequested => { elwp.exit(); }
                    WindowEvent::Resized(new_size) => { if new_size.width > 0 && new_size.height > 0 { current_window_size = (new_size.width as f32, new_size.height as f32); if let Some(ref mut gs) = game_state { gs.window_size = current_window_size; } resize_needed = true; } }
                    WindowEvent::ModifiersChanged(modifiers) => { modifiers_state = modifiers.state(); }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        match current_app_state {
                            AppState::Menu => {
                                // Pass cloned Arcs (&Arc<...>) to input handler
                                if let Some(requested_state) = handle_menu_input(
                                    key_event,
                                    &mut menu_state,
                                    elwp,
                                    &stream_handle_clone,
                                    &change_sfx_clone, // Pass the cloned Arc
                                    &start_sfx_clone,  // Pass the cloned Arc
                                ) {
                                    next_app_state = Some(requested_state);
                                }
                            }
                            AppState::Gameplay => { if let Some(ref mut gs) = game_state { if let Some(requested_state) = handle_gameplay_input(key_event, gs, modifiers_state, elwp) { next_app_state = Some(requested_state); } } }
                        }
                    }
                     _ => {}
                 }
            }
            Event::AboutToWait => {
                // --- State Transition Logic ---
                if let Some(new_state) = next_app_state.take() {
                    if new_state != current_app_state {
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
                                // NO descriptor set update needed here - descriptor_set_gameplay is already configured
                            }
                            (AppState::Gameplay, AppState::Menu) => {
                                info!("Transitioning Gameplay -> Menu");
                                if let Some(ref mut gs) = game_state {
                                    if let Some(sink) = gs.audio_sink.take() {
                                        info!("Stopping gameplay audio.");
                                        sink.stop();
                                    }
                                }
                                game_state = None;
                                base.window.set_title("DeadSync");
                                menu_state.selected_index = 0;
                                // NO descriptor set update needed here - logo/dancer sets are already configured
                            }
                            _ => {
                                warn!(
                                    "Unexpected state transition requested from {:?} to {:?}",
                                    current_app_state, new_state
                                );
                            }
                        }
                        current_app_state = new_state;
                    }
                }

                // --- Initialize Game State if Pending ---
                if game_init_pending && current_app_state == AppState::Gameplay {
                    info!("Initializing Game State...");
                    let file = match File::open(&audio_path) { Ok(f) => f, Err(e) => { error!("Failed to open audio: {}",e); game_init_pending=false; current_app_state=AppState::Menu; base.window.request_redraw(); return; } };
                    let source = match Decoder::new(BufReader::new(file)) { Ok(s) => s, Err(e) => { error!("Failed to decode audio: {}",e); game_init_pending=false; current_app_state=AppState::Menu; base.window.request_redraw(); return; } };
                    let sink = match Sink::try_new(&stream_handle) { Ok(s) => s, Err(e) => { error!("Failed to create sink: {}",e); game_init_pending=false; current_app_state=AppState::Menu; base.window.request_redraw(); return; } };
                    sink.append(source); sink.play(); let audio_start_time = Instant::now();
                    game_state = Some(initialize_game_state( current_window_size.0, current_window_size.1, Some(sink), Some(audio_start_time) ));
                    info!("Gameplay initialized and audio started at {:?}", audio_start_time); game_init_pending = false;
                }

                // --- Handle Resize ---
                if resize_needed {
                    log::warn!("Resize detected - Vulkan swapchain recreation NOT IMPLEMENTED! Graphics may be distorted.");
                    if let Err(e) = update_projection_matrix(&mut base, &projection_ubo, current_window_size) { error!("Failed to update projection UBO after resize: {}", e); }
                    else { info!("Projection matrix updated for new size: {:?}", current_window_size); }
                    resize_needed = false;
                 }

                // --- Update Logic ---
                let now = Instant::now(); let dt = (now - last_frame_time).as_secs_f32().max(0.0).min(0.1); last_frame_time = now;
                match current_app_state {
                    AppState::Menu => { if let Some(fps) = fps_counter.update() { base.window.set_title(&format!("DeadSync | FPS: {}", fps)); } }
                    AppState::Gameplay => { if let Some(ref mut gs) = game_state { update_game_state(gs, dt, &mut rng); if let Some(fps) = fps_counter.update() { base.window.set_title(&format!("DeadSync | BPM: {} | FPS: {}", SONG_BPM, fps)); } } else { error!("In Gameplay state but game_state is None!"); } }
                }

                // --- Drawing ---
                let current_surface_extent = base.surface_resolution;
                let menu_state_clone = if current_app_state == AppState::Menu { Some(menu_state.clone()) } else { None };
                let game_state_data_for_draw = if current_app_state == AppState::Gameplay { game_state.as_ref().map(|gs| (gs.current_beat, gs.arrows.clone(), gs.targets.clone(), gs.flash_states.clone())) } else { None };
                let app_state_for_draw = current_app_state;

                // References needed inside the closure
                let font_ref = &font; // Still need font ref for draw_menu_options
                // No longer need direct texture refs here, using descriptor set handles

                match base.draw_frame(|device, cmd_buf| {
                    // Common Setup (Bind pipeline, vertex/index buffers)
                    unsafe {
                        device.cmd_bind_pipeline(cmd_buf, vk::PipelineBindPoint::GRAPHICS, graphics_pipeline);
                        device.cmd_bind_vertex_buffers(cmd_buf, 0, &[vertex_buffer.buffer], &[0]);
                        device.cmd_bind_index_buffer(cmd_buf, index_buffer.buffer, 0, vk::IndexType::UINT32);
                        // DO NOT bind descriptor sets here globally
                        let viewport = vk::Viewport { x: 0.0, y: 0.0, width: current_surface_extent.width as f32, height: current_surface_extent.height as f32, min_depth: 0.0, max_depth: 1.0 };
                        let scissor = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: current_surface_extent };
                        device.cmd_set_viewport(cmd_buf, 0, &[viewport]);
                        device.cmd_set_scissor(cmd_buf, 0, &[scissor]);
                    }

                    // State-Specific Drawing
                    match app_state_for_draw {
                        AppState::Menu => {
                            if let Some(ms) = menu_state_clone {
                                unsafe {
                                    // --- Draw Logo ---
                                    device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[descriptor_set_logo], &[]);
                                    let aspect_ratio_logo = logo_texture.width as f32 / logo_texture.height.max(1) as f32;
                                    let logo_height = LOGO_DISPLAY_WIDTH / aspect_ratio_logo;
                                    let logo_x = (current_window_size.0 - LOGO_DISPLAY_WIDTH) / 2.0;
                                    let logo_y = LOGO_Y_POS;
                                    let model_matrix_logo = Matrix4::from_translation(Vector3::new(logo_x + LOGO_DISPLAY_WIDTH / 2.0, logo_y + logo_height / 2.0, 0.0)) * Matrix4::from_nonuniform_scale(LOGO_DISPLAY_WIDTH, logo_height, 1.0);
                                    let push_data_logo = PushConstantData { model: model_matrix_logo, color: [1.0, 1.0, 1.0, 1.0], uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0] };
                                    let push_data_bytes_logo = std::slice::from_raw_parts(&push_data_logo as *const _ as *const u8, mem::size_of::<PushConstantData>());
                                    device.cmd_push_constants(cmd_buf, pipeline_layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, push_data_bytes_logo);
                                    device.cmd_draw_indexed(cmd_buf, quad_index_count, 1, 0, 0, 0); // Draw Logo

                                    // --- Draw Dancer ---
                                    device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[descriptor_set_dancer], &[]); // Bind DANCER set
                                    let aspect_ratio_dancer = dance_texture.width as f32 / dance_texture.height.max(1) as f32;
                                    let dancer_height = LOGO_DISPLAY_WIDTH / aspect_ratio_dancer; // Scale dancer width to logo width
                                    let dancer_x = logo_x; // Same horizontal position as logo
                                    // Calculate Y position to center dancer in the logo's vertical center
                                    let dancer_y = logo_y + (logo_height / 2.0) - (dancer_height / 2.0);
                                    let model_matrix_dancer = Matrix4::from_translation(Vector3::new(dancer_x + LOGO_DISPLAY_WIDTH / 2.0, dancer_y + dancer_height / 2.0, 0.0)) * Matrix4::from_nonuniform_scale(LOGO_DISPLAY_WIDTH, dancer_height, 1.0);
                                    let push_data_dancer = PushConstantData { model: model_matrix_dancer, color: [1.0, 1.0, 1.0, 1.0], uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0] };
                                    let push_data_bytes_dancer = std::slice::from_raw_parts(&push_data_dancer as *const _ as *const u8, mem::size_of::<PushConstantData>());
                                    device.cmd_push_constants(cmd_buf, pipeline_layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, push_data_bytes_dancer);
                                    device.cmd_draw_indexed(cmd_buf, quad_index_count, 1, 0, 0, 0); // Draw Dancer


                                    // --- Draw Menu Options ---
                                    device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[descriptor_set_font], &[]); // Bind FONT set
                                    draw_menu_options(device, cmd_buf, pipeline_layout, &ms, font_ref, current_window_size, quad_index_count);
                                }
                            }
                        }
                        AppState::Gameplay => {
                             if let Some((current_beat, arrows, targets, flash_states)) = game_state_data_for_draw {
                                unsafe {
                                    // Bind the SPRITE descriptor set
                                    device.cmd_bind_descriptor_sets(cmd_buf, vk::PipelineBindPoint::GRAPHICS, pipeline_layout, 0, &[descriptor_set_gameplay], &[]);
                                }
                                // Call gameplay draw function
                                draw_gameplay(device, cmd_buf, pipeline_layout, current_beat, &arrows, &targets, &flash_states, quad_index_count);
                             }
                        }
                    }
                }) {
                    Ok(needs_resize) => { if needs_resize { resize_needed = true; } }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => { resize_needed = true; }
                    Err(e) => { error!("Failed to draw frame: {:?}", e); elwp.exit(); }
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
    info!("Cleaning up application resources...");
    if let Some(mut gs) = game_state.take() {
        if let Some(sink) = gs.audio_sink.take() {
            info!("Stopping game audio sink.");
            sink.stop();
        }
    }
    unsafe {
        info!("Destroying graphics pipeline...");
        base.device.destroy_pipeline(graphics_pipeline, None);
        info!("Destroying pipeline layout...");
        base.device.destroy_pipeline_layout(pipeline_layout, None);
        info!("Destroying descriptor pool...");
        base.device.destroy_descriptor_pool(descriptor_pool, None); // Destroys sets
        info!("Destroying descriptor set layout...");
        base.device
            .destroy_descriptor_set_layout(descriptor_set_layout, None);
        info!("Destroying vertex buffer...");
        vertex_buffer.destroy(&base.device);
        info!("Destroying index buffer...");
        index_buffer.destroy(&base.device);
        info!("Destroying projection UBO...");
        projection_ubo.destroy(&base.device);
        info!("Destroying arrow texture...");
        arrow_texture.destroy(&base.device);
        info!("Destroying font texture...");
        font.destroy(&base.device);
        info!("Destroying logo texture...");
        logo_texture.destroy(&base.device);
        info!("Destroying dance texture...");
        dance_texture.destroy(&base.device);
    }
    info!("Main Vulkan resources cleaned up.");
    info!("Exiting application.");
    Ok(())
}

// --- Helper to update projection matrix ---
fn update_projection_matrix(
    base: &mut VulkanBase,
    ubo_buffer: &BufferResource,
    window_size: (f32, f32),
) -> Result<(), Box<dyn Error>> {
    let proj = ortho(0.0, window_size.0, 0.0, window_size.1, -1.0, 1.0);
    let ubo = UniformBufferObject { projection: proj };
    base.update_buffer(ubo_buffer, &[ubo])?;
    Ok(())
}

fn handle_menu_input(
    key_event: KeyEvent,
    menu_state: &mut MenuState,
    elwp: &EventLoopWindowTarget<()>,
    stream_handle: &OutputStreamHandle,
    change_sfx: &Buffered<Decoder<BufReader<File>>>,
    start_sfx: &Buffered<Decoder<BufReader<File>>>,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        match key_event.logical_key {
            Key::Named(NamedKey::ArrowUp) => {
                let old_index = menu_state.selected_index;
                menu_state.selected_index = if menu_state.selected_index == 0 {
                    menu_state.options.len() - 1
                } else {
                    menu_state.selected_index - 1
                };
                if menu_state.selected_index != old_index {
                    if let Ok(sink) = Sink::try_new(stream_handle) {
                        sink.append(change_sfx.clone());
                        sink.detach();
                    } else {
                        warn!("Failed to create temporary sink for change SFX");
                    }
                }
                debug!("Menu Up: Selected index {}", menu_state.selected_index);
            }
            Key::Named(NamedKey::ArrowDown) => {
                let old_index = menu_state.selected_index;
                menu_state.selected_index = (menu_state.selected_index + 1) % menu_state.options.len();
                if menu_state.selected_index != old_index {
                    if let Ok(sink) = Sink::try_new(stream_handle) {
                        sink.append(change_sfx.clone());
                        sink.detach();
                    } else {
                        warn!("Failed to create temporary sink for change SFX");
                    }
                }
                debug!("Menu Down: Selected index {}", menu_state.selected_index);
            }
            Key::Named(NamedKey::Enter) => {
                debug!("Menu Enter: Selected index {}", menu_state.selected_index);
                if let Ok(sink) = Sink::try_new(stream_handle) {
                    sink.append(start_sfx.clone());
                    sink.detach();
                } else {
                    warn!("Failed to create temporary sink for start SFX");
                }
                std::thread::sleep(Duration::from_millis(50));
                match menu_state.selected_index {
                    0 => return Some(AppState::Gameplay),
                    1 => elwp.exit(),
                    _ => {}
                }
            }
            Key::Named(NamedKey::Escape) => {
                debug!("Menu Escape: Exiting");
                elwp.exit();
            }
            _ => {}
        }
    }
    None
}
fn handle_gameplay_input(
    key_event: KeyEvent,
    state: &mut GameState,
    _modifiers: ModifiersState,
    _elwp: &EventLoopWindowTarget<()>,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if key_event.logical_key == Key::Named(NamedKey::Escape)
            || key_event.physical_key == PhysicalKey::Code(KeyCode::Escape)
        {
            info!("Escape pressed in gameplay, returning to menu.");
            return Some(AppState::Menu);
        }
    }
    if let Key::Named(named_key) = key_event.logical_key {
        if let Some(virtual_keycode) = key_to_virtual_keycode(named_key) {
            match key_event.state {
                ElementState::Pressed => {
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
    }
    None
}

// --- Game Initialization ---
fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_sink: Option<Sink>,
    audio_start_time: Option<Instant>,
) -> GameState {
    info!(
        "Initializing game state for window size: {}x{}",
        win_w, win_h
    );
    let center_x = win_w / 2.0;
    let target_spacing = TARGET_SIZE * 1.2;
    let total_width = (ARROW_DIRECTIONS.len() as f32 * target_spacing) - (target_spacing * 0.2);
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
    let initial_beat = -(AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) * (SONG_BPM / 60.0);
    info!(
        "Audio Sync Offset: {} ms -> Estimated Initial Beat: {:.4}",
        AUDIO_SYNC_OFFSET_MS, initial_beat
    );
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
        current_beat: initial_beat,
        window_size: (win_w, win_h),
        flash_states: HashMap::new(),
        audio_sink,
        audio_start_time,
    }
}

// --- Game State Update ---
fn update_game_state(state: &mut GameState, dt: f32, rng: &mut impl Rng) {
    if let Some(start_time) = state.audio_start_time {
        let elapsed_since_audio_start = Instant::now().duration_since(start_time).as_secs_f32();
        let beat_offset = (AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) * (SONG_BPM / 60.0);
        state.current_beat = elapsed_since_audio_start * (SONG_BPM / 60.0) - beat_offset;
    } else {
        let beat_delta = dt * (SONG_BPM / 60.0);
        state.current_beat += beat_delta;
        warn!("Updating beat based on dt, audio start time not available or audio not playing.");
    }
    let seconds_per_beat = 60.0 / SONG_BPM;
    let target_16th_index = ((state.current_beat + SPAWN_LOOKAHEAD_BEATS) * 4.0).floor() as i32;
    if target_16th_index > state.last_spawned_16th_index {
        let bernoulli_half = Bernoulli::new(0.5).unwrap();
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
                        || (note_type == NoteType::Eighth && bernoulli_half.sample(rng))
                }
                2 => note_type == NoteType::Quarter || note_type == NoteType::Eighth,
                3 | _ => true,
            };
            if !should_spawn {
                continue;
            }
            let beats_remaining = target_beat - state.current_beat;
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
            if spawn_y <= TARGET_Y_POS + (ARROW_SIZE * 0.1) {
                trace!(
                    "Skipping spawn for arrow too close to target (y: {:.1})",
                    spawn_y
                );
                continue;
            }
            let dir: ArrowDirection = if DIFFICULTY >= 3 && state.last_spawned_direction.is_some() {
                let mut available_dirs: Vec<ArrowDirection> = ARROW_DIRECTIONS
                    .iter()
                    .copied()
                    .filter(|&d| Some(d) != state.last_spawned_direction)
                    .collect();
                if available_dirs.is_empty() {
                    available_dirs = ARROW_DIRECTIONS.to_vec();
                }
                *available_dirs.choose(rng).unwrap_or(&ARROW_DIRECTIONS[0])
            } else {
                ARROW_DIRECTIONS[rng.random_range(0..ARROW_DIRECTIONS.len())]
            };
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
                trace!(
                    "Spawned {:?} {:?} at y={:.1}, target_beat={:.2}",
                    dir,
                    note_type,
                    spawn_y,
                    target_beat
                );
                if DIFFICULTY >= 3 {
                    state.last_spawned_direction = Some(dir);
                }
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
                    "MISSED! {:?} {:?} (Tgt: {:.2}, Now: {:.2}, Diff: {:.1}ms)",
                    arrow.direction,
                    arrow.note_type,
                    arrow.target_beat,
                    state.current_beat,
                    (state.current_beat - arrow.target_beat) * seconds_per_beat * 1000.0
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

// --- Hit Checking ---
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
                let note_type_for_log = hit_arrow.note_type;
                info!(
                    "HIT! {:?} {:?}. Time Diff: {:.1}ms -> {:?}",
                    dir, note_type_for_log, time_diff_for_log, judgment
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
                    "Input {:?} registered, but no arrow within hit window (Beat: {:.2}).",
                    keycode, current_beat
                );
            }
        }
    }
}

// --- Drawing Function for Menu Options ONLY ---
fn draw_menu_options(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    menu_state: &MenuState,
    font: &Font,
    window_size: (f32, f32),
    index_count: u32,
) {
    // Assumes FONT descriptor set is already bound
    let center_x = window_size.0 / 2.0;
    let start_y = window_size.1 / 2.0 + 210.0;
    let spacing_y = font.line_height * 4.5;
    for (index, option_text) in menu_state.options.iter().enumerate() {
        let y_pos = start_y + index as f32 * spacing_y;
        let color = if index == menu_state.selected_index {
            [1.0, 1.0, 0.5, 1.0]
        } else {
            [0.8, 0.8, 0.8, 1.0]
        };
        let text_width = font.measure_text(option_text);
        let x_pos = center_x - text_width / 2.0;
        draw_text(
            device,
            cmd_buf,
            pipeline_layout,
            font,
            option_text,
            x_pos,
            y_pos,
            color,
            index_count,
        );
    }
}

// --- Gameplay Drawing ---
fn draw_gameplay(
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    current_beat: f32,
    arrows: &HashMap<ArrowDirection, Vec<Arrow>>,
    targets: &[TargetInfo],
    flash_states: &HashMap<ArrowDirection, FlashState>,
    index_count: u32,
) {
    // Assumes SPRITE descriptor set (pointing to arrow_texture) is already bound
    unsafe {
        let frame_index = ((current_beat * 2.0) % 4.0).floor() as usize;
        let uv_width = 1.0 / 4.0;
        let uv_x_start = frame_index as f32 * uv_width;
        let uv_offset = [uv_x_start, 0.0];
        let uv_scale = [uv_width, 1.0];
        let now_for_flash = Instant::now();
        for target in targets {
            let current_tint = flash_states
                .get(&target.direction)
                .filter(|flash| now_for_flash < flash.end_time)
                .map_or(TARGET_TINT, |flash| flash.color);
            let rotation_angle = match target.direction {
                ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Left => Rad(PI / 2.0),
                ArrowDirection::Up => Rad(PI),
                ArrowDirection::Right => Rad(-PI / 2.0),
            };
            let model_matrix = Matrix4::from_translation(Vector3::new(target.x, target.y, 0.0))
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
            device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);
        }
        for column_arrows in arrows.values() {
            for arrow in column_arrows {
                if arrow.y > WINDOW_HEIGHT as f32 + 200.0 || arrow.y < -200.0 {
                    continue;
                }
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
                let model_matrix = Matrix4::from_translation(Vector3::new(arrow.x, arrow.y, 0.0))
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
                device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);
            }
        }
    }
}

// --- Key Mapping ---
fn key_to_virtual_keycode(key: winit::keyboard::NamedKey) -> Option<VirtualKeyCode> {
    match key {
        NamedKey::ArrowLeft => Some(VirtualKeyCode::Left),
        NamedKey::ArrowDown => Some(VirtualKeyCode::Down),
        NamedKey::ArrowUp => Some(VirtualKeyCode::Up),
        NamedKey::ArrowRight => Some(VirtualKeyCode::Right),
        _ => None,
    }
}

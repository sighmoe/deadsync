use crate::{
    renderer,
    screen::{ObjectType, Screen},
};
use ash::{
    khr::{surface, swapchain},
    vk, Device, Entry, Instance,
};
use cgmath::Matrix4;
use image::RgbaImage;
use log::{debug, error, info, warn};
use std::{collections::HashMap, error::Error, ffi, mem, sync::Arc};
use winit::{
    dpi::PhysicalSize,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

// --- Constants ---
const MAX_FRAMES_IN_FLIGHT: usize = 3;

// --- Structs ---

// Push constants for drawing solid-colored objects.
#[repr(C)]
struct SolidPushConstants {
    mvp: Matrix4<f32>,
    color: [f32; 4],
}

// Push constants for drawing textured objects. Color comes from the texture, so we only need the matrix.
#[repr(C)]
struct TexturedPushConstants {
    mvp: Matrix4<f32>,
}

// A handle to a Vulkan texture on the GPU.
// It bundles the image, its memory, its view, and the descriptor set that links it to the shader.
pub struct Texture {
    device: Arc<Device>, // For automatic cleanup on Drop
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    pub descriptor_set: vk::DescriptorSet,
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image_view(self.view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

struct ObjectDrawInfo {
    index_count: u32,
    first_index: u32,
}

struct BufferResource {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

struct SwapchainResources {
    swapchain_loader: swapchain::Device,
    swapchain: vk::SwapchainKHR,
    _images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    extent: vk::Extent2D,
    format: vk::SurfaceFormatKHR,
}

// The main Vulkan state struct, now with resources for two pipelines and texturing.
pub struct State {
    _entry: Entry,
    instance: Instance,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    debug_loader: Option<ash::ext::debug_utils::Instance>,
    surface: vk::SurfaceKHR,
    surface_loader: surface::Instance,
    pub pdevice: vk::PhysicalDevice,
    pub device: Arc<Device>, // FIX: Use Arc for sharing with Texture
    pub queue: vk::Queue,
    pub command_pool: vk::CommandPool,
    swapchain_resources: SwapchainResources,
    render_pass: vk::RenderPass,
    solid_pipeline_layout: vk::PipelineLayout,
    solid_pipeline: vk::Pipeline,
    texture_pipeline_layout: vk::PipelineLayout,
    texture_pipeline: vk::Pipeline,
    vertex_buffer: Option<BufferResource>,
    index_buffer: Option<BufferResource>,
    object_draw_info: Vec<ObjectDrawInfo>,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub descriptor_pool: vk::DescriptorPool,
    pub sampler: vk::Sampler,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    images_in_flight: Vec<vk::Fence>,
    current_frame: usize,
    window_size: PhysicalSize<u32>,
}

// --- Main Procedural Functions ---
pub fn init(window: &Window, screen: &Screen) -> Result<State, Box<dyn Error>> {
    info!("Initializing Vulkan backend...");
    let entry = Entry::linked();
    let instance = create_instance(&entry, window)?;
    let (debug_loader, debug_messenger) = setup_debug_messenger(&entry, &instance)?;
    let surface = create_surface(&entry, &instance, window)?;
    let surface_loader = surface::Instance::new(&entry, &instance);
    let pdevice = select_physical_device(&instance, &surface_loader, surface)?;
    let (device, queue, queue_family_index) =
        create_logical_device(&instance, pdevice, &surface_loader, surface)?;
    let device = Arc::new(device); // FIX: Wrap in Arc for sharing
    let command_pool = create_command_pool(&device, queue_family_index)?;

    let initial_size = window.inner_size();
    let mut swapchain_resources = create_swapchain(
        &instance,
        &device,
        pdevice,
        surface,
        &surface_loader,
        initial_size,
        None,
    )?;
    let render_pass = create_render_pass(&device, swapchain_resources.format.format)?;
    recreate_framebuffers(&device, &mut swapchain_resources, render_pass)?;

    // --- NEW: Create resources required for texturing ---
    let sampler = create_sampler(&device)?;
    let descriptor_set_layout = create_descriptor_set_layout(&device)?;
    let descriptor_pool = create_descriptor_pool(&device)?;

    // --- NEW: Create the two distinct graphics pipelines ---
    let (solid_pipeline_layout, solid_pipeline) =
        create_solid_pipeline(&device, render_pass)?;
    let (texture_pipeline_layout, texture_pipeline) =
        create_texture_pipeline(&device, render_pass, descriptor_set_layout)?;

    let command_buffers = create_command_buffers(&device, command_pool, MAX_FRAMES_IN_FLIGHT)?;
    let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
        create_sync_objects(&device)?;
    let images_in_flight = vec![vk::Fence::null(); swapchain_resources._images.len()];

    let mut state = State {
        _entry: entry,
        instance,
        debug_messenger,
        debug_loader,
        surface,
        surface_loader,
        pdevice,
        device,
        queue,
        command_pool,
        swapchain_resources,
        render_pass,
        solid_pipeline_layout,
        solid_pipeline,
        texture_pipeline_layout,
        texture_pipeline,
        vertex_buffer: None,
        index_buffer: None,
        object_draw_info: Vec::new(),
        descriptor_set_layout,
        descriptor_pool,
        sampler,
        command_buffers,
        image_available_semaphores,
        render_finished_semaphores,
        in_flight_fences,
        images_in_flight,
        current_frame: 0,
        window_size: initial_size,
    };

    load_screen(&mut state, screen)?;

    info!("Vulkan backend initialized successfully.");
    Ok(state)
}

// --- NEW HELPER FUNCTIONS FOR TEXTURING (CALLED BY INIT) ---

// Creates a sampler to tell shaders how to read textures (e.g., with linear filtering).
fn create_sampler(device: &Device) -> Result<vk::Sampler, vk::Result> {
    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT)
        .address_mode_v(vk::SamplerAddressMode::REPEAT)
        .address_mode_w(vk::SamplerAddressMode::REPEAT)
        .anisotropy_enable(false)
        .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
        .unnormalized_coordinates(false)
        .compare_enable(false)
        .compare_op(vk::CompareOp::ALWAYS)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR);
    unsafe { device.create_sampler(&sampler_info, None) }
}

// Creates a layout that describes to the GPU what resources a shader will bind (e.g., "binding 0 is a texture").
fn create_descriptor_set_layout(device: &Device) -> Result<vk::DescriptorSetLayout, vk::Result> {
    let sampler_layout_binding = vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT); // The fragment shader uses the texture

    let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(std::slice::from_ref(&sampler_layout_binding));

    unsafe { device.create_descriptor_set_layout(&layout_info, None) }
}

// Creates a pool from which we can allocate descriptor sets.
fn create_descriptor_pool(device: &Device) -> Result<vk::DescriptorPool, vk::Result> {
    // We'll allow for up to 100 textures to be allocated.
    let pool_size = vk::DescriptorPoolSize::default()
        .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(100);

    let pool_info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(std::slice::from_ref(&pool_size))
        .max_sets(100);

    unsafe { device.create_descriptor_pool(&pool_info, None) }
}

// --- NEW PIPELINE CREATION FUNCTIONS ---

// Creates the pipeline for drawing solid-colored objects.
fn create_solid_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
) -> Result<(vk::PipelineLayout, vk::Pipeline), Box<dyn Error>> {
    let vert_shader_code = include_bytes!(concat!(env!("OUT_DIR"), "/vulkan_solid.vert.spv"));
    let frag_shader_code = include_bytes!(concat!(env!("OUT_DIR"), "/vulkan_solid.frag.spv"));
    let vert_module = create_shader_module(device, vert_shader_code)?;
    let frag_module = create_shader_module(device, frag_shader_code)?;
    let main_name = ffi::CStr::from_bytes_with_nul(b"main\0")?;

    let shader_stages = [
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vert_module).name(&main_name),
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(frag_module).name(&main_name),
    ];

    let binding_descriptions = [vk::VertexInputBindingDescription::default().binding(0).stride(mem::size_of::<[f32; 2]>() as u32).input_rate(vk::VertexInputRate::VERTEX)];
    let attribute_descriptions = [vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32_SFLOAT).offset(0)];
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding_descriptions).vertex_attribute_descriptions(&attribute_descriptions);
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.0).cull_mode(vk::CullModeFlags::BACK).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
    let multisampling = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA);
    let color_blending = vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&color_blend_attachment));
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let push_constant_range = vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(mem::size_of::<SolidPushConstants>() as u32);
        
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default().push_constant_ranges(std::slice::from_ref(&push_constant_range));
    let pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_layout_info, None)? };

    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stages).vertex_input_state(&vertex_input_info).input_assembly_state(&input_assembly).viewport_state(&viewport_state)
        .rasterization_state(&rasterizer).multisample_state(&multisampling).color_blend_state(&color_blending).dynamic_state(&dynamic_state)
        .layout(pipeline_layout).render_pass(render_pass).subpass(0);

    let pipeline = unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).map_err(|e| e.1)?[0] };

    unsafe {
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
    }

    Ok((pipeline_layout, pipeline))
}

// Creates the pipeline for drawing textured objects.
fn create_texture_pipeline(
    device: &Device,
    render_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
) -> Result<(vk::PipelineLayout, vk::Pipeline), Box<dyn Error>> {
    let vert_shader_code = include_bytes!(concat!(env!("OUT_DIR"), "/vulkan_texture.vert.spv"));
    let frag_shader_code = include_bytes!(concat!(env!("OUT_DIR"), "/vulkan_texture.frag.spv"));
    let vert_module = create_shader_module(device, vert_shader_code)?;
    let frag_module = create_shader_module(device, frag_shader_code)?;
    let main_name = ffi::CStr::from_bytes_with_nul(b"main\0")?;

    let shader_stages = [
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vert_module).name(&main_name),
        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(frag_module).name(&main_name),
    ];
    
    // Vertex format is the same for both pipelines
    let binding_descriptions = [vk::VertexInputBindingDescription::default().binding(0).stride(mem::size_of::<[f32; 2]>() as u32).input_rate(vk::VertexInputRate::VERTEX)];
    let attribute_descriptions = [vk::VertexInputAttributeDescription::default().binding(0).location(0).format(vk::Format::R32G32_SFLOAT).offset(0)];
    let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding_descriptions).vertex_attribute_descriptions(&attribute_descriptions);
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport_state = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.0).cull_mode(vk::CullModeFlags::BACK).front_face(vk::FrontFace::COUNTER_CLOCKWISE);
    let multisampling = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
    
    // Enable alpha blending for textures
    let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
        .alpha_blend_op(vk::BlendOp::ADD);

    let color_blending = vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&color_blend_attachment));
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic_state = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let push_constant_range = vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(mem::size_of::<TexturedPushConstants>() as u32);
        
    // The key difference: This layout USES the descriptor set for textures.
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(std::slice::from_ref(&set_layout))
        .push_constant_ranges(std::slice::from_ref(&push_constant_range));

    let pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_layout_info, None)? };

    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&shader_stages).vertex_input_state(&vertex_input_info).input_assembly_state(&input_assembly).viewport_state(&viewport_state)
        .rasterization_state(&rasterizer).multisample_state(&multisampling).color_blend_state(&color_blending).dynamic_state(&dynamic_state)
        .layout(pipeline_layout).render_pass(render_pass).subpass(0);

    let pipeline = unsafe { device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).map_err(|e| e.1)?[0] };

    unsafe {
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
    }

    Ok((pipeline_layout, pipeline))
}


// --- STUBS FOR NEXT ITERATION ---

// The following functions are needed to make the code compile but their logic
// will be provided in the next part.

pub fn create_texture(state: &mut State, image: &RgbaImage) -> Result<Texture, Box<dyn Error>> {
    let (width, height) = image.dimensions();
    let image_size = (width * height * 4) as vk::DeviceSize;
    let image_data = image.as_raw();

    // 1. Create a staging buffer on the CPU that we can write to.
    let staging_buffer = create_buffer_with_data_raw(
        &state.instance, &state.device, state.pdevice, image_size, vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    
    // 2. Copy the pixel data into the staging buffer.
    unsafe {
        let mapped = state.device.map_memory(staging_buffer.memory, 0, image_size, vk::MemoryMapFlags::empty())?;
        std::ptr::copy_nonoverlapping(image_data.as_ptr(), mapped as *mut u8, image_data.len());
        state.device.unmap_memory(staging_buffer.memory);
    }

    // 3. Create the final image object on the GPU. It cannot be written to directly by the CPU.
    let (texture_image, texture_memory) = create_image(
        state, width, height, vk::Format::R8G8B8A8_SRGB, vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED, vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;

    // 4. Transition the image layout to be ready for the copy, then copy from the buffer.
    transition_image_layout(state, texture_image, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL)?;
    copy_buffer_to_image(state, staging_buffer.buffer, texture_image, width, height)?;
    
    // 5. Transition the image layout to be ready for being read by the shader.
    transition_image_layout(state, texture_image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)?;

    // 6. Clean up the temporary staging buffer.
    destroy_buffer(&state.device, &staging_buffer);

    // 7. Create a view and descriptor set for the final image.
    let view = create_image_view(&state.device, texture_image, vk::Format::R8G8B8A8_SRGB)?;
    let descriptor_set = create_texture_descriptor_set(state, view, state.sampler)?;

        Ok(Texture {
        device: state.device.clone(), // FIX: Clone the Arc to share ownership
        image: texture_image,
        memory: texture_memory,
        view,
        descriptor_set,
    })
}

// --- NEW: `load_screen` full implementation ---
pub fn load_screen(state: &mut State, screen: &Screen) -> Result<(), Box<dyn Error>> {
    info!("Loading new screen for Vulkan...");
    unsafe { state.device.device_wait_idle()? };

    if let Some(buffer) = state.vertex_buffer.take() {
        destroy_buffer(&state.device, &buffer);
    }
    if let Some(buffer) = state.index_buffer.take() {
        destroy_buffer(&state.device, &buffer);
    }
    state.object_draw_info.clear();

    if screen.objects.is_empty() {
        info!("New screen has no objects to load.");
        return Ok(());
    }

    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    for object in &screen.objects {
        let first_index = all_indices.len() as u32;
        let vertex_offset = all_vertices.len() as u16;
        all_vertices.extend_from_slice(&object.vertices);
        all_indices.extend(object.indices.iter().map(|&i| i + vertex_offset));
        state.object_draw_info.push(ObjectDrawInfo {
            index_count: object.indices.len() as u32,
            first_index,
        });
    }

    state.vertex_buffer = Some(create_buffer_with_data_vec(&state.instance, &state.device, state.pdevice, state.command_pool, state.queue, &all_vertices, vk::BufferUsageFlags::VERTEX_BUFFER)?);
    state.index_buffer = Some(create_buffer_with_data_vec(&state.instance, &state.device, state.pdevice, state.command_pool, state.queue, &all_indices, vk::BufferUsageFlags::INDEX_BUFFER)?);
    
    info!("Vulkan screen loaded successfully.");
    Ok(())
}


pub fn draw(
    state: &mut State,
    screen: &Screen,
    textures: &HashMap<String, renderer::Texture>,
) -> Result<(), Box<dyn Error>> {
    if state.window_size.width == 0 || state.window_size.height == 0 {
        return Ok(());
    }
    unsafe {
        let fence = state.in_flight_fences[state.current_frame];
        state.device.wait_for_fences(&[fence], true, u64::MAX)?;

        let result = state.swapchain_resources.swapchain_loader.acquire_next_image(
            state.swapchain_resources.swapchain, u64::MAX, state.image_available_semaphores[state.current_frame], vk::Fence::null(),
        );
        let image_index = match result {
            Ok((index, _)) => index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                recreate_swapchain_and_dependents(state)?;
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
        let image_in_flight_fence = state.images_in_flight[image_index as usize];
        if image_in_flight_fence != vk::Fence::null() {
            state.device.wait_for_fences(&[image_in_flight_fence], true, u64::MAX)?;
        }
        state.images_in_flight[image_index as usize] = fence;

        state.device.reset_fences(&[fence])?;
        let cmd = state.command_buffers[state.current_frame];
        state.device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
        let begin_info = vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        state.device.begin_command_buffer(cmd, &begin_info)?;

        let c = screen.clear_color;
        let clear_value = vk::ClearValue { color: vk::ClearColorValue { float32: [c[0], c[1], c[2], c[3]] } };
        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(state.render_pass)
            .framebuffer(state.swapchain_resources.framebuffers[image_index as usize])
            .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: state.swapchain_resources.extent })
            .clear_values(std::slice::from_ref(&clear_value));
        
        state.device.cmd_begin_render_pass(cmd, &render_pass_info, vk::SubpassContents::INLINE);

        if let (Some(vertex_buffer), Some(index_buffer)) = (&state.vertex_buffer, &state.index_buffer) {
            let viewport = vk::Viewport {
                x: 0.0, y: state.swapchain_resources.extent.height as f32, width: state.swapchain_resources.extent.width as f32,
                height: -(state.swapchain_resources.extent.height as f32), min_depth: 0.0, max_depth: 1.0,
            };
            state.device.cmd_set_viewport(cmd, 0, &[viewport]);
            let scissor = vk::Rect2D { offset: vk::Offset2D::default(), extent: state.swapchain_resources.extent };
            state.device.cmd_set_scissor(cmd, 0, &[scissor]);

            state.device.cmd_bind_vertex_buffers(cmd, 0, &[vertex_buffer.buffer], &[0]);
            state.device.cmd_bind_index_buffer(cmd, index_buffer.buffer, 0, vk::IndexType::UINT16);

            let proj = create_projection_matrix(state.window_size.width, state.window_size.height);

            for (i, object) in screen.objects.iter().enumerate() {
                if let Some(draw_info) = state.object_draw_info.get(i) {
                    match &object.object_type {
                        ObjectType::SolidColor { color } => {
                            state.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, state.solid_pipeline);
                            let push_constants = SolidPushConstants { mvp: proj * object.transform, color: *color };
                            let bytes = std::slice::from_raw_parts(&push_constants as *const _ as *const u8, mem::size_of::<SolidPushConstants>());
                            state.device.cmd_push_constants(cmd, state.solid_pipeline_layout, vk::ShaderStageFlags::VERTEX, 0, bytes);
                        }
                        ObjectType::Textured { texture_id } => {
                            state.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, state.texture_pipeline);
                            if let Some(renderer::Texture::Vulkan(texture)) = textures.get(texture_id) {
                                state.device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, state.texture_pipeline_layout, 0, &[texture.descriptor_set], &[]);
                                let push_constants = TexturedPushConstants { mvp: proj * object.transform };
                                let bytes = std::slice::from_raw_parts(&push_constants as *const _ as *const u8, mem::size_of::<TexturedPushConstants>());
                                state.device.cmd_push_constants(cmd, state.texture_pipeline_layout, vk::ShaderStageFlags::VERTEX, 0, bytes);
                            } else {
                                warn!("Vulkan texture ID '{}' not found in texture manager!", texture_id);
                                continue;
                            }
                        }
                    }
                    state.device.cmd_draw_indexed(cmd, draw_info.index_count, 1, draw_info.first_index, 0, 0);
                }
            }
        }
        
        state.device.cmd_end_render_pass(cmd);
        state.device.end_command_buffer(cmd)?;

        let wait_semaphores = [state.image_available_semaphores[state.current_frame]];
        let signal_semaphores = [state.render_finished_semaphores[state.current_frame]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let cmd_buffers = [cmd]; // This is the fix

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&cmd_buffers) // Use the binding
            .signal_semaphores(&signal_semaphores);

        state.device.queue_submit(state.queue, &[submit_info], fence)?;

        // FIX: Create bindings for slices to extend their lifetimes.
        let swapchains = [state.swapchain_resources.swapchain];
        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains) // Use the binding
            .image_indices(&image_indices); // Use the binding
        
        let present_result = state.swapchain_resources.swapchain_loader.queue_present(state.queue, &present_info);

        let is_out_of_date = matches!(present_result, Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR));
        if is_out_of_date {
            recreate_swapchain_and_dependents(state)?;
        } else if let Err(e) = present_result {
            return Err(e.into());
        }

        state.current_frame = (state.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }
    Ok(())
}

pub fn cleanup(state: &mut State) {
    info!("Cleaning up Vulkan resources...");
    unsafe {
        state.device.device_wait_idle().unwrap();

        // The Texture `Drop` impl handles destroying individual textures.

        cleanup_swapchain_and_dependents(state);
        
        for i in 0..MAX_FRAMES_IN_FLIGHT {
            state.device.destroy_semaphore(state.render_finished_semaphores[i], None);
            state.device.destroy_semaphore(state.image_available_semaphores[i], None);
            state.device.destroy_fence(state.in_flight_fences[i], None);
        }

        if let Some(buffer) = state.vertex_buffer.take() { destroy_buffer(&state.device, &buffer); }
        if let Some(buffer) = state.index_buffer.take() { destroy_buffer(&state.device, &buffer); }

        state.device.destroy_sampler(state.sampler, None);
        state.device.destroy_descriptor_pool(state.descriptor_pool, None);
        state.device.destroy_descriptor_set_layout(state.descriptor_set_layout, None);
        
        state.device.destroy_pipeline(state.solid_pipeline, None);
        state.device.destroy_pipeline_layout(state.solid_pipeline_layout, None);
        state.device.destroy_pipeline(state.texture_pipeline, None);
        state.device.destroy_pipeline_layout(state.texture_pipeline_layout, None);

        state.device.destroy_render_pass(state.render_pass, None);
        state.device.destroy_command_pool(state.command_pool, None);
        state.surface_loader.destroy_surface(state.surface, None);

        if let (Some(loader), Some(messenger)) = (&state.debug_loader, state.debug_messenger) {
            loader.destroy_debug_utils_messenger(messenger, None);
        }
        
        // The device is now owned by an Arc, it will be dropped automatically
        // when the last Arc (in State and Textures) goes out of scope.
        // We no longer need to manually destroy it here.

        state.instance.destroy_instance(None);
    }
    info!("Vulkan resources cleaned up.");
}

pub fn resize(state: &mut State, width: u32, height: u32) {
    info!("Vulkan resize requested to {}x{}", width, height);
    state.window_size = PhysicalSize::new(width, height);
    if width > 0 && height > 0 {
        if let Err(e) = recreate_swapchain_and_dependents(state) {
            error!("Failed to recreate swapchain: {}", e);
        }
    }
}

// --- ALL HELPER FUNCTIONS ---

fn create_projection_matrix(width: u32, height: u32) -> Matrix4<f32> {
    let aspect_ratio = width as f32 / height as f32;
    let (ortho_width, ortho_height) = if aspect_ratio >= 1.0 {
        (400.0 * aspect_ratio, 400.0)
    } else {
        (400.0, 400.0 / aspect_ratio)
    };
    cgmath::ortho(-ortho_width, ortho_width, -ortho_height, ortho_height, -1.0, 1.0)
}

// --- Image & Texture Helpers ---

fn create_image_view(device: &Device, image: vk::Image, format: vk::Format) -> Result<vk::ImageView, vk::Result> {
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    unsafe { device.create_image_view(&view_info, None) }
}

fn create_image(
    state: &State, width: u32, height: u32, format: vk::Format, tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags, properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Image, vk::DeviceMemory), vk::Result> {
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .format(format)
        .tiling(tiling)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .usage(usage)
        .samples(vk::SampleCountFlags::TYPE_1)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    unsafe {
        let image = state.device.create_image(&image_info, None)?;
        let mem_requirements = state.device.get_image_memory_requirements(image);
        let mem_type_index = find_memory_type(&state.instance, state.pdevice, mem_requirements.memory_type_bits, properties);
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(mem_type_index);
        let memory = state.device.allocate_memory(&alloc_info, None)?;
        state.device.bind_image_memory(image, memory, 0)?;
        Ok((image, memory))
    }
}

fn transition_image_layout(
    state: &State, image: vk::Image, old_layout: vk::ImageLayout, new_layout: vk::ImageLayout,
) -> Result<(), Box<dyn Error>> {
    // CORRECTED: Pass the correct arguments instead of the whole `state` struct.
    let cmd = begin_single_time_commands(&state.device, state.command_pool)?;
    let (src_access_mask, dst_access_mask, src_stage, dst_stage) = match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
            vk::AccessFlags::empty(), vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER,
        ),
        (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_WRITE, vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
        ),
        _ => return Err("Unsupported layout transition!".into()),
    };

    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .src_access_mask(src_access_mask)
        .dst_access_mask(dst_access_mask);

    unsafe {
        state.device.cmd_pipeline_barrier(cmd, src_stage, dst_stage, vk::DependencyFlags::empty(), &[], &[], &[barrier]);
    }

    // CORRECTED: Pass the correct arguments instead of the whole `state` struct.
    end_single_time_commands(&state.device, state.command_pool, state.queue, cmd)?;
    Ok(())
}

fn copy_buffer_to_image(
    state: &State, buffer: vk::Buffer, image: vk::Image, width: u32, height: u32,
) -> Result<(), Box<dyn Error>> {
    // CORRECTED: Pass the correct arguments instead of the whole `state` struct.
    let cmd = begin_single_time_commands(&state.device, state.command_pool)?;
    let region = vk::BufferImageCopy::default()
        .buffer_offset(0)
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        })
        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
        .image_extent(vk::Extent3D { width, height, depth: 1 });

    unsafe {
        state.device.cmd_copy_buffer_to_image(cmd, buffer, image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
    }
    // CORRECTED: Pass the correct arguments instead of the whole `state` struct.
    end_single_time_commands(&state.device, state.command_pool, state.queue, cmd)?;
    Ok(())
}

fn create_texture_descriptor_set(
    state: &mut State, texture_image_view: vk::ImageView, sampler: vk::Sampler,
) -> Result<vk::DescriptorSet, vk::Result> {
    let layouts = [state.descriptor_set_layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(state.descriptor_pool)
        .set_layouts(&layouts);
    let descriptor_set = unsafe { state.device.allocate_descriptor_sets(&alloc_info)?[0] };

    let image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(texture_image_view)
        .sampler(sampler);

    let descriptor_write = vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(0)
        .dst_array_element(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(std::slice::from_ref(&image_info));

    unsafe {
        state.device.update_descriptor_sets(&[descriptor_write], &[]);
    }
    Ok(descriptor_set)
}

// --- Buffer & Command Helpers ---

fn begin_single_time_commands(device: &Device, pool: vk::CommandPool) -> Result<vk::CommandBuffer, vk::Result> {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_pool(pool)
        .command_buffer_count(1);
    let cmd = unsafe { device.allocate_command_buffers(&alloc_info)?[0] };
    let begin_info = vk::CommandBufferBeginInfo::default()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe {
        device.begin_command_buffer(cmd, &begin_info)?;
    }
    Ok(cmd)
}

fn end_single_time_commands(device: &Device, pool: vk::CommandPool, queue: vk::Queue, command_buffer: vk::CommandBuffer) -> Result<(), Box<dyn Error>> {
    unsafe {
        device.end_command_buffer(command_buffer)?;
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
        device.queue_submit(queue, &[submit_info], vk::Fence::null())?;
        device.queue_wait_idle(queue)?;
        device.free_command_buffers(pool, &[command_buffer]);
    }
    Ok(())
}

fn create_buffer_with_data_raw(
    instance: &Instance, device: &Device, pdevice: vk::PhysicalDevice, size: vk::DeviceSize,
    usage: vk::BufferUsageFlags, properties: vk::MemoryPropertyFlags,
) -> Result<BufferResource, Box<dyn Error>> {
    let (buffer, memory) = create_gpu_buffer(instance, device, pdevice, size, usage, properties)?;
    Ok(BufferResource { buffer, memory })
}

fn create_buffer_with_data_vec<T: Copy>(
    instance: &Instance, device: &Device, pdevice: vk::PhysicalDevice, pool: vk::CommandPool,
    queue: vk::Queue, data: &[T], usage: vk::BufferUsageFlags,
) -> Result<BufferResource, Box<dyn Error>> {
    let buffer_size = (mem::size_of::<T>() * data.len()) as vk::DeviceSize;
    let staging_buffer = create_buffer_with_data_raw(
        instance, device, pdevice, buffer_size, vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    unsafe {
        let mapped = device.map_memory(staging_buffer.memory, 0, buffer_size, vk::MemoryMapFlags::empty())?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), mapped as *mut T, data.len());
        device.unmap_memory(staging_buffer.memory);
    }
    let device_buffer = create_buffer_with_data_raw(
        instance, device, pdevice, buffer_size, usage | vk::BufferUsageFlags::TRANSFER_DST,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    copy_buffer(device, pool, queue, staging_buffer.buffer, device_buffer.buffer, buffer_size)?;
    destroy_buffer(device, &staging_buffer);
    Ok(device_buffer)
}

fn copy_buffer(
    device: &Device, pool: vk::CommandPool, queue: vk::Queue,
    src: vk::Buffer, dst: vk::Buffer, size: vk::DeviceSize,
) -> Result<(), Box<dyn Error>> {
    let cmd = begin_single_time_commands(device, pool)?;
    unsafe {
        let region = vk::BufferCopy::default().size(size);
        device.cmd_copy_buffer(cmd, src, dst, &[region]);
    }
    end_single_time_commands(device, pool, queue, cmd)?;
    Ok(())
}

fn create_gpu_buffer(
    instance: &Instance, device: &Device, pdevice: vk::PhysicalDevice, size: vk::DeviceSize,
    usage: vk::BufferUsageFlags, properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), Box<dyn Error>> {
    let buffer_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&buffer_info, None)? };
    let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let mem_type_index = find_memory_type(instance, pdevice, mem_requirements.memory_type_bits, properties);
    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_requirements.size)
        .memory_type_index(mem_type_index);
    let memory = unsafe { device.allocate_memory(&alloc_info, None)? };
    unsafe { device.bind_buffer_memory(buffer, memory, 0)? };
    Ok((buffer, memory))
}

fn destroy_buffer(device: &Device, buffer: &BufferResource) {
    unsafe {
        device.destroy_buffer(buffer.buffer, None);
        device.free_memory(buffer.memory, None);
    }
}

fn find_memory_type(
    instance: &Instance, pdevice: vk::PhysicalDevice, type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> u32 {
    let mem_properties = unsafe { instance.get_physical_device_memory_properties(pdevice) };
    (0..mem_properties.memory_type_count)
        .find(|i| {
            let i_usize = *i as usize;
            (type_filter & (1 << i)) != 0
                && (mem_properties.memory_types[i_usize].property_flags & properties) == properties
        })
        .expect("Failed to find suitable memory type!")
}

// --- Original Base Helpers ---

fn create_instance(entry: &Entry, window: &Window) -> Result<Instance, Box<dyn Error>> {
    let app_name = ffi::CStr::from_bytes_with_nul(b"Simple Renderer\0")?;
    let app_info = vk::ApplicationInfo::default()
        .application_name(app_name)
        .application_version(vk::make_api_version(0, 1, 0, 0))
        .engine_name(ffi::CStr::from_bytes_with_nul(b"No Engine\0")?)
        .engine_version(vk::make_api_version(0, 1, 0, 0))
        .api_version(vk::API_VERSION_1_3);

    let mut extension_names = ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?.to_vec();
    if cfg!(debug_assertions) {
        extension_names.push(ash::ext::debug_utils::NAME.as_ptr());
    }

    let layers_names_raw: Vec<*const ffi::c_char> = if cfg!(debug_assertions) {
        vec![ffi::CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")?.as_ptr()]
    } else {
        vec![]
    };

    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&extension_names)
        .enabled_layer_names(&layers_names_raw);

    unsafe { Ok(entry.create_instance(&create_info, None)?) }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut ffi::c_void,
) -> vk::Bool32 {
    let message = unsafe { ffi::CStr::from_ptr((*p_callback_data).p_message) };
    let severity = format!("{:?}", message_severity).to_lowercase();
    let ty = format!("{:?}", message_type).to_lowercase();
    let log_message = format!("[vulkan_{}_{}] {}", severity, ty, message.to_string_lossy());

    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => error!("{}", log_message),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("{}", log_message),
        _ => debug!("{}", log_message),
    }
    vk::FALSE
}

fn setup_debug_messenger(
    entry: &Entry,
    instance: &Instance,
) -> Result<(Option<ash::ext::debug_utils::Instance>, Option<vk::DebugUtilsMessengerEXT>), vk::Result> {
    if !cfg!(debug_assertions) { return Ok((None, None)); }
    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
        // FIX: Use bitwise OR for flags
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));
    let loader = ash::ext::debug_utils::Instance::new(entry, instance);
    let messenger = unsafe { loader.create_debug_utils_messenger(&create_info, None)? };
    Ok((Some(loader), Some(messenger)))
}

fn create_surface(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<vk::SurfaceKHR, Box<dyn Error>> {
    unsafe {
        Ok(ash_window::create_surface(
            entry,
            instance,
            window.display_handle()?.as_raw(),
            window.window_handle()?.as_raw(),
            None,
        )?)
    }
}

fn select_physical_device(
    instance: &Instance,
    surface_loader: &surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<vk::PhysicalDevice, Box<dyn Error>> {
    let pdevices = unsafe { instance.enumerate_physical_devices()? };
    pdevices
        .into_iter()
        .find(|pdevice| is_device_suitable(instance, *pdevice, surface_loader, surface))
        .ok_or_else(|| "Failed to find a suitable GPU!".into())
}

fn is_device_suitable(
    instance: &Instance,
    pdevice: vk::PhysicalDevice,
    surface_loader: &surface::Instance,
    surface: vk::SurfaceKHR,
) -> bool {
    find_queue_family(instance, pdevice, surface_loader, surface).is_some()
}

fn find_queue_family(
    instance: &Instance,
    pdevice: vk::PhysicalDevice,
    surface_loader: &surface::Instance,
    surface: vk::SurfaceKHR,
) -> Option<u32> {
    let queue_families = unsafe { instance.get_physical_device_queue_family_properties(pdevice) };
    queue_families.iter().enumerate().find_map(|(i, family)| {
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            && unsafe { surface_loader.get_physical_device_surface_support(pdevice, i as u32, surface).unwrap_or(false) }
        {
            Some(i as u32)
        } else {
            None
        }
    })
}

fn create_logical_device(
    instance: &Instance,
    pdevice: vk::PhysicalDevice,
    surface_loader: &surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(Device, vk::Queue, u32), Box<dyn Error>> {
    let queue_family_index = find_queue_family(instance, pdevice, surface_loader, surface)
        .ok_or("No suitable queue family found")?;
    let queue_priorities = [1.0];
    let queue_create_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&queue_priorities);
    let device_extensions = [swapchain::NAME.as_ptr()];
    let features = vk::PhysicalDeviceFeatures::default();
    let create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_create_info))
        .enabled_extension_names(&device_extensions)
        .enabled_features(&features);

    let device = unsafe { instance.create_device(pdevice, &create_info, None)? };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
    Ok((device, queue, queue_family_index))
}

fn create_swapchain(
    instance: &Instance,
    device: &Device,
    pdevice: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    surface_loader: &surface::Instance,
    window_size: PhysicalSize<u32>,
    old_swapchain: Option<vk::SwapchainKHR>,
) -> Result<SwapchainResources, Box<dyn Error>> {
    let capabilities = unsafe { surface_loader.get_physical_device_surface_capabilities(pdevice, surface)? };
    let formats = unsafe { surface_loader.get_physical_device_surface_formats(pdevice, surface)? };
    let present_modes = unsafe { surface_loader.get_physical_device_surface_present_modes(pdevice, surface)? };

    let format = formats.iter().find(|f| {
        f.format == vk::Format::B8G8R8A8_SRGB && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    }).cloned().unwrap_or(formats[0]);
    
    let present_mode = present_modes.iter().cloned().find(|&mode| mode == vk::PresentModeKHR::MAILBOX).unwrap_or(vk::PresentModeKHR::FIFO);

    let extent = if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        vk::Extent2D {
            width: window_size.width.clamp(capabilities.min_image_extent.width, capabilities.max_image_extent.width),
            height: window_size.height.clamp(capabilities.min_image_extent.height, capabilities.max_image_extent.height),
        }
    };

    let image_count = (capabilities.min_image_count + 1).min(if capabilities.max_image_count > 0 { capabilities.max_image_count } else { u32::MAX });

    let create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface).min_image_count(image_count).image_format(format.format)
        .image_color_space(format.color_space).image_extent(extent).image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT).image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(capabilities.current_transform).composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode).clipped(true).old_swapchain(old_swapchain.unwrap_or(vk::SwapchainKHR::null()));

    let swapchain_loader = swapchain::Device::new(instance, device);
    let swapchain = unsafe { swapchain_loader.create_swapchain(&create_info, None)? };
    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
    let image_views = images.iter().map(|&image| create_image_view(device, image, format.format)).collect::<Result<Vec<_>, _>>()?;

    Ok(SwapchainResources { swapchain_loader, swapchain, _images: images, image_views, framebuffers: vec![], extent, format })
}

fn recreate_framebuffers(
    device: &Device,
    swapchain_resources: &mut SwapchainResources,
    render_pass: vk::RenderPass,
) -> Result<(), vk::Result> {
    swapchain_resources.framebuffers = swapchain_resources.image_views.iter().map(|view| {
        // FIX: Create binding to extend lifetime of slice
        let attachments = [*view];
        let create_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(swapchain_resources.extent.width)
            .height(swapchain_resources.extent.height)
            .layers(1);
        unsafe { device.create_framebuffer(&create_info, None) }
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

fn create_render_pass(device: &Device, format: vk::Format) -> Result<vk::RenderPass, vk::Result> {
    let color_attachment = vk::AttachmentDescription::default()
        .format(format).samples(vk::SampleCountFlags::TYPE_1).load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE).stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE).initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
    let color_attachment_ref = vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(std::slice::from_ref(&color_attachment_ref));
    let dependency = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL).dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
    let create_info = vk::RenderPassCreateInfo::default()
        .attachments(std::slice::from_ref(&color_attachment))
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(std::slice::from_ref(&dependency));
    unsafe { device.create_render_pass(&create_info, None) }
}

fn create_command_pool(device: &Device, queue_family_index: u32) -> Result<vk::CommandPool, vk::Result> {
    let create_info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    unsafe { device.create_command_pool(&create_info, None) }
}

fn create_command_buffers(device: &Device, pool: vk::CommandPool, count: usize) -> Result<Vec<vk::CommandBuffer>, vk::Result> {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(count as u32);
    unsafe { device.allocate_command_buffers(&alloc_info) }
}

fn create_shader_module(device: &Device, code: &[u8]) -> Result<vk::ShaderModule, vk::Result> {
    let code_u32 = ash::util::read_spv(&mut std::io::Cursor::new(code)).unwrap();
    let create_info = vk::ShaderModuleCreateInfo::default().code(&code_u32);
    unsafe { device.create_shader_module(&create_info, None) }
}

fn create_sync_objects(device: &Device) -> Result<(Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>), vk::Result> {
    let semaphore_info = vk::SemaphoreCreateInfo::default();
    let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
    let mut image_available = vec![];
    let mut render_finished = vec![];
    let mut in_flight_fences = vec![];
    for _ in 0..MAX_FRAMES_IN_FLIGHT {
        image_available.push(unsafe { device.create_semaphore(&semaphore_info, None)? });
        render_finished.push(unsafe { device.create_semaphore(&semaphore_info, None)? });
        in_flight_fences.push(unsafe { device.create_fence(&fence_info, None)? });
    }
    Ok((image_available, render_finished, in_flight_fences))
}

fn cleanup_swapchain_and_dependents(state: &mut State) {
    unsafe {
        for &framebuffer in &state.swapchain_resources.framebuffers {
            state.device.destroy_framebuffer(framebuffer, None);
        }
        for &view in &state.swapchain_resources.image_views {
            state.device.destroy_image_view(view, None);
        }
        state.swapchain_resources.swapchain_loader.destroy_swapchain(state.swapchain_resources.swapchain, None);
    }
}

fn recreate_swapchain_and_dependents(state: &mut State) -> Result<(), Box<dyn Error>> {
    debug!("Recreating swapchain...");
    unsafe { state.device.device_wait_idle()? };
    cleanup_swapchain_and_dependents(state);
    state.swapchain_resources = create_swapchain(
        &state.instance, &state.device, state.pdevice, state.surface, &state.surface_loader, state.window_size, None,
    )?;
    recreate_framebuffers(&state.device, &mut state.swapchain_resources, state.render_pass)?;
    state.images_in_flight = vec![vk::Fence::null(); state.swapchain_resources._images.len()];
    debug!("Swapchain recreated.");
    Ok(())
}
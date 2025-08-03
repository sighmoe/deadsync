#![warn(
    clippy::use_self,
    deprecated_in_future,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications
)]

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk, Device, Entry, Instance,
};
use cgmath::Matrix4;
use log::{debug, error, info, warn};
use std::{
    borrow::Cow, default::Default, error::Error, ffi, mem::size_of, ops::Drop, os::raw::c_char,
};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowId},
};

// --- Constants ---
const MAX_FRAMES_IN_FLIGHT: u32 = 2;

// --- Helper Functions ---
pub fn record_submit_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    command_buffer_reuse_fence: vk::Fence,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .wait_for_fences(&[command_buffer_reuse_fence], true, u64::MAX)
            .expect("Wait for fence failed.");
        device
            .reset_fences(&[command_buffer_reuse_fence])
            .expect("Reset fences failed.");
        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(std::slice::from_ref(&command_buffer))
            .signal_semaphores(signal_semaphores);
        device
            .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
            .expect("queue submit failed.");
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = &*p_callback_data;
    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };
    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };
    log::debug!(
        "{:?}: {:?} [{}({})] : {}\n",
        message_severity,
        message_type,
        message_id_name,
        callback_data.message_id_number,
        message
    );
    vk::FALSE
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as usize]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags.contains(flags)
        })
        .map(|(index, _memory_type)| index as u32)
}


// --- Struct Definitions ---
#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub tex_coord: [f32; 2],
}
#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct UniformBufferObject {
    pub projection: Matrix4<f32>,
}

pub struct BufferResource {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
    pub mapped_ptr: Option<*mut ffi::c_void>,
}
impl BufferResource {
    pub fn destroy(&mut self, device: &Device) {
        unsafe {
            if let Some(ptr) = self.mapped_ptr {
                if !ptr.is_null() {
                    device.unmap_memory(self.memory);
                }
            }
            if self.buffer != vk::Buffer::null() {
                device.destroy_buffer(self.buffer, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                device.free_memory(self.memory, None);
            }
            self.buffer = vk::Buffer::null();
            self.memory = vk::DeviceMemory::null();
        }
    }
}

pub struct SwapchainResources {
    pub swapchain_loader: swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,
    pub present_image_views: Vec<vk::ImageView>,
    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,
    pub framebuffers: Vec<vk::Framebuffer>,
}

pub struct VulkanBase {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub surface_loader: surface::Instance,
    pub debug_utils_loader: debug_utils::Instance,
    pub window: Window,
    pub debug_call_back: Option<vk::DebugUtilsMessengerEXT>,
    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub pdevice_properties: vk::PhysicalDeviceProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,
    pub surface: vk::SurfaceKHR,
    pub pool: vk::CommandPool,
    pub draw_command_buffers: Vec<vk::CommandBuffer>,
    pub setup_command_buffer: vk::CommandBuffer,
    pub render_pass: vk::RenderPass,
    pub present_complete_semaphores: Vec<vk::Semaphore>,
    pub rendering_complete_semaphores: Vec<vk::Semaphore>,
    pub draw_commands_fences: Vec<vk::Fence>,
    pub setup_commands_reuse_fence: vk::Fence,
    frame_index: usize,
    pub swapchain_resources: SwapchainResources,
}

// --- Procedural Functions ---

unsafe fn init_instance_and_debug(
    entry: &Entry,
    window: &Window,
) -> Result<
    (
        Instance,
        debug_utils::Instance,
        Option<vk::DebugUtilsMessengerEXT>,
    ),
    Box<dyn Error>,
> {
    let app_name = ffi::CStr::from_bytes_with_nul(b"DeadSyncVulkan\0")?;
    let layer_names = [ffi::CStr::from_bytes_with_nul(
        b"VK_LAYER_KHRONOS_validation\0",
    )?];
    let layers_names_raw: Vec<*const c_char> = layer_names
        .iter()
        .map(|raw_name| raw_name.as_ptr())
        .collect();
    let mut extension_names =
        ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?.to_vec();
    extension_names.push(debug_utils::NAME.as_ptr());
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        extension_names.push(ash::khr::portability_enumeration::NAME.as_ptr());
    }

    let appinfo = vk::ApplicationInfo::default()
        .application_name(app_name)
        .application_version(vk::make_api_version(0, 0, 1, 0))
        .engine_name(app_name)
        .engine_version(vk::make_api_version(0, 0, 1, 0))
        .api_version(vk::API_VERSION_1_1);
    let create_flags = if cfg!(any(target_os = "macos", target_os = "ios")) {
        vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
    } else {
        vk::InstanceCreateFlags::default()
    };
    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&appinfo)
        .enabled_layer_names(&layers_names_raw)
        .enabled_extension_names(&extension_names)
        .flags(create_flags);
    let instance = entry
        .create_instance(&create_info, None)
        .map_err(|e| format!("Instance creation error: {}", e))?;

    let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(vulkan_debug_callback));
    let debug_utils_loader = debug_utils::Instance::new(entry, &instance);
    let debug_call_back = debug_utils_loader
        .create_debug_utils_messenger(&debug_info, None)
        .ok();
    Ok((instance, debug_utils_loader, debug_call_back))
}

unsafe fn init_surface_and_select_physical_device(
    entry: &Entry,
    instance: &Instance,
    window: &Window,
) -> Result<
    (
        surface::Instance,
        vk::SurfaceKHR,
        vk::PhysicalDevice,
        u32,
        vk::PhysicalDeviceProperties,
        vk::PhysicalDeviceMemoryProperties,
    ),
    Box<dyn Error>,
> {
    let surface = ash_window::create_surface(
        entry,
        instance,
        window.display_handle()?.as_raw(),
        window.window_handle()?.as_raw(),
        None,
    )?;
    let surface_loader = surface::Instance::new(entry, instance);
    let pdevices = instance.enumerate_physical_devices()?;
    let (pdevice, queue_family_index) = pdevices
        .iter()
        .find_map(|pdevice| {
            instance
                .get_physical_device_queue_family_properties(*pdevice)
                .iter()
                .enumerate()
                .find_map(|(index, info)| {
                    if info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        && surface_loader
                            .get_physical_device_surface_support(
                                *pdevice,
                                index as u32,
                                surface,
                            )
                            .unwrap_or(false)
                    {
                        Some((*pdevice, index as u32))
                    } else {
                        None
                    }
                })
        })
        .ok_or("Couldn't find suitable physical device.")?;
    let pdevice_properties = instance.get_physical_device_properties(pdevice);
    let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
    Ok((
        surface_loader,
        surface,
        pdevice,
        queue_family_index,
        pdevice_properties,
        device_memory_properties,
    ))
}

unsafe fn create_logical_device(
    instance: &Instance,
    pdevice: vk::PhysicalDevice,
    queue_family_index: u32,
) -> Result<(Device, vk::Queue), Box<dyn Error>> {
    let device_extension_names_raw = [
        swapchain::NAME.as_ptr(),
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        ash::khr::portability_subset::NAME.as_ptr(),
    ];
    let features = vk::PhysicalDeviceFeatures {
        sampler_anisotropy: vk::TRUE,
        ..Default::default()
    };
    let priorities = [1.0];
    let queue_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&priorities);
    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_info))
        .enabled_extension_names(&device_extension_names_raw)
        .enabled_features(&features);
    let device = instance
        .create_device(pdevice, &device_create_info, None)
        .map_err(|e| format!("Device creation error: {}", e))?;
    let present_queue = device.get_device_queue(queue_family_index, 0);
    Ok((device, present_queue))
}

unsafe fn init_command_pool_and_buffers(
    device: &Device,
    queue_family_index: u32,
) -> Result<(vk::CommandPool, vk::CommandBuffer, Vec<vk::CommandBuffer>), Box<dyn Error>> {
    let pool_create_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
        .queue_family_index(queue_family_index);
    let pool = device.create_command_pool(&pool_create_info, None)?;
    let cmd_buf_alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(MAX_FRAMES_IN_FLIGHT + 1);
    let all_cmd_bufs = device.allocate_command_buffers(&cmd_buf_alloc_info)?;
    let setup_command_buffer = all_cmd_bufs[0];
    let draw_command_buffers = all_cmd_bufs[1..].to_vec();
    Ok((pool, setup_command_buffer, draw_command_buffers))
}

unsafe fn create_main_render_pass(
    device: &Device,
    surface_format: vk::Format,
) -> Result<vk::RenderPass, Box<dyn Error>> {
    let attachments = [
        vk::AttachmentDescription {
            format: surface_format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        },
        vk::AttachmentDescription {
            format: vk::Format::D16_UNORM,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::DONT_CARE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ..Default::default()
        },
    ];
    let color_attachment_refs = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let depth_attachment_ref = vk::AttachmentReference {
        attachment: 1,
        layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    };
    let dependencies = [vk::SubpassDependency {
        src_subpass: vk::SUBPASS_EXTERNAL,
        dst_subpass: 0,
        src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
        src_access_mask: vk::AccessFlags::NONE,
        dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ..Default::default()
    }];
    let subpass = vk::SubpassDescription::default()
        .color_attachments(&color_attachment_refs)
        .depth_stencil_attachment(&depth_attachment_ref)
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);
    let renderpass_create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(std::slice::from_ref(&subpass))
        .dependencies(&dependencies);
    Ok(device.create_render_pass(&renderpass_create_info, None)?)
}

unsafe fn init_synchronization_objects(
    device: &Device,
) -> Result<
    (
        Vec<vk::Semaphore>,
        Vec<vk::Semaphore>,
        Vec<vk::Fence>,
        vk::Fence,
    ),
    Box<dyn Error>,
> {
    let semaphore_create_info = vk::SemaphoreCreateInfo::default();
    let fence_create_info =
        vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

    let mut present_complete_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
    let mut rendering_complete_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
    let mut draw_commands_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);

    for _ in 0..MAX_FRAMES_IN_FLIGHT {
        present_complete_semaphores
            .push(device.create_semaphore(&semaphore_create_info, None)?);
        rendering_complete_semaphores
            .push(device.create_semaphore(&semaphore_create_info, None)?);
        draw_commands_fences.push(device.create_fence(&fence_create_info, None)?);
    }
    let setup_commands_reuse_fence = device.create_fence(
        &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
        None,
    )?;
    Ok((
        present_complete_semaphores,
        rendering_complete_semaphores,
        draw_commands_fences,
        setup_commands_reuse_fence,
    ))
}

unsafe fn select_surface_format(
    pdevice: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    surface_loader: &surface::Instance,
) -> Result<vk::SurfaceFormatKHR, Box<dyn Error>> {
    let formats = surface_loader.get_physical_device_surface_formats(pdevice, surface)?;
    let selected_format = formats
        .iter()
        .find(|f| f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM)
        .map(|f| *f)
        .unwrap_or_else(|| {
            warn!("B8G8R8A8_UNORM not found, using first available format.");
            formats[0]
        });
    Ok(selected_format)
}

unsafe fn destroy_swapchain_resources(device: &Device, resources: &mut SwapchainResources) {
    debug!("Destroying swapchain resources...");
    for framebuffer in resources.framebuffers.drain(..) {
        if framebuffer != vk::Framebuffer::null() {
            device.destroy_framebuffer(framebuffer, None);
        }
    }
    if resources.depth_image_view != vk::ImageView::null() {
        device.destroy_image_view(resources.depth_image_view, None);
    }
    if resources.depth_image != vk::Image::null() {
        device.destroy_image(resources.depth_image, None);
    }
    if resources.depth_image_memory != vk::DeviceMemory::null() {
        device.free_memory(resources.depth_image_memory, None);
    }
    for view in resources.present_image_views.drain(..) {
        if view != vk::ImageView::null() {
            device.destroy_image_view(view, None);
        }
    }
    if resources.swapchain != vk::SwapchainKHR::null() {
        resources.swapchain_loader.destroy_swapchain(resources.swapchain, None);
    }
    debug!("Swapchain resources destroyed.");
}

unsafe fn create_swapchain_resources(
    instance: &Instance,
    device: &Device,
    pdevice: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    surface_loader: &surface::Instance,
    device_memory_properties: &vk::PhysicalDeviceMemoryProperties,
    render_pass: vk::RenderPass,
    _setup_command_buffer: vk::CommandBuffer,
    _setup_commands_reuse_fence: vk::Fence,
    _present_queue: vk::Queue,
    surface_format: vk::SurfaceFormatKHR,
    new_width: u32,
    new_height: u32,
) -> Result<SwapchainResources, Box<dyn Error>> {
    let swapchain_loader = swapchain::Device::new(instance, device);
    let surface_capabilities =
        surface_loader.get_physical_device_surface_capabilities(pdevice, surface)?;

    let mut surface_resolution = match surface_capabilities.current_extent.width {
        u32::MAX => vk::Extent2D { width: new_width, height: new_height },
        _ => surface_capabilities.current_extent,
    };
    surface_resolution.width = surface_resolution.width.clamp(
        surface_capabilities.min_image_extent.width.max(1),
        surface_capabilities.max_image_extent.width,
    );
    surface_resolution.height = surface_resolution.height.clamp(
        surface_capabilities.min_image_extent.height.max(1),
        surface_capabilities.max_image_extent.height,
    );
    if surface_resolution.width == 0 || surface_resolution.height == 0 {
        return Err("Cannot create swapchain with zero size.".into());
    }

    let image_count = (surface_capabilities.min_image_count + 1)
        .min(surface_capabilities.max_image_count.max(surface_capabilities.min_image_count + 1));

    let pre_transform = if surface_capabilities.supported_transforms.contains(vk::SurfaceTransformFlagsKHR::IDENTITY) {
        vk::SurfaceTransformFlagsKHR::IDENTITY
    } else {
        surface_capabilities.current_transform
    };

    let present_mode = surface_loader
        .get_physical_device_surface_present_modes(pdevice, surface)?
        .into_iter()
        .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO);

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(image_count)
        .image_color_space(surface_format.color_space)
        .image_format(surface_format.format)
        .image_extent(surface_resolution)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(pre_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .image_array_layers(1);

    let swapchain = swapchain_loader.create_swapchain(&swapchain_create_info, None)?;
    let present_images = swapchain_loader.get_swapchain_images(swapchain)?;

    let present_image_views = present_images
        .iter()
        .map(|&image| {
            let create_view_info = vk::ImageViewCreateInfo::default()
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_format.format)
                .components(vk::ComponentMapping::default())
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image(image);
            device.create_image_view(&create_view_info, None)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let depth_format = vk::Format::D16_UNORM;
    let depth_image_create_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(depth_format)
        .extent(surface_resolution.into())
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let depth_image = device.create_image(&depth_image_create_info, None)?;
    let depth_mem_req = device.get_image_memory_requirements(depth_image);
    let depth_mem_idx = find_memorytype_index(
        &depth_mem_req,
        device_memory_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or("Failed to find memory type for depth image")?;

    let depth_alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(depth_mem_req.size)
        .memory_type_index(depth_mem_idx);
    let depth_image_memory = device.allocate_memory(&depth_alloc_info, None)?;
    device.bind_image_memory(depth_image, depth_image_memory, 0)?;

    let depth_view_info = vk::ImageViewCreateInfo::default()
        .image(depth_image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(depth_format)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::DEPTH,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    let depth_image_view = device.create_image_view(&depth_view_info, None)?;

    let framebuffers = present_image_views
        .iter()
        .map(|&present_view| {
            let attachments = [present_view, depth_image_view];
            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(surface_resolution.width)
                .height(surface_resolution.height)
                .layers(1);
            device.create_framebuffer(&fb_info, None)
        })
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(SwapchainResources {
        swapchain_loader,
        swapchain,
        surface_format,
        surface_resolution,
        present_image_views,
        depth_image,
        depth_image_view,
        depth_image_memory,
        framebuffers,
    })
}

pub fn init(window: Window) -> Result<VulkanBase, Box<dyn Error>> {
    info!("Vulkan: Initializing...");
    unsafe {
        let entry = Entry::linked();
        let (instance, debug_utils_loader, debug_call_back) =
            init_instance_and_debug(&entry, &window)?;
        let (
            surface_loader,
            surface,
            pdevice,
            queue_family_index,
            pdevice_properties,
            device_memory_properties,
        ) = init_surface_and_select_physical_device(&entry, &instance, &window)?;
        let (device, present_queue) =
            create_logical_device(&instance, pdevice, queue_family_index)?;
        let (pool, setup_command_buffer, draw_command_buffers) =
            init_command_pool_and_buffers(&device, queue_family_index)?;
        
        let surface_format = select_surface_format(pdevice, surface, &surface_loader)?;
        let render_pass = create_main_render_pass(&device, surface_format.format)?;

        let (
            present_complete_semaphores,
            rendering_complete_semaphores,
            draw_commands_fences,
            setup_commands_reuse_fence,
        ) = init_synchronization_objects(&device)?;
        
        let initial_size = window.inner_size();
        let swapchain_resources = create_swapchain_resources(
            &instance,
            &device,
            pdevice,
            surface,
            &surface_loader,
            &device_memory_properties,
            render_pass,
            setup_command_buffer,
            setup_commands_reuse_fence,
            present_queue,
            surface_format,
            initial_size.width,
            initial_size.height,
        )?;

        info!("Vulkan: Initialization complete.");
        Ok(VulkanBase {
            entry,
            instance,
            device,
            surface_loader,
            debug_utils_loader,
            window,
            debug_call_back,
            pdevice,
            device_memory_properties,
            pdevice_properties,
            queue_family_index,
            present_queue,
            surface,
            pool,
            draw_command_buffers,
            setup_command_buffer,
            render_pass,
            present_complete_semaphores,
            rendering_complete_semaphores,
            draw_commands_fences,
            setup_commands_reuse_fence,
            frame_index: 0,
            swapchain_resources,
        })
    }
}

pub fn begin_frame(base: &mut VulkanBase) -> Result<Option<(vk::CommandBuffer, u32)>, vk::Result> {
    unsafe {
        let current_sync_idx = base.frame_index % MAX_FRAMES_IN_FLIGHT as usize;
        let fence = base.draw_commands_fences[current_sync_idx];
        let present_complete_semaphore = base.present_complete_semaphores[current_sync_idx];
        
        base.device.wait_for_fences(&[fence], true, u64::MAX)?;
        base.device.reset_fences(&[fence])?;

        let acquire_result = base.swapchain_resources.swapchain_loader.acquire_next_image(
            base.swapchain_resources.swapchain,
            u64::MAX,
            present_complete_semaphore,
            vk::Fence::null(),
        );

        let (present_index, suboptimal) = match acquire_result {
            Ok((index, suboptimal)) => (index, suboptimal),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(None), // Signal rebuild
            Err(e) => return Err(e),
        };

        if suboptimal {
            return Ok(None); // Signal rebuild
        }

        let command_buffer = base.draw_command_buffers[current_sync_idx];
        base.device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())?;
        let cmd_begin_info = vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        base.device.begin_command_buffer(command_buffer, &cmd_begin_info)?;

        Ok(Some((command_buffer, present_index)))
    }
}

pub unsafe fn end_frame(base: &mut VulkanBase, command_buffer: vk::CommandBuffer, present_index: u32) -> Result<bool, vk::Result> {
    base.device.end_command_buffer(command_buffer)?;

    let current_sync_idx = base.frame_index % MAX_FRAMES_IN_FLIGHT as usize;
    let fence = base.draw_commands_fences[current_sync_idx];
    let present_complete_semaphore = base.present_complete_semaphores[current_sync_idx];
    let rendering_complete_semaphore = base.rendering_complete_semaphores[current_sync_idx];

    let submit_info = vk::SubmitInfo::default()
        .wait_semaphores(std::slice::from_ref(&present_complete_semaphore))
        .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
        .command_buffers(std::slice::from_ref(&command_buffer))
        .signal_semaphores(std::slice::from_ref(&rendering_complete_semaphore));
    
    base.device.queue_submit(base.present_queue, &[submit_info], fence)?;

    let present_info = vk::PresentInfoKHR::default()
        .wait_semaphores(std::slice::from_ref(&rendering_complete_semaphore))
        .swapchains(std::slice::from_ref(&base.swapchain_resources.swapchain))
        .image_indices(std::slice::from_ref(&present_index));
    
    let present_result = base.swapchain_resources.swapchain_loader.queue_present(base.present_queue, &present_info);

    base.frame_index += 1;

    match present_result {
        Ok(suboptimal) => Ok(suboptimal),
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => Ok(true),
        Err(e) => Err(e),
    }
}


pub fn rebuild_swapchain_resources(
    base: &mut VulkanBase,
    new_width: u32,
    new_height: u32,
) -> Result<(), Box<dyn Error>> {
    wait_idle(base)?;
    info!("Rebuilding swapchain for size: {}x{}", new_width, new_height);

    unsafe {
        destroy_swapchain_resources(&base.device, &mut base.swapchain_resources);
        
        let surface_format = select_surface_format(base.pdevice, base.surface, &base.surface_loader)?;
        
        base.swapchain_resources = create_swapchain_resources(
            &base.instance,
            &base.device,
            base.pdevice,
            base.surface,
            &base.surface_loader,
            &base.device_memory_properties,
            base.render_pass,
            base.setup_command_buffer,
            base.setup_commands_reuse_fence,
            base.present_queue,
            surface_format,
            new_width,
            new_height,
        )?;
    }
    
    base.frame_index = 0;
    Ok(())
}

pub fn create_buffer(
    base: &VulkanBase,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    memory_flags: vk::MemoryPropertyFlags,
) -> Result<BufferResource, Box<dyn Error>> {
    unsafe {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = base.device.create_buffer(&buffer_info, None)?;
        let mem_requirements = base.device.get_buffer_memory_requirements(buffer);
        let mem_type_index = find_memorytype_index(
            &mem_requirements,
            &base.device_memory_properties,
            memory_flags,
        )
        .ok_or("Failed to find suitable memory type for buffer")?;
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(mem_type_index);
        let memory = base.device.allocate_memory(&alloc_info, None)?;
        base.device.bind_buffer_memory(buffer, memory, 0)?;

        let mapped_ptr = if memory_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
            match base.device.map_memory(
                memory,
                0,
                mem_requirements.size,
                vk::MemoryMapFlags::empty(),
            ) {
                Ok(ptr) => Some(ptr),
                Err(e) => {
                    warn!("Failed to map buffer memory (size {}): {}", mem_requirements.size, e);
                    base.device.destroy_buffer(buffer, None);
                    base.device.free_memory(memory, None);
                    return Err(e.into());
                }
            }
        } else {
            None
        };
        Ok(BufferResource {
            buffer,
            memory,
            size: mem_requirements.size,
            mapped_ptr,
        })
    }
}

pub fn update_buffer<T: Copy>(
    base: &VulkanBase,
    buffer_resource: &BufferResource,
    data: &[T],
) -> Result<(), Box<dyn Error>> {
    unsafe {
        let data_size = (data.len() * size_of::<T>()) as vk::DeviceSize;
        if data_size > buffer_resource.size {
            return Err(format!(
                "Data size ({}) exceeds buffer size ({})",
                data_size, buffer_resource.size
            )
            .into());
        }

        if let Some(mapped_ptr) = buffer_resource.mapped_ptr {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                mapped_ptr as *mut u8,
                data_size as usize,
            );

            let mem_requirements =
                base.device
                    .get_buffer_memory_requirements(buffer_resource.buffer);
            if let Some(mem_type_index) = find_memorytype_index(
                &mem_requirements,
                &base.device_memory_properties,
                vk::MemoryPropertyFlags::HOST_VISIBLE,
            ) {
                let mem_type =
                    &base.device_memory_properties.memory_types[mem_type_index as usize];
                if !mem_type
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
                {
                    let flush_range = vk::MappedMemoryRange::default()
                        .memory(buffer_resource.memory)
                        .offset(0)
                        .size(vk::WHOLE_SIZE);
                    base.device.flush_mapped_memory_ranges(&[flush_range])?;
                }
            } else {
                warn!("Could not find HOST_VISIBLE memory type index for a mapped buffer during update_buffer. Skipping flush check.");
            }
        } else {
            return Err("Buffer is not mapped (HOST_VISIBLE flag missing or map failed), cannot update directly.".into());
        }
    }
    Ok(())
}

pub fn wait_idle(base: &VulkanBase) -> Result<(), vk::Result> {
    debug!("Waiting for device idle...");
    unsafe { base.device.device_wait_idle()? };
    debug!("Device idle.");
    Ok(())
}

pub fn get_gpu_name(base: &VulkanBase) -> String {
    let name_bytes: Vec<u8> = base
        .pdevice_properties
        .device_name
        .iter()
        .map(|&c| c as u8)
        .take_while(|&c| c != 0)
        .collect();
    String::from_utf8_lossy(&name_bytes).into_owned()
}

pub fn get_window_id(base: &VulkanBase) -> WindowId {
    base.window.id()
}

impl Drop for VulkanBase {
    fn drop(&mut self) {
        info!("VulkanBase: Dropping resources...");
        unsafe {
            let _ = wait_idle(self);

            destroy_swapchain_resources(&self.device, &mut self.swapchain_resources);

            for fence in self.draw_commands_fences.drain(..) {
                self.device.destroy_fence(fence, None);
            }
            self.device.destroy_fence(self.setup_commands_reuse_fence, None);
            for semaphore in self.present_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore, None);
            }
            for semaphore in self.rendering_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore, None);
            }

            self.device.destroy_render_pass(self.render_pass, None);
            self.device.destroy_command_pool(self.pool, None);
            self.surface_loader.destroy_surface(self.surface, None);
            if let Some(callback) = self.debug_call_back {
                self.debug_utils_loader.destroy_debug_utils_messenger(callback, None);
            }
        }
        info!("VulkanBase: Resources dropped.");
    }
}
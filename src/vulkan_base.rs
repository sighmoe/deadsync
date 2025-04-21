#![warn(
    clippy::use_self,
    deprecated_in_future,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications
)]

use std::{
    borrow::Cow, default::Default, error::Error, ffi, ops::Drop, os::raw::c_char,
};

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    util::*,
    vk, Device, Entry, Instance,
};
use cgmath::Matrix4;
use log::{debug, error, info, warn};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

// Simple offset_of macro akin to C++ offsetof
#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = mem::zeroed();
            std::ptr::addr_of!(b.$field) as isize - std::ptr::addr_of!(b) as isize
        }
    }};
}

// Helper function remains the same
#[allow(clippy::too_many_arguments)]
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

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
            .expect("queue submit failed.");
    }
}

// Debug callback remains the same
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

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
        message_id_number,
        message,
    );

    vk::FALSE
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _memory_type)| index as _)
}

// Define the vertex structure used by this game (pos + texCoord)
#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub tex_coord: [f32; 2], // ADDED
}

// UBO struct for projection matrix
#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct UniformBufferObject {
    pub projection: Matrix4<f32>,
}

// Struct to hold buffer and memory together
pub struct BufferResource {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
    pub mapped_ptr: Option<*mut ffi::c_void>,
}

impl BufferResource {
    // Helper to cleanup
    pub fn destroy(&mut self, device: &Device) {
         unsafe {
            if let Some(ptr) = self.mapped_ptr {
                if !ptr.is_null() {
                    device.unmap_memory(self.memory);
                }
            }
            device.destroy_buffer(self.buffer, None);
            device.free_memory(self.memory, None);
        }
    }
}


pub struct VulkanBase {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub surface_loader: surface::Instance,
    pub swapchain_loader: swapchain::Device,
    pub debug_utils_loader: debug_utils::Instance,
    pub window: Window,
    pub debug_call_back: Option<vk::DebugUtilsMessengerEXT>,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub pdevice_properties: vk::PhysicalDeviceProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffers: Vec<vk::CommandBuffer>,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub render_pass: vk::RenderPass,
    pub framebuffers: Vec<vk::Framebuffer>,

    pub present_complete_semaphores: Vec<vk::Semaphore>,
    pub rendering_complete_semaphores: Vec<vk::Semaphore>,

    pub draw_commands_fences: Vec<vk::Fence>,
    pub setup_commands_reuse_fence: vk::Fence,

    pub frame_index: usize,
}

impl VulkanBase {
    pub fn new(window: Window) -> Result<Self, Box<dyn Error>> {
        unsafe {
            let window_width = window.inner_size().width;
            let window_height = window.inner_size().height;

            let entry = Entry::linked();
            let app_name = ffi::CStr::from_bytes_with_nul(b"AshRITG\0")?;

            let layer_names = [ffi::CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")?];
            let layers_names_raw: Vec<*const c_char> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let mut extension_names =
                ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?
                    .to_vec();
            extension_names.push(debug_utils::NAME.as_ptr());

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                extension_names.push(ash::khr::portability_enumeration::NAME.as_ptr());
                extension_names.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());
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

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .expect("Instance creation error");

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

            let debug_utils_loader = debug_utils::Instance::new(&entry, &instance);
            let debug_call_back = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None).ok();

            let surface = ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?;
            let pdevices = instance.enumerate_physical_devices()?;
            let surface_loader = surface::Instance::new(&entry, &instance);
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .find_map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                        .get_physical_device_surface_support(
                                            *pdevice,
                                            index as u32,
                                            surface,
                                        )
                                        .unwrap_or(false);
                            if supports_graphic_and_surface {
                                Some((*pdevice, index as u32))
                            } else {
                                None
                            }
                        })
                })
                .ok_or("Couldn't find suitable device.")?;

            // --- Device Creation ---
            let device_extension_names_raw = [
                swapchain::NAME.as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                ash::khr::portability_subset::NAME.as_ptr(),
            ];
            let features = vk::PhysicalDeviceFeatures {
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

            let device: Device = instance.create_device(pdevice, &device_create_info, None)?;
            let present_queue = device.get_device_queue(queue_family_index, 0);

            // --- Swapchain Setup ---
            let surface_format = surface_loader
                .get_physical_device_surface_formats(pdevice, surface)?
                .into_iter()
                .find(|f| {
                    f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM
                })
                .unwrap_or_else(|| {
                    surface_loader.get_physical_device_surface_formats(pdevice, surface).unwrap()[0]
                });

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(pdevice, surface)?;
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                u32::MAX => vk::Extent2D {
                    width: window_width,
                    height: window_height,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(pdevice, surface)?;
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);
            log::info!("Selected Present Mode: {:?}", present_mode);

            let swapchain_loader = swapchain::Device::new(&instance, &device);
            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .min_image_count(desired_image_count)
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

            // --- Image Views ---
            let present_images = swapchain_loader.get_swapchain_images(swapchain)?;
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::default()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        })
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

            // --- Command Pool ---
            let pool_create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);
            let pool = device.create_command_pool(&pool_create_info, None)?;

            // --- Command Buffers ---
            let command_buffer_count = present_image_views.len() as u32;
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(command_buffer_count + 1); // +1 for setup CB

            let all_command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)?;
            let setup_command_buffer = all_command_buffers[0];
            let draw_command_buffers = all_command_buffers[1..].to_vec();

            // --- Depth Buffer Resources ---
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

            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let pdevice_properties = instance.get_physical_device_properties(pdevice);

            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let depth_image_memory_index = find_memorytype_index(
                &depth_image_memory_req,
                &device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .ok_or("Unable to find suitable memory index for depth image.")?;

            let depth_image_allocate_info = vk::MemoryAllocateInfo::default()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index);
            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)?;
            device
                .bind_image_memory(depth_image, depth_image_memory, 0)?;

            // Transition depth image layout
            let setup_commands_reuse_fence = device
                .create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )?;
            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                setup_commands_reuse_fence,
                present_queue,
                &[], &[], &[],
                |device, setup_command_buffer| {
                    let layout_transition_barrier = vk::ImageMemoryBarrier::default()
                        .image(depth_image)
                        .src_access_mask(vk::AccessFlags::NONE)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1),
                        );

                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[], &[],
                        &[layout_transition_barrier],
                    );
                },
            );
            //device.wait_for_fences(&[setup_commands_reuse_fence], true, u64::MAX)?;
            //device.reset_fences(&[setup_commands_reuse_fence])?;

            let depth_image_view_info = vk::ImageViewCreateInfo::default()
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1),
                )
                .image(depth_image)
                .format(depth_format)
                .view_type(vk::ImageViewType::TYPE_2D);
            let depth_image_view = device.create_image_view(&depth_image_view_info, None)?;

            // --- Render Pass ---
            let renderpass_attachments = [
                // Color attachment
                vk::AttachmentDescription {
                    flags: vk::AttachmentDescriptionFlags::empty(),
                    format: surface_format.format,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::STORE,
                    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                    stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                    initial_layout: vk::ImageLayout::UNDEFINED,
                    final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                },
                // Depth attachment
                vk::AttachmentDescription {
                    flags: vk::AttachmentDescriptionFlags::empty(),
                    format: depth_format,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::DONT_CARE,
                    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                    stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                    initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
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
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                src_access_mask: vk::AccessFlags::NONE,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                ..Default::default()
            }];

            let subpass = vk::SubpassDescription::default()
                .color_attachments(&color_attachment_refs)
                .depth_stencil_attachment(&depth_attachment_ref)
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

            let renderpass_create_info = vk::RenderPassCreateInfo::default()
                .attachments(&renderpass_attachments)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(&dependencies);
            let render_pass = device.create_render_pass(&renderpass_create_info, None)?;

            // --- Framebuffers ---
            let framebuffers: Vec<vk::Framebuffer> = present_image_views
                .iter()
                .map(|&present_image_view| {
                    let framebuffer_attachments = [present_image_view, depth_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::default()
                        .render_pass(render_pass)
                        .attachments(&framebuffer_attachments)
                        .width(surface_resolution.width)
                        .height(surface_resolution.height)
                        .layers(1);
                    device.create_framebuffer(&frame_buffer_create_info, None)
                })
                .collect::<Result<Vec<_>, _>>()?;

            // --- Synchronization Objects (Per Frame) ---
            let semaphore_create_info = vk::SemaphoreCreateInfo::default();
            let fence_create_info = vk::FenceCreateInfo::default()
                                      .flags(vk::FenceCreateFlags::SIGNALED);

            let mut present_complete_semaphores = Vec::with_capacity(command_buffer_count as usize);
            let mut rendering_complete_semaphores = Vec::with_capacity(command_buffer_count as usize);
            let mut draw_commands_fences = Vec::with_capacity(command_buffer_count as usize);

            for _ in 0..command_buffer_count {
                present_complete_semaphores.push(device.create_semaphore(&semaphore_create_info, None)?);
                rendering_complete_semaphores.push(device.create_semaphore(&semaphore_create_info, None)?);
                draw_commands_fences.push(device.create_fence(&fence_create_info, None)?);
            }

            // Use VulkanBase explicitly instead of Self, pass in window
            Ok(VulkanBase {
                entry,
                instance,
                device,
                queue_family_index,
                pdevice,
                pdevice_properties,
                device_memory_properties,
                window,
                surface_loader,
                surface_format,
                present_queue,
                surface_resolution,
                swapchain_loader,
                swapchain,
                present_image_views,
                pool,
                draw_command_buffers,
                setup_command_buffer,
                depth_image,
                depth_image_view,
                depth_image_memory,
                render_pass,
                framebuffers,
                present_complete_semaphores,
                rendering_complete_semaphores,
                draw_commands_fences,
                setup_commands_reuse_fence,
                surface,
                debug_call_back,
                debug_utils_loader,
                frame_index: 0,
            })
        }
    }

     // Helper function to create buffers
    pub fn create_buffer(
        &self,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        memory_flags: vk::MemoryPropertyFlags,
    ) -> Result<BufferResource, Box<dyn Error>> {
        unsafe {
            let buffer_info = vk::BufferCreateInfo::default()
                .size(size)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let buffer = self.device.create_buffer(&buffer_info, None)?;

            let mem_requirements = self.device.get_buffer_memory_requirements(buffer);
            let mem_type_index = find_memorytype_index(
                &mem_requirements,
                &self.device_memory_properties,
                memory_flags,
            )
            .ok_or("Failed to find suitable memory type")?;

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(mem_requirements.size)
                .memory_type_index(mem_type_index);
            let memory = self.device.allocate_memory(&alloc_info, None)?;

            self.device.bind_buffer_memory(buffer, memory, 0)?;

            let mapped_ptr = if memory_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
                match self.device.map_memory(memory, 0, mem_requirements.size, vk::MemoryMapFlags::empty()) {
                    Ok(ptr) => Some(ptr),
                    Err(e) => {
                        log::warn!("Failed to map buffer memory: {}", e);
                        self.device.destroy_buffer(buffer, None);
                        self.device.free_memory(memory, None);
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

    // Helper to copy data to HOST_VISIBLE buffer
    pub fn update_buffer<T: Copy>(
        &self,
        buffer_resource: &BufferResource,
        data: &[T],
    ) -> Result<(), Box<dyn Error>> {
        use std::mem::{align_of, size_of};
        unsafe {
            let data_size = (data.len() * size_of::<T>()) as vk::DeviceSize;
            if data_size > buffer_resource.size {
                return Err(format!("Data size ({}) exceeds buffer size ({})", data_size, buffer_resource.size).into());
            }

            if let Some(ptr) = buffer_resource.mapped_ptr {
                let mut align = Align::new(ptr, align_of::<T>() as u64, buffer_resource.size);
                align.copy_from_slice(data);

                // Simplified: Assume we need to find the memory type again for properties.
                // In a real app, might store the memory type index or flags with the buffer.
                let mem_requirements = self.device.get_buffer_memory_requirements(buffer_resource.buffer);
                let mem_type_index = find_memorytype_index(
                    &mem_requirements,
                    &self.device_memory_properties,
                    vk::MemoryPropertyFlags::HOST_VISIBLE // We assume it must have this flag if mapped
                ).ok_or("Could not find buffer memory type index for flushing")?;

                let mem_type = &self.device_memory_properties.memory_types[mem_type_index as usize];

                if !mem_type.property_flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {
                    let flush_range = vk::MappedMemoryRange::default()
                        .memory(buffer_resource.memory)
                        .offset(0)
                        .size(vk::WHOLE_SIZE);
                    self.device.flush_mapped_memory_ranges(&[flush_range])?;
                }
            } else {
                return Err("Buffer is not mapped for host updates.".into());
            }
        }
        Ok(())
    }

    // Method to get GPU name
    pub fn get_gpu_name(&self) -> String {
        let props = self.pdevice_properties;
        let name_bytes = props.device_name.iter().map(|&c| c as u8).take_while(|&c| c != 0).collect::<Vec<_>>();
        String::from_utf8_lossy(&name_bytes).into_owned()
    }

    pub fn draw_frame<F>(&mut self, draw_commands_fn: F) -> Result<bool, vk::Result>
    where
        F: FnOnce(&Device, vk::CommandBuffer),
    {
        unsafe {
            // --- Per-Frame Setup ---
            let fence = self.draw_commands_fences[self.frame_index];
            let present_complete_semaphore = self.present_complete_semaphores[self.frame_index];
            let rendering_complete_semaphore = self.rendering_complete_semaphores[self.frame_index];
            let current_command_buffer = self.draw_command_buffers[self.frame_index];
    
            self.device
                .wait_for_fences(&[fence], true, u64::MAX)
                .expect("Fence wait failed");
    
            self.device
                .reset_fences(&[fence])
                .expect("Fence reset failed");
            self.device
                .reset_command_buffer(current_command_buffer, vk::CommandBufferResetFlags::empty())
                .expect("Reset command buffer failed");
    
            let (present_index, _suboptimal) = self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                present_complete_semaphore,
                vk::Fence::null(),
            )?;
    
            // --- Record Command Buffer ---
            let cmd_begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device
                .begin_command_buffer(current_command_buffer, &cmd_begin_info)
                .expect("Begin commandbuffer failed.");
    
            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.1, 0.1, 0.1, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];
            let render_pass_begin_info = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[present_index as usize])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.surface_resolution,
                })
                .clear_values(&clear_values);
    
            self.device.cmd_begin_render_pass(
                current_command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
            draw_commands_fn(&self.device, current_command_buffer);
            self.device.cmd_end_render_pass(current_command_buffer);
            self.device
                .end_command_buffer(current_command_buffer)
                .expect("End commandbuffer failed.");
    
            // --- Submit to Queue ---
            let wait_semaphores = [present_complete_semaphore];
            let wait_dst_stage_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [current_command_buffer];
            let signal_semaphores = [rendering_complete_semaphore];
    
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_dst_stage_mask)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);
    
            self.device
                .queue_submit(self.present_queue, &[submit_info], fence)
                .expect("Queue submit failed.");
    
            // --- Present ---
            let wait_semaphores_present = [rendering_complete_semaphore];
            let swapchains = [self.swapchain];
            let image_indices = [present_index];
    
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphores_present)
                .swapchains(&swapchains)
                .image_indices(&image_indices);
    
            let present_result = self
                .swapchain_loader
                .queue_present(self.present_queue, &present_info);
    
            // Determine if resize is needed
            let resize_needed = match present_result {
                Ok(suboptimal) => suboptimal, // true if suboptimal, false otherwise
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true, // Out of date, needs resize
                Err(e) => return Err(e), // Propagate other errors
            };
    
            self.frame_index = (self.frame_index + 1) % self.draw_command_buffers.len();
            Ok(resize_needed)
        }
    }
}

impl Drop for VulkanBase {
    fn drop(&mut self) {
        unsafe {
            // Ensure device is idle before destroying anything
            // Ignore error here as we are already dropping
            let _ = self.device.device_wait_idle();

            // Destroy synchronization objects
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

            // Destroy framebuffers
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            // Destroy render pass
            self.device.destroy_render_pass(self.render_pass, None);
            // Destroy depth buffer resources
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image(self.depth_image, None);
            // Destroy swapchain image views
            for &image_view in self.present_image_views.iter() { // Iterate before clearing
                self.device.destroy_image_view(image_view, None);
            }
            self.present_image_views.clear(); // Clear the vec after destroying
            // Destroy command pool (destroys command buffers)
            self.device.destroy_command_pool(self.pool, None);
            // Destroy swapchain
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            // Destroy device
            self.device.destroy_device(None);
            // Destroy surface
            self.surface_loader.destroy_surface(self.surface, None);
            // Destroy debug messenger (check if Some)
            if let Some(callback) = self.debug_call_back {
                self.debug_utils_loader
                    .destroy_debug_utils_messenger(callback, None);
            }
            // Destroy instance
            self.instance.destroy_instance(None);
            // Entry is automatically handled
            // Window drop is handled automatically when VulkanBase is dropped
        }
        log::info!("VulkanBase dropped and cleaned up.");
    }
}
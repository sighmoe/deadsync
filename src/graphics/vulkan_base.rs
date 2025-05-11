#![warn(
    clippy::use_self,
    deprecated_in_future,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications
)]

use std::{borrow::Cow, default::Default, error::Error, ffi, ops::Drop, os::raw::c_char};

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    vk, Device, Entry, Instance,
};
use cgmath::Matrix4;
use log::{debug, error, info, warn}; // Added info, debug, error
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

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
        // Changed to debug to reduce verbosity, can be info for more detail
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
            if self.buffer != vk::Buffer::null() {
                device.destroy_buffer(self.buffer, None);
                self.buffer = vk::Buffer::null();
            }
            if self.memory != vk::DeviceMemory::null() {
                device.free_memory(self.memory, None);
                self.memory = vk::DeviceMemory::null();
            }
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

    pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub pdevice_properties: vk::PhysicalDeviceProperties,
    queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    surface_format: vk::SurfaceFormatKHR,
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
            let entry = Entry::linked();
            let app_name = ffi::CStr::from_bytes_with_nul(b"DeadSyncVulkan\0")?; // Updated app name

            let layer_names = [ffi::CStr::from_bytes_with_nul(
                b"VK_LAYER_KHRONOS_validation\0",
            )?];
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
                // extension_names.push(ash::khr::get_physical_device_properties2::NAME.as_ptr()); // Not strictly needed for 1.1
            }

            let appinfo = vk::ApplicationInfo::default()
                .application_name(app_name)
                .application_version(vk::make_api_version(0, 0, 1, 0))
                .engine_name(app_name)
                .engine_version(vk::make_api_version(0, 0, 1, 0))
                .api_version(vk::API_VERSION_1_1); // Ensure this matches shader version if issues arise

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
                .map_err(|e| format!("Instance creation error: {}", e))?;

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        // | vk::DebugUtilsMessageSeverityFlagsEXT::INFO // Can be verbose
                        | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE, // Useful for detailed validation
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = debug_utils::Instance::new(&entry, &instance);
            let debug_call_back = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .ok(); // Optional: handle error if needed

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
                .ok_or("Couldn't find suitable physical device.")?;

            let pdevice_properties = instance.get_physical_device_properties(pdevice);
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);

            let device_extension_names_raw = [
                swapchain::NAME.as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                ash::khr::portability_subset::NAME.as_ptr(),
            ];
            let features = vk::PhysicalDeviceFeatures {
                sampler_anisotropy: vk::TRUE, // Example: Enable if you use it
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

            let device: Device = instance
                .create_device(pdevice, &device_create_info, None)
                .map_err(|e| format!("Device creation error: {}", e))?;
            let present_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format = surface_loader
                .get_physical_device_surface_formats(pdevice, surface)?
                .into_iter()
                .find(|f| {
                    f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM
                })
                .unwrap_or_else(|| {
                    warn!("Desired surface format not found, using first available.");
                    surface_loader
                        .get_physical_device_surface_formats(pdevice, surface)
                        .unwrap()[0]
                });

            let swapchain_loader = swapchain::Device::new(&instance, &device);

            // --- Command Pool ---
            let pool_create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);
            let pool = device.create_command_pool(&pool_create_info, None)?;

            // --- Command Buffers (Draw + 1 Setup) ---
            // Max frames in flight - typically 2 or 3 for triple buffering.
            // This will be the number of draw command buffers and sync objects.
            const MAX_FRAMES_IN_FLIGHT: u32 = 2;
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(MAX_FRAMES_IN_FLIGHT + 1); // +1 for setup CB

            let all_command_buffers =
                device.allocate_command_buffers(&command_buffer_allocate_info)?;
            let setup_command_buffer = all_command_buffers[0];
            let draw_command_buffers = all_command_buffers[1..].to_vec();
            debug_assert_eq!(draw_command_buffers.len(), MAX_FRAMES_IN_FLIGHT as usize);

            // --- Render Pass (created once, assuming format doesn't change) ---
            let renderpass_attachments = [
                vk::AttachmentDescription {
                    // Color
                    format: surface_format.format,
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
                    // Depth
                    format: vk::Format::D16_UNORM, // Fixed depth format
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::DONT_CARE, // Don't need to store depth
                    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
                    stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
                    initial_layout: vk::ImageLayout::UNDEFINED, // Start undefined for first use
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
                .attachments(&renderpass_attachments)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(&dependencies);
            let render_pass = device.create_render_pass(&renderpass_create_info, None)?;

            // --- Synchronization Objects ---
            let setup_commands_reuse_fence = device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), // Start signaled
                None,
            )?;

            let mut present_complete_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
            let mut rendering_complete_semaphores =
                Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
            let mut draw_commands_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();
            let fence_create_info =
                vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED); // Fences start signaled
            for _ in 0..MAX_FRAMES_IN_FLIGHT {
                present_complete_semaphores
                    .push(device.create_semaphore(&semaphore_create_info, None)?);
                rendering_complete_semaphores
                    .push(device.create_semaphore(&semaphore_create_info, None)?);
                draw_commands_fences.push(device.create_fence(&fence_create_info, None)?);
            }

            let mut base = Self {
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
                surface_resolution: vk::Extent2D::default(),
                swapchain_loader,
                swapchain: vk::SwapchainKHR::null(),
                present_image_views: Vec::new(),
                pool,
                draw_command_buffers,
                setup_command_buffer,
                depth_image: vk::Image::null(),
                depth_image_view: vk::ImageView::null(),
                depth_image_memory: vk::DeviceMemory::null(),
                render_pass,
                framebuffers: Vec::new(),
                present_complete_semaphores,
                rendering_complete_semaphores,
                draw_commands_fences,
                setup_commands_reuse_fence,
                surface,
                debug_call_back,
                debug_utils_loader,
                frame_index: 0,
            };

            let initial_size = base.window.inner_size();
            base.rebuild_swapchain_resources(initial_size.width, initial_size.height)?;

            Ok(base)
        }
    }

    fn destroy_swapchain_dependents(&mut self) {
        unsafe {
            debug!("Destroying swapchain dependents (Framebuffers, Depth Buffer, Image Views)...");
            for framebuffer in self.framebuffers.drain(..) {
                if framebuffer != vk::Framebuffer::null() {
                    self.device.destroy_framebuffer(framebuffer, None);
                }
            }
            if self.depth_image_view != vk::ImageView::null() {
                self.device.destroy_image_view(self.depth_image_view, None);
                self.depth_image_view = vk::ImageView::null();
            }
            if self.depth_image != vk::Image::null() {
                self.device.destroy_image(self.depth_image, None);
                self.depth_image = vk::Image::null();
            }
            if self.depth_image_memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.depth_image_memory, None);
                self.depth_image_memory = vk::DeviceMemory::null();
            }

            for view in self.present_image_views.drain(..) {
                if view != vk::ImageView::null() {
                    self.device.destroy_image_view(view, None);
                }
            }
            debug!("Swapchain dependents destroyed.");
        }
    }

    pub fn rebuild_swapchain_resources(
        &mut self,
        new_width: u32,
        new_height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.wait_idle()?;
        info!(
            "Rebuilding swapchain resources for requested size: {}x{}",
            new_width, new_height
        );

        self.destroy_swapchain_dependents();

        let old_swapchain_khr = self.swapchain;

        unsafe {
            let surface_capabilities = self
                .surface_loader
                .get_physical_device_surface_capabilities(self.pdevice, self.surface)?;

            self.surface_resolution = match surface_capabilities.current_extent.width {
                u32::MAX => vk::Extent2D {
                    width: new_width,
                    height: new_height,
                },
                _ => surface_capabilities.current_extent,
            };
            self.surface_resolution.width = self.surface_resolution.width.clamp(
                surface_capabilities.min_image_extent.width.max(1), // Ensure at least 1
                surface_capabilities.max_image_extent.width,
            );
            self.surface_resolution.height = self.surface_resolution.height.clamp(
                surface_capabilities.min_image_extent.height.max(1), // Ensure at least 1
                surface_capabilities.max_image_extent.height,
            );

            if self.surface_resolution.width == 0 || self.surface_resolution.height == 0 {
                warn!(
                    "Attempting to rebuild swapchain with zero extent ({:?}). Aborting rebuild.",
                    self.surface_resolution
                );
                self.swapchain = old_swapchain_khr;
                // If we aborted, dependents are destroyed but swapchain KHR object remains.
                // This path should ideally be prevented by App not calling with zero size.
                // The caller (App::handle_resize) should ensure non-zero dimensions.
                return Ok(());
            }
            info!(
                "Actual new surface resolution: {:?}",
                self.surface_resolution
            );

            // Number of images for the swapchain. Try to match MAX_FRAMES_IN_FLIGHT.
            // This also dictates the number of image views and framebuffers.
            let image_count =
                (self.draw_command_buffers.len() as u32).max(surface_capabilities.min_image_count);
            let image_count = if surface_capabilities.max_image_count > 0 {
                image_count.min(surface_capabilities.max_image_count)
            } else {
                image_count // No max limit
            };

            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = self
                .surface_loader
                .get_physical_device_surface_present_modes(self.pdevice, self.surface)?;
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX) // Prefer Mailbox for low latency
                .unwrap_or(vk::PresentModeKHR::FIFO); // Fallback to FIFO (guaranteed)

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(self.surface)
                .min_image_count(image_count) // Use the determined image_count
                .image_color_space(self.surface_format.color_space)
                .image_format(self.surface_format.format)
                .image_extent(self.surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1)
                .old_swapchain(old_swapchain_khr);

            self.swapchain = self
                .swapchain_loader
                .create_swapchain(&swapchain_create_info, None)?;
            info!(
                "New swapchain KHR object created with {} images.",
                image_count
            );

            if old_swapchain_khr != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(old_swapchain_khr, None);
                debug!("Old swapchain KHR object destroyed.");
            }

            let present_images = self.swapchain_loader.get_swapchain_images(self.swapchain)?;
            // Ensure present_image_views has the correct number of views
            // This should now match the new swapchain's actual image count
            self.present_image_views.clear(); // Clear old ones first (already destroyed by destroy_swapchain_dependents)
            for &image in present_images.iter() {
                let create_view_info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.surface_format.format)
                    .components(vk::ComponentMapping::default()) // Default is identity
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);
                self.present_image_views
                    .push(self.device.create_image_view(&create_view_info, None)?);
            }
            info!(
                "Present image views recreated: {} views.",
                self.present_image_views.len()
            );

            // Sync objects (semaphores, fences) count should match draw_command_buffers len (MAX_FRAMES_IN_FLIGHT)
            // They were already created with this count in `new()`. No need to recreate them here
            // unless MAX_FRAMES_IN_FLIGHT itself changed, which it doesn't.
            self.frame_index = 0;

            let depth_format = vk::Format::D16_UNORM;
            let depth_image_create_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(depth_format)
                .extent(self.surface_resolution.into())
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            self.depth_image = self.device.create_image(&depth_image_create_info, None)?;
            let depth_mem_req = self.device.get_image_memory_requirements(self.depth_image);
            let depth_mem_idx = find_memorytype_index(
                &depth_mem_req,
                &self.device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .ok_or("Failed to find memory type for depth image")?;
            let depth_alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(depth_mem_req.size)
                .memory_type_index(depth_mem_idx);
            self.depth_image_memory = self.device.allocate_memory(&depth_alloc_info, None)?;
            self.device
                .bind_image_memory(self.depth_image, self.depth_image_memory, 0)?;

            record_submit_commandbuffer(
                &self.device,
                self.setup_command_buffer,
                self.setup_commands_reuse_fence,
                self.present_queue,
                &[],
                &[],
                &[],
                |device, cmd_buf| {
                    let barrier = vk::ImageMemoryBarrier::default()
                        .image(self.depth_image)
                        .src_access_mask(vk::AccessFlags::NONE)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });
                    device.cmd_pipeline_barrier(
                        cmd_buf,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[barrier],
                    );
                },
            );
            // No need to wait/reset fence here, record_submit_commandbuffer handles it.

            let depth_view_info = vk::ImageViewCreateInfo::default()
                .image(self.depth_image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(depth_format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            self.depth_image_view = self.device.create_image_view(&depth_view_info, None)?;
            info!(
                "Depth buffer recreated for extent: {:?}",
                self.surface_resolution
            );

            // Framebuffers need to match the number of present_image_views
            self.framebuffers.clear(); // They were destroyed by destroy_swapchain_dependents
            for &present_view in self.present_image_views.iter() {
                let attachments = [present_view, self.depth_image_view];
                let fb_info = vk::FramebufferCreateInfo::default()
                    .render_pass(self.render_pass)
                    .attachments(&attachments)
                    .width(self.surface_resolution.width)
                    .height(self.surface_resolution.height)
                    .layers(1);
                self.framebuffers
                    .push(self.device.create_framebuffer(&fb_info, None)?);
            }
            info!(
                "Framebuffers recreated: {} framebuffers.",
                self.framebuffers.len()
            );
            debug_assert_eq!(self.framebuffers.len(), self.present_image_views.len());
        }
        Ok(())
    }

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
            .ok_or("Failed to find suitable memory type for buffer")?;

            let alloc_info = vk::MemoryAllocateInfo::default()
                .allocation_size(mem_requirements.size)
                .memory_type_index(mem_type_index);
            let memory = self.device.allocate_memory(&alloc_info, None)?;

            self.device.bind_buffer_memory(buffer, memory, 0)?;

            let mapped_ptr = if memory_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
                match self.device.map_memory(
                    memory,
                    0,
                    mem_requirements.size,
                    vk::MemoryMapFlags::empty(),
                ) {
                    Ok(ptr) => Some(ptr),
                    Err(e) => {
                        warn!("Failed to map buffer memory: {}", e);
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

    pub fn update_buffer<T: Copy>(
        &self,
        buffer_resource: &BufferResource,
        data: &[T],
    ) -> Result<(), Box<dyn Error>> {
        use std::ptr;
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
                ptr::copy_nonoverlapping(
                    data.as_ptr() as *const u8,
                    mapped_ptr as *mut u8,
                    data_size as usize,
                );

                let mem_requirements = self
                    .device
                    .get_buffer_memory_requirements(buffer_resource.buffer);
                let mem_type_index_opt = find_memorytype_index(
                    &mem_requirements,
                    &self.device_memory_properties,
                    vk::MemoryPropertyFlags::HOST_VISIBLE,
                );

                if let Some(mem_type_index) = mem_type_index_opt {
                    let mem_type =
                        &self.device_memory_properties.memory_types[mem_type_index as usize];
                    if !mem_type
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
                    {
                        let flush_range = vk::MappedMemoryRange::default()
                            .memory(buffer_resource.memory)
                            .offset(0)
                            .size(vk::WHOLE_SIZE);
                        self.device.flush_mapped_memory_ranges(&[flush_range])?;
                    }
                } else {
                    warn!("Could not find memory type index for a mapped buffer during update_buffer. Skipping flush check.");
                }
            } else {
                return Err("Buffer is not mapped (HOST_VISIBLE flag missing or map failed), cannot update directly.".into());
            }
        }
        Ok(())
    }

    pub fn wait_idle(&self) -> Result<(), vk::Result> {
        debug!("Waiting for device idle...");
        unsafe { self.device.device_wait_idle()? };
        debug!("Device idle.");
        Ok(())
    }

    pub fn get_gpu_name(&self) -> String {
        let props = self.pdevice_properties;
        let name_bytes = props
            .device_name
            .iter()
            .map(|&c| c as u8)
            .take_while(|&c| c != 0)
            .collect::<Vec<_>>();
        String::from_utf8_lossy(&name_bytes).into_owned()
    }

    pub fn draw_frame<F>(&mut self, draw_commands_fn: F) -> Result<bool, vk::Result>
    where
        F: FnOnce(&Device, vk::CommandBuffer),
    {
        unsafe {
            let current_frame_sync_idx = self.frame_index % self.draw_commands_fences.len();

            let fence = self.draw_commands_fences[current_frame_sync_idx];
            let present_complete_semaphore =
                self.present_complete_semaphores[current_frame_sync_idx];
            let rendering_complete_semaphore =
                self.rendering_complete_semaphores[current_frame_sync_idx];
            let current_command_buffer = self.draw_command_buffers[current_frame_sync_idx];

            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
            self.device.reset_fences(&[fence])?;
            self.device.reset_command_buffer(
                current_command_buffer,
                vk::CommandBufferResetFlags::empty(),
            )?;

            let acquire_result = self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                present_complete_semaphore,
                vk::Fence::null(),
            );

            let (present_index, suboptimal_acquire) = match acquire_result {
                Ok((index, suboptimal)) => (index, suboptimal),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return Ok(true);
                }
                Err(e) => return Err(e),
            };

            // Check if framebuffer for present_index is valid
            if present_index as usize >= self.framebuffers.len() {
                error!("Acquired present_index {} is out of bounds for framebuffers (len {}). This indicates a critical swapchain issue.", present_index, self.framebuffers.len());
                return Err(vk::Result::ERROR_OUT_OF_DATE_KHR); // Signal for rebuild
            }

            let cmd_begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device
                .begin_command_buffer(current_command_buffer, &cmd_begin_info)?;

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
                    offset: vk::Offset2D::default(),
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
            self.device.end_command_buffer(current_command_buffer)?;

            let wait_semaphores = [present_complete_semaphore];
            let wait_dst_stage_mask = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers_submit = [current_command_buffer];
            let signal_semaphores = [rendering_complete_semaphore];

            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_dst_stage_mask)
                .command_buffers(&command_buffers_submit)
                .signal_semaphores(&signal_semaphores);

            self.device
                .queue_submit(self.present_queue, &[submit_info], fence)?;

            let wait_semaphores_present = [rendering_complete_semaphore];
            let swapchains_present = [self.swapchain];
            let image_indices_present = [present_index];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphores_present)
                .swapchains(&swapchains_present)
                .image_indices(&image_indices_present);

            let present_result = self
                .swapchain_loader
                .queue_present(self.present_queue, &present_info);

            self.frame_index = (self.frame_index + 1) % self.draw_command_buffers.len(); // Cycle through available command buffers / sync objects

            match present_result {
                Ok(suboptimal_present) => Ok(suboptimal_acquire || suboptimal_present),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => Ok(true),
                Err(e) => Err(e),
            }
        }
    }
}

impl Drop for VulkanBase {
    fn drop(&mut self) {
        unsafe {
            // No need to check self.device.handle(), ash::Device's own Drop handles it.
            // device_wait_idle is crucial before destroying anything.
            //if self.device.fp_v1_0().device_wait_idle != std::ptr::null_mut() { // Check if function pointer is valid
            //    let _ = self.device.device_wait_idle(); // Call it if device likely initialized
            // }

            let _ = self.device.device_wait_idle();

            for fence in self.draw_commands_fences.drain(..) {
                if fence != vk::Fence::null() {
                    self.device.destroy_fence(fence, None);
                }
            }
            if self.setup_commands_reuse_fence != vk::Fence::null() {
                self.device
                    .destroy_fence(self.setup_commands_reuse_fence, None);
            }
            for semaphore in self.present_complete_semaphores.drain(..) {
                if semaphore != vk::Semaphore::null() {
                    self.device.destroy_semaphore(semaphore, None);
                }
            }
            for semaphore in self.rendering_complete_semaphores.drain(..) {
                if semaphore != vk::Semaphore::null() {
                    self.device.destroy_semaphore(semaphore, None);
                }
            }

            self.destroy_swapchain_dependents();

            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(self.swapchain, None);
            }
            if self.render_pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.render_pass, None);
            }
            if self.pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(self.pool, None);
            }

            // self.device is dropped automatically by ash::Device's Drop impl,
            // which calls vkDestroyDevice. We don't need to call it explicitly here
            // if self.device is a full ash::Device struct.
            // However, if we called self.device.destroy_device(None) earlier,
            // then the ash::Device Drop would be a no-op on a null handle.
            // The pattern is: ash::Device's Drop will call vkDestroyDevice.
            // So, no explicit self.device.destroy_device(None) needed here.

            if self.surface != vk::SurfaceKHR::null() {
                self.surface_loader.destroy_surface(self.surface, None);
            }
            if let Some(callback) = self.debug_call_back {
                if callback != vk::DebugUtilsMessengerEXT::null() {
                    self.debug_utils_loader
                        .destroy_debug_utils_messenger(callback, None);
                }
            }
            // self.instance is dropped automatically by ash::Instance's Drop impl.
        }
        info!("VulkanBase dropped and cleaned up.");
    }
}
